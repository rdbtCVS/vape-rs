use super::vape_save_worker::Writer;
use horizon_world::nbt::Tag;
use std::sync::OnceLock;

static WRITER: OnceLock<Writer<Tag>> = OnceLock::new();

pub fn submit(path: std::path::PathBuf, tag: Tag) {
    WRITER
        .get_or_init(|| {
            Writer::new(|path, tag| {
                crate::save::write_dat(&path, tag).map_err(|error| {
                    let message = format!("failed to save player {}: {error}", path.display());
                    log::warn!("{message}");
                    message
                })
            })
        })
        .submit(path, tag);
}

pub fn flush() -> Result<(), String> {
    WRITER.get().map_or(Ok(()), Writer::flush)
}
