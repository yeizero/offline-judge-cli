mod command;
mod editor;

use std::io::{self, BufReader, BufWriter};

use crate::{command::merge_keymap, editor::Editor};

use argh::FromArgs;
use fs_err as fs;
use ropey::Rope;
use shared::bridge::read_keymap_from_file;

#[derive(FromArgs)]
struct Args {
    #[argh(positional)]
    file: Option<String>,

    #[argh(option, arg_name="input-fast")]
    input_fast: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    let mut editor = if let Some(path) = &args.file {
        match fs::File::open(path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                Editor::from_rope(Rope::from_reader(reader)?)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Editor::new(),
            Err(e) => return Err(e.into()),
        }
    } else {
        // args.file == None
        Editor::new()
    };

    if let Some(path) = &args.input_fast {
        let user_config = unsafe { read_keymap_from_file(path)? };
        editor.keymap = merge_keymap(editor.keymap, user_config);
    }

    editor.run()?;

    if let Some(path) = &args.file {
        let file = fs::File::create(path)?;
        let writer = BufWriter::new(file);
        editor.text().write_to(writer)?;
    }

    Ok(())
}
