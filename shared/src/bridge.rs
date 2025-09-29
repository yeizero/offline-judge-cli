use fs_err::OpenOptions;
use memmap2::Mmap;
use memmap2::MmapMut;
use musli::{Decode, Encode};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub struct FastEditorConfigProtocal {
    pub keymap: Vec<FastKeyMapProtocol>,
}

#[derive(Debug)]
pub struct KeyMapListProtocal(Vec<FastKeyMapProtocol>);

#[derive(Debug, Encode, Decode)]
#[musli(packed)]
pub struct FastKeyMapProtocol {
    pub input: String,
    pub command: String,
}

impl<'de> Deserialize<'de> for KeyMapListProtocal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CEVisitor;

        impl<'de> Visitor<'de> for CEVisitor {
            type Value = KeyMapListProtocal;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a JSON object to convert into Vec<KeyMapList>")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut result = Vec::new();

                while let Some((input, command)) = access.next_entry()? {
                    result.push(FastKeyMapProtocol { input, command });
                }

                Ok(KeyMapListProtocal(result))
            }
        }

        deserializer.deserialize_map(CEVisitor)
    }
}

/// # Safety
/// 無法保證檔案突然失效、被突然修改
pub unsafe fn read_keymap_from_file(path: &str) -> anyhow::Result<Vec<FastKeyMapProtocol>> {
    let file = OpenOptions::new().read(true).open(path)?;

    let mmap = unsafe { Mmap::map(&file)? };
    let bytes = &mmap[..];
    let keymap: Vec<FastKeyMapProtocol> = musli::packed::from_slice(bytes)?;

    Ok(keymap)
}

/// # Safety
/// 無法保證檔案突然失效、無法寫入
pub unsafe fn write_keymap_to_file(path: impl Into<PathBuf>, keymap: &KeyMapListProtocal) -> anyhow::Result<()> {
    let bytes = musli::packed::to_vec(&keymap.0)?;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;

    file.set_len(bytes.len() as u64)?;

    let mut mmap = unsafe { MmapMut::map_mut(&file)? };

    mmap[..bytes.len()].copy_from_slice(bytes.as_slice());
    mmap.flush()?;

    Ok(())
}
