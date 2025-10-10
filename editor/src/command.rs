use std::{collections::HashMap, str::FromStr};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use shared::bridge::FastKeyMapProtocol;

macro_rules! _keymap_entry {
    (@internal [ $( $mods:ident )* ] $next_mod:ident + $( $rest:tt )+) => {
        _keymap_entry!(@internal [ $( $mods )* $next_mod ] $( $rest )+)
    };

    (@internal [ $( $modifier:ident )* ] $code_func:ident ( $( $args:tt )* ) => $command:expr) => {
        (InputEvent {
            code: KeyCode::$code_func ( $( $args )* ),
            modifiers: $( KeyModifiers::$modifier | )* KeyModifiers::NONE
        }, $command)
    };

    (@internal [ $( $modifier:ident )* ] $code:ident => $command:expr) => {
        (InputEvent {
            code: KeyCode::$code,
            modifiers: $( KeyModifiers::$modifier | )* KeyModifiers::NONE
        }, $command)
    };

    ( $( $tokens:tt )+ ) => {
        _keymap_entry!(@internal [] $( $tokens )+)
    };
}

macro_rules! keymap {
    ( $( ( $( $entry:tt )+ ) ),* $(,)? ) => {
        {
            use Command::*;
            let entries = [$( _keymap_entry!( $( $entry )+ )),*];
            let mut map = HashMap::with_capacity(entries.len());
            map.extend(entries);
            map
        }
    };
}

