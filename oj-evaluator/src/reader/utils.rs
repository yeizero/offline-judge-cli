use std::{fs::create_dir_all, io, path::Path};

pub fn ensure_dir_exists<P: AsRef<Path>>(folder_path: P) -> io::Result<()> {
    let path = folder_path.as_ref();

    if !path.is_dir() {
        create_dir_all(path)
    } else {
        Ok(())
    }
}
