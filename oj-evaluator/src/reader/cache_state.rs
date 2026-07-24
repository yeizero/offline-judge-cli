use anyhow::{Result, anyhow, bail};
use camino::Utf8Path;
use rkyv::{Archive, Deserialize, Serialize, rancor};
use std::{fs, path::Path, time::SystemTime};

use crate::config::KEEP_FILE;

/// Manages the cache state to determine if a recompile is needed.
pub struct FileCacheState {
    old_cache: Option<Cache>,
    new_cache: Cache,
}

impl FileCacheState {
    pub fn new(source_path: impl AsRef<Utf8Path>) -> Result<Self> {
        let new_cache = Cache::from_source_file(source_path)?;

        let old_cache = Cache::read_from(KEEP_FILE.as_path())
            .inspect_err(|e| log::debug!("Failed to read cache file: {e}"))
            .ok();

        Ok(Self {
            old_cache,
            new_cache,
        })
    }

    /// Return false if the source file needs to be recompiled.
    ///
    /// A recompile is needed if:
    /// - There is no previous cache.
    /// - The source file path or its modification time has changed.
    pub fn is_fresh(&self) -> bool {
        self.old_cache.as_ref() == Some(&self.new_cache)
    }

    /// Saves the current cache state to the disk.
    pub fn save(&self) -> Result<()> {
        self.new_cache.write_to(KEEP_FILE.as_path())
    }
}

const VERSION_MARK: u32 = 0xF000_0001;

#[derive(Archive, Deserialize, Serialize, PartialEq, Debug)]
struct Cache {
    version_mark: u32,
    source_path: String,
    source_mtime: u64,
    source_size: u32,
}

impl Cache {
    /// Creates a new Cache instance from a source file's metadata.
    fn from_source_file(path: impl AsRef<Utf8Path>) -> Result<Self> {
        let path = path.as_ref();
        let metadata =
            fs::metadata(path).map_err(|e| anyhow!("Failed to get metadata for '{path}': {e}"))?;

        let mtime = metadata
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?;

        // The logic for calculating a stable, low-precision timestamp.
        // Division by 64 reduces sensitivity to insignificant sub-second changes.
        // Adding 1 ensures the timestamp is never zero, which stands for an empty/invalid state.
        #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
        let mtime_hash = ((mtime.as_millis() / 64) % (u64::MAX as u128)) as u64 + 1;

        Ok(Self {
            version_mark: VERSION_MARK,
            source_path: fs::canonicalize(path)?.to_string_lossy().into_owned(),
            source_mtime: mtime_hash,
            #[allow(clippy::cast_possible_truncation, reason = "only be used to cache")]
            source_size: metadata.len() as u32,
        })
    }

    /// Reads and deserializes a Cache from a given path.
    fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = fs::read(path.as_ref())?;
        let cache = rkyv::from_bytes::<Self, rancor::Error>(&bytes)
            .map_err(|e| anyhow!("Rkyv deserialization error: {e}"))?;
        if cache.version_mark != VERSION_MARK {
            bail!("Mismatch version");
        }
        Ok(cache)
    }

    /// Serializes and writes the Cache to a given path.
    fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        fs::write(path, rkyv::to_bytes::<rancor::Error>(self)?)?;
        Ok(())
    }
}
