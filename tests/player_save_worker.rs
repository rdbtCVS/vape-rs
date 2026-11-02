// Compile the exact worker shipped in the native bundle in the normal test gate.
#[path = "../mixins/vape_rs/save_worker.rs"]
mod save_worker;

#[test]
fn flush_persists_owned_snapshots_in_an_isolated_directory() {
    let root = std::env::temp_dir().join(format!(
        "vape-player-writer-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let writer = save_worker::Writer::new(|path, bytes: Vec<u8>| {
        std::fs::write(path, bytes).map_err(|e| e.to_string())
    });
    for i in 0..150 {
        let mut bytes = format!("snapshot {i}").into_bytes();
        writer.submit(root.join(format!("{i}.dat")), bytes.clone());
        bytes.fill(0); // Subsequent live state cannot change the queued snapshot.
    }
    writer.flush().unwrap();
    for i in 0..150 {
        assert_eq!(
            std::fs::read(root.join(format!("{i}.dat"))).unwrap(),
            format!("snapshot {i}").into_bytes()
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