pub fn default_keymap() -> HashMap<InputEvent, Command> {
    keymap! {
        // --- Text Editing ---
        (Enter                      => InputEnter),
        (SHIFT + Enter              => InputEnter),
        (Backspace                  => DeleteLeft),
        (Delete                     => DeleteRight),
        (SHIFT + Backspace          => DeleteLeft),
        (SHIFT + Delete             => DeleteRight),
        (CONTROL + Backspace        => DeleteWordLeft),
        (CONTROL + Delete           => DeleteWordRight),
        (CONTROL + SHIFT + Backspace => DeleteWordLeft),
        (CONTROL + SHIFT + Delete   => DeleteWordRight),
        (CONTROL + Char('w')        => DeleteWordLeft),
        (ALT + Char('d')            => DeleteWordRight),

        // --- Cursor Movement ---
        (Up                         => CursorUp),
        (Down                       => CursorDown),
        (Left                       => CursorLeft),
        (Right                      => CursorRight),
        (SHIFT + Up                 => CursorUp),
        (SHIFT + Down               => CursorDown),
        (SHIFT + Left               => CursorLeft),
        (SHIFT + Right              => CursorRight),
        (CONTROL + Up               => CursorUp),
        (CONTROL + Down             => CursorDown),
        (CONTROL + Left             => CursorWordLeft),
        (CONTROL + Right            => CursorWordRight),
        (CONTROL + SHIFT + Up       => CursorUp),
        (CONTROL + SHIFT + Down     => CursorDown),
        (CONTROL + SHIFT + Left     => CursorWordLeft),
        (CONTROL + SHIFT + Right    => CursorWordRight),

        // --- Page/Line Navigation ---
        (Home       => CursorHome),
        (End        => CursorEnd),
        (PageUp     => CursorPageUp),
        (PageDown   => CursorPageDown),

        // --- Selection ---
        (CONTROL + Char('a') => SelectAll),
        (CONTROL + Char('l') => SelectLine),

        // --- Clipboard ---
        (CONTROL + Char('c') => TextCopy),
        (CONTROL + Char('x') => TextCut),
        (CONTROL + Char('v') => TextPaste),
        // (CONTROL + Char('n') => TextPaste),

        // --- Application ---
        (Esc => Exit)
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Command {
    InputChar(char),
    InputEnter,
    DeleteLeft,
    DeleteRight,
    DeleteWordLeft,
    DeleteWordRight,
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorWordLeft,
    CursorWordRight,
    CursorHome,
    CursorEnd,
    CursorPageUp,
    CursorPageDown,
    SelectAll,
    SelectLine,
    TextCopy,
    TextCut,
    TextPaste,
    TextCopyAndClearSelection,
    Exit,
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "inputenter" => Ok(Command::InputEnter),
            "deleteleft" => Ok(Command::DeleteLeft),
            "deleteright" => Ok(Command::DeleteRight),
            "deletewordleft" => Ok(Command::DeleteWordLeft),
            "deletewordright" => Ok(Command::DeleteWordRight),
            "cursorup" => Ok(Command::CursorUp),
            "cursordown" => Ok(Command::CursorDown),
            "cursorleft" => Ok(Command::CursorLeft),
            "cursorright" => Ok(Command::CursorRight),
            "cursorwordleft" => Ok(Command::CursorWordLeft),
            "cursorwordright" => Ok(Command::CursorWordRight),
            "cursorhome" => Ok(Command::CursorHome),
            "cursorend" => Ok(Command::CursorEnd),
            "cursorpageup" => Ok(Command::CursorPageUp),
            "cursorpagedown" => Ok(Command::CursorPageDown),
            "selectall" => Ok(Command::SelectAll),
            "selectline" => Ok(Command::SelectLine),
            "textcopy" => Ok(Command::TextCopy),
            "textcut" => Ok(Command::TextCut),
            "textpaste" => Ok(Command::TextPaste),
            "textcopyandclearselection" => Ok(Command::TextCopyAndClearSelection),
            "exit" => Ok(Command::Exit),
            _ => Err(format!("Unknown command: '{}'", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl From<KeyEvent> for InputEvent {
    fn from(ev: KeyEvent) -> Self {
        Self {
            code: ev.code,
            modifiers: ev.modifiers,
        }
    }
}

fn parse_input_event(s: &str) -> Result<InputEvent, String> {
    let mut modifiers = KeyModifiers::empty();

    let mut parts = s.split('+');

    let key_part = match parts.next_back() {
        Some(part) if !part.is_empty() => part,
        _ => return Err("Input must specify a key".to_string()),
    };

    // 此時，迭代器 `parts` 中剩下的就是所有的修飾鍵
    for modifier in parts {
        match modifier {
            "ctrl" => modifiers.insert(KeyModifiers::CONTROL),
            "alt" => modifiers.insert(KeyModifiers::ALT),
            "shift" => modifiers.insert(KeyModifiers::SHIFT),
            "" => {
                return Err("Empty modifier found. Check for double '+' like 'ctrl++a'".to_string());
            }
            _ => return Err(format!("Unknown modifier: '{}'", modifier)),
        }
    }

    let code = match key_part {
        "enter" => KeyCode::Enter,
        "backspace" => KeyCode::Backspace,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "esc" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        key if key.starts_with('f') && key.len() > 1 => {
            if let Ok(n) = key[1..].parse::<u8>()
                && (1..=12).contains(&n)
            {
                KeyCode::F(n)
            } else {
                return Err(format!("Invalid F key: '{}'", key));
            }
        }
        key if key.chars().count() == 1 => KeyCode::Char(key.chars().next().unwrap()),
        _ => return Err(format!("Unknown key code: '{}'", key_part)),
    };

    Ok(InputEvent { code, modifiers })
}

pub fn merge_keymap(
    base: HashMap<InputEvent, Command>,
    keymap: Vec<FastKeyMapProtocol>,
) -> HashMap<InputEvent, Command> {
    let mut final_keymap = base;
    final_keymap.reserve(final_keymap.len() + keymap.len());

    for FastKeyMapProtocol {
        input: input_str,
        command: command_str,
    } in keymap
    {
        let input_event = match parse_input_event(&input_str.to_ascii_lowercase()) {
            Ok(event) => event,
            Err(e) => {
                eprintln!("Warning: Failed to parse keybind '{}': {}", input_str, e);
                continue;
            }
        };

        let command = match Command::from_str(&command_str) {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse command for keybind '{}': {}",
                    input_str, e
                );
                continue;
            }
        };

        final_keymap.insert(input_event, command);
    }

    final_keymap
}
