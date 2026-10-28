use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_BYTES: u64 = 1024 * 1024;
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct Document(pub Value);
impl Default for Document {
    fn default() -> Self {
        Self(json!({"schema":1,"modules":{},"settings":{},"friends":[],"enemies":[],"ui":{}}))
    }
}
impl Document {
    pub fn parse(bytes: &[u8], imported: bool) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_BYTES {
            return Err("profile exceeds 1 MiB".into());
        }
        let mut value: Value =
            serde_json::from_slice(bytes).map_err(|e| format!("invalid profile JSON: {e}"))?;
        let object = value.as_object_mut().ok_or("profile must be an object")?;
        let schema = object
            .get("schema")
            .and_then(Value::as_u64)
            .ok_or("missing profile schema")?;
        match schema {
            0 => {
                object.insert("schema".into(), json!(1));
            }
            1 => {}
            _ => return Err(format!("unsupported profile schema {schema}")),
        }
        for name in ["modules", "settings", "ui"] {
            if !object.contains_key(name) {
                object.insert(name.into(), json!({}));
            }
            if !object[name].is_object() {
                return Err(format!("profile {name} must be an object"));
            }
        }
        for name in ["friends", "enemies"] {
            if !object.contains_key(name) {
                object.insert(name.into(), json!([]));
            }
            let entries = object[name]
                .as_array()
                .ok_or_else(|| format!("profile {name} must be an array"))?;
            if entries.len() > 4096 || entries.iter().any(|v| !v.is_object()) {
                return Err(format!("invalid local {name} list"));
            }
        }
        for (id, module) in object.get_mut("modules").unwrap().as_object_mut().unwrap() {
            if !super::settings::valid_id(id) || !module.is_object() {
                return Err(format!("invalid module entry {id}"));
            }
            let fields = module.as_object_mut().unwrap();
            if fields.get("enabled").is_some_and(|v| !v.is_boolean()) {
                return Err(format!("invalid enabled state for {id}"));
            }
            // Import is a data operation, never an implicit activation request.
            if imported {
                fields.insert("enabled".into(), json!(false));
            }
        }
        Ok(Self(value))
    }
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec_pretty(&self.0).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        if bytes.len() as u64 > MAX_BYTES {
            return Err("profile exceeds 1 MiB".into());
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct Profiles {
    root: PathBuf,
}
impl Profiles {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        if root.is_relative() {
            return Err("profile root must be absolute and instance-owned".into());
        }
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        reject_link(&root)?;
        let profiles = root.join("profiles");
        if profiles.exists() {
            reject_link(&profiles)?;
        }
        fs::create_dir_all(profiles).map_err(|e| e.to_string())?;
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn path(&self, name: &str) -> Result<PathBuf, String> {
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err("profile names use 1–64 letters, digits, - or _".into());
        }
        reject_link(&self.root.join("profiles"))?;
        Ok(self.root.join("profiles").join(format!("{name}.json")))
    }
    pub fn list(&self) -> Result<Vec<String>, String> {
        reject_link(&self.root.join("profiles"))?;
        let mut result = Vec::new();
        for entry in fs::read_dir(self.root.join("profiles")).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "json")
                && entry.file_type().map_err(|e| e.to_string())?.is_file()
                && let Some(name) = p.file_stem().and_then(|n| n.to_str())
                && self.path(name).is_ok()
            {
                result.push(name.into());
            }
        }
        result.sort();
        Ok(result)
    }
    pub fn save(&self, name: &str, document: &Document) -> Result<(), String> {
        atomic_write(&self.path(name)?, &document.bytes()?)
    }
    pub fn load(&self, name: &str) -> Result<Document, String> {
        read_document(&self.path(name)?, false)
    }
    pub fn import(&self, name: &str, bytes: &[u8]) -> Result<Document, String> {
        let document = Document::parse(bytes, true)?;
        let path = self.path(name)?;
        if path.exists() {
            return Err("profile already exists; choose a new name".into());
        }
        atomic_write(&path, &document.bytes()?)?;
        Ok(document)
    }
    pub fn export(&self, name: &str) -> Result<Vec<u8>, String> {
        self.load(name)?.bytes()
    }
    pub fn rename(&self, old: &str, new: &str) -> Result<(), String> {
        let old = self.path(old)?;
        let new = self.path(new)?;
        reject_link(&old)?;
        if new.exists() {
            return Err("profile already exists".into());
        }
        fs::rename(old, new).map_err(|e| e.to_string())
    }
    pub fn delete(&self, name: &str) -> Result<(), String> {
        let path = self.path(name)?;
        reject_link(&path)?;
        fs::remove_file(path).map_err(|e| e.to_string())
    }
    pub fn current(&self) -> Result<Option<Document>, String> {
        let path = self.root.join("current.json");
        if !path.exists() {
            return Ok(None);
        }
        read_document(&path, false).map(Some)
    }
    pub fn save_current(&self, document: &Document) -> Result<(), String> {
        atomic_write(&self.root.join("current.json"), &document.bytes()?)
    }
}
fn reject_link(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err(format!("refusing profile symlink: {}", path.display()));
    }
    Ok(())
}
fn read_document(path: &Path, imported: bool) -> Result<Document, String> {
    reject_link(path)?;
    let file = File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("profile is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Document::parse(&bytes, imported)
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        reject_link(path)?;
    }
    let parent = path.parent().ok_or("profile has no parent")?;
    reject_link(parent)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    for _ in 0..16 {
        let temporary = parent.join(format!(
            ".vape-{}-{}.tmp",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match options.open(&temporary) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)?;
            #[cfg(unix)]
            {
                File::open(parent)?.sync_all()?;
            }
            Ok::<(), std::io::Error>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return result.map_err(|e| e.to_string());
    }
    Err("could not allocate temporary profile file".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "vape-profile-test-{}-{}",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn imported_profiles_preserve_unknown_data_but_never_auto_enable() {
        let doc=Document::parse(br#"{"schema":1,"modules":{"hud.fps":{"enabled":true,"future":4}},"extension":{"x":1}}"#,true).unwrap();
        assert_eq!(doc.0["modules"]["hud.fps"]["enabled"], json!(false));
        assert_eq!(doc.0["modules"]["hud.fps"]["future"], json!(4));
        assert_eq!(doc.0["extension"]["x"], json!(1));
        assert!(Document::parse(br#"{"schema":999}"#, false).is_err());
    }
    #[test]
    fn save_replace_load_rename_export_and_delete_are_local() {
        let scratch = Scratch::new();
        let store = Profiles::new(scratch.0.join("mod")).unwrap();
        let mut doc = Document::default();
        doc.0["modules"] = json!({"hud.fps":{"enabled":true}});
        store.save("one", &doc).unwrap();
        doc.0["ui"] = json!({"x":42});
        store.save("one", &doc).unwrap();
        assert_eq!(store.load("one").unwrap().0, doc.0);
        store.rename("one", "two").unwrap();
        assert_eq!(store.list().unwrap(), vec!["two"]);
        assert!(store.load("one").is_err());
        let imported = store.import("copy", &store.export("two").unwrap()).unwrap();
        assert_eq!(imported.0["modules"]["hud.fps"]["enabled"], json!(false));
        store.delete("two").unwrap();
        assert_eq!(store.list().unwrap(), vec!["copy"]);
        store.save_current(&doc).unwrap();
        assert_eq!(store.current().unwrap().unwrap().0, doc.0);
        assert_eq!(
            fs::read_dir(store.root())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
                .count(),
            0
        );
    }
    #[test]
    fn invalid_paths_and_oversized_or_malformed_documents_fail_without_writes() {
        let scratch = Scratch::new();
        let store = Profiles::new(scratch.0.join("mod")).unwrap();
        for name in ["../escape", "/tmp/escape", "", "a/b", "..", "a\\b"] {
            assert!(store.save(name, &Document::default()).is_err());
        }
        assert!(store.import("broken", b"{}").is_err());
        assert!(
            store
                .import("big", &vec![b' '; (MAX_BYTES + 1) as usize])
                .is_err()
        );
        assert!(store.list().unwrap().is_empty());
    }
    #[test]
    fn schema_zero_migrates_and_wrong_field_types_are_rejected() {
        assert_eq!(
            Document::parse(br#"{"schema":0}"#, false).unwrap().0["schema"],
            json!(1)
        );
        for bytes in [
            br#"{"schema":1,"modules":[]}"#.as_slice(),
            br#"{"schema":1,"friends":true}"#.as_slice(),
            br#"{"schema":1,"modules":{"x":{"enabled":"yes"}}}"#.as_slice(),
        ] {
            assert!(Document::parse(bytes, false).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn profiles_do_not_follow_symlinks() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        let store = Profiles::new(scratch.0.join("mod")).unwrap();
        let outside = scratch.0.join("outside.json");
        fs::write(&outside, b"unchanged").unwrap();
        symlink(&outside, store.root().join("profiles/link.json")).unwrap();
        assert!(store.save("link", &Document::default()).is_err());
        assert!(store.load("link").is_err());
        assert!(store.delete("link").is_err());
        assert_eq!(fs::read(outside).unwrap(), b"unchanged");
    }
}
