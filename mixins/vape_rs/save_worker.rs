use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};

struct State<T> {
    pending: BTreeMap<PathBuf, T>,
    writing: bool,
    error: Option<String>,
}

/// One writer serializes replacements; pending snapshots coalesce by file.
/// Only the worker calls `write`, including compression and directory creation.
pub struct Writer<T> {
    shared: Arc<(Mutex<State<T>>, Condvar)>,
}

impl<T: Send + 'static> Writer<T> {
    pub fn new(write: impl Fn(PathBuf, T) -> Result<(), String> + Send + 'static) -> Self {
        let shared = Arc::new((
            Mutex::new(State {
                pending: BTreeMap::new(),
                writing: false,
                error: None,
            }),
            Condvar::new(),
        ));
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("player-file-writer".into())
            .spawn(move || {
                let (lock, wake) = &*worker;
                loop {
                    let mut state = lock.lock().unwrap();
                    while state.pending.is_empty() {
                        state = wake.wait(state).unwrap();
                    }
                    let (path, snapshot) = state.pending.pop_first().unwrap();
                    state.writing = true;
                    drop(state);
                    let result = write(path, snapshot);
                    let mut state = lock.lock().unwrap();
                    if let Err(error) = result {
                        state.error = Some(error);
                    }
                    state.writing = false;
                    wake.notify_all();
                }
            })
            .expect("start player file writer");
        Self { shared }
    }

    pub fn submit(&self, path: PathBuf, snapshot: T) {
        let (lock, wake) = &*self.shared;
        lock.lock().unwrap().pending.insert(path, snapshot);
        wake.notify_all();
    }

    /// Explicit saves, reads and shutdown wait for every preceding snapshot.
    pub fn flush(&self) -> Result<(), String> {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().unwrap();
        while state.writing || !state.pending.is_empty() {
            state = wake.wait(state).unwrap();
        }
        state.error.take().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn slow_io_does_not_block_submissions_and_latest_snapshot_wins() {
        let (started, ready) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let writes = Arc::new(Mutex::new(Vec::new()));
        let results = writes.clone();
        let caller = std::thread::current().id();
        let writer = Writer::new(move |path, value| {
            assert_ne!(caller, std::thread::current().id());
            if value == 0 {
                started.send(()).unwrap();
                wait.recv().unwrap();
            }
            results.lock().unwrap().push((path, value));
            Ok(())
        });
        writer.submit("player.dat".into(), 0);
        ready.recv().unwrap();
        // All 150 players enqueue while the first physical write is blocked.
        for i in 0..150 {
            writer.submit(format!("{i}.dat").into(), 1);
            writer.submit(format!("{i}.dat").into(), 2);
        }
        writer.submit("player.dat".into(), 3);
        release.send(()).unwrap();
        writer.flush().unwrap();
        let writes = writes.lock().unwrap();
        assert_eq!(writes.len(), 152);
        assert_eq!(writes.last(), Some(&(PathBuf::from("player.dat"), 3)));
        assert!(
            writes
                .iter()
                .skip(1)
                .take(150)
                .all(|(_, value)| *value == 2)
        );
    }

    #[test]
    fn flush_reports_failures_and_worker_continues() {
        let writer = Writer::new(|_, fail| {
            if fail {
                Err("disk full".into())
            } else {
                Ok(())
            }
        });
        writer.submit("bad.dat".into(), true);
        assert_eq!(writer.flush(), Err("disk full".into()));
        writer.submit("good.dat".into(), false);
        assert_eq!(writer.flush(), Ok(()));
    }
}
