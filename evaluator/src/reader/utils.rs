use std::{
    fs::create_dir_all,
    io,
    path::{Path, PathBuf},
};

pub fn change_extension<P: AsRef<Path>>(path: P, new_extension: &str) -> PathBuf {
    let mut path = path.as_ref().to_path_buf();
    path.set_extension(new_extension);
    path
}

pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
    let path = Path::new(path.as_ref());
    path.exists() && path.is_file()
}

pub fn ensure_dir_exists<P: AsRef<Path>>(folder_path: P) -> io::Result<()> {
    let path = folder_path.as_ref();

    if !path.is_dir() {
        create_dir_all(path)
    } else {
        Ok(())
    }
}
