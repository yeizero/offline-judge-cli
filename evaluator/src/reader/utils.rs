use std::{fs::create_dir_all, path::{Path, PathBuf}};

pub fn change_extension<P: AsRef<Path>>(path: P, new_extension: &str) -> PathBuf {
  let mut path = path.as_ref().to_path_buf();
  path.set_extension(new_extension);
  path
}

pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
  let path = Path::new(path.as_ref());
  path.exists() && path.is_file()
}

pub fn ensure_dir_exists<P: AsRef<Path>>(folder_path: P) -> PathBuf {
  let path = folder_path.as_ref();

  if !path.exists() {
      create_dir_all(path).unwrap();
  }

  path.to_path_buf()
}