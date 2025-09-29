use std::{collections::HashMap, str::FromStr};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use maplit::hashmap;
use shared::bridge::FastKeyMapProtocol;

pub fn default_keymap() -> std::collections::HashMap<InputEvent, Command> {
    hashmap! {
        InputEvent { code: KeyCode::Enter,      modifiers: KeyModifiers::NONE  } => Command::InputEnter,
        InputEvent { code: KeyCode::Enter,      modifiers: KeyModifiers::SHIFT } => Command::InputEnter,
        InputEvent { code: KeyCode::Backspace,  modifiers: KeyModifiers::NONE  } => Command::DeleteLeft,
        InputEvent { code: KeyCode::Delete,     modifiers: KeyModifiers::NONE  } => Command::DeleteRight,        
        InputEvent { code: KeyCode::Backspace,  modifiers: KeyModifiers::SHIFT } => Command::DeleteLeft,
        InputEvent { code: KeyCode::Delete,     modifiers: KeyModifiers::SHIFT } => Command::DeleteRight,
        InputEvent { code: KeyCode::Backspace,  modifiers: KeyModifiers::CONTROL } => Command::DeleteWordLeft,
        InputEvent { code: KeyCode::Delete,     modifiers: KeyModifiers::CONTROL } => Command::DeleteWordRight,
        InputEvent { code: KeyCode::Backspace,  modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT } => Command::DeleteWordLeft,        
        InputEvent { code: KeyCode::Delete,     modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT } => Command::DeleteWordRight,
        InputEvent { code: KeyCode::Char('w'),  modifiers: KeyModifiers::CONTROL } => Command::DeleteWordLeft,
        InputEvent { code: KeyCode::Char('d'),     modifiers: KeyModifiers::ALT     } => Command::DeleteWordRight,
        
        InputEvent { code: KeyCode::Up,         modifiers: KeyModifiers::NONE } => Command::CursorUp,
        InputEvent { code: KeyCode::Down,       modifiers: KeyModifiers::NONE } => Command::CursorDown,
        InputEvent { code: KeyCode::Left,       modifiers: KeyModifiers::NONE } => Command::CursorLeft,
        InputEvent { code: KeyCode::Right,      modifiers: KeyModifiers::NONE } => Command::CursorRight,
        InputEvent { code: KeyCode::Up,         modifiers: KeyModifiers::SHIFT } => Command::CursorUp,
        InputEvent { code: KeyCode::Down,       modifiers: KeyModifiers::SHIFT } => Command::CursorDown,
        InputEvent { code: KeyCode::Left,       modifiers: KeyModifiers::SHIFT } => Command::CursorLeft,
        InputEvent { code: KeyCode::Right,      modifiers: KeyModifiers::SHIFT } => Command::CursorRight,
        InputEvent { code: KeyCode::Left,       modifiers: KeyModifiers::CONTROL } => Command::CursorWordLeft,
        InputEvent { code: KeyCode::Right,      modifiers: KeyModifiers::CONTROL } => Command::CursorWordRight,
        InputEvent { code: KeyCode::Left,       modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT } => Command::CursorWordLeft,
        InputEvent { code: KeyCode::Right,      modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT } => Command::CursorWordRight,

        InputEvent { code: KeyCode::Home,       modifiers: KeyModifiers::NONE } => Command::CursorHome,
        InputEvent { code: KeyCode::End,        modifiers: KeyModifiers::NONE } => Command::CursorEnd,
        InputEvent { code: KeyCode::PageUp,     modifiers: KeyModifiers::NONE } => Command::CursorPageUp,
        InputEvent { code: KeyCode::PageDown,   modifiers: KeyModifiers::NONE } => Command::CursorPageDown,

        InputEvent { code: KeyCode::Char('a'),  modifiers: KeyModifiers::CONTROL } => Command::SelectAll,
        InputEvent { code: KeyCode::Char('l'),  modifiers: KeyModifiers::CONTROL } => Command::SelectLine,

        InputEvent { code: KeyCode::Char('c'),  modifiers: KeyModifiers::CONTROL } => Command::TextCopy,
        InputEvent { code: KeyCode::Char('x'),  modifiers: KeyModifiers::CONTROL } => Command::TextCut,
        InputEvent { code: KeyCode::Char('v'),  modifiers: KeyModifiers::CONTROL } => Command::TextPaste,

        InputEvent { code: KeyCode::Esc,        modifiers: KeyModifiers::NONE } => Command::Exit,
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
            "deletewordleft" => Ok(Command::DeleteLeft),
            "deletewordright" => Ok(Command::DeleteRight),
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
            "copy" => Ok(Command::TextCopy),
            "cut" => Ok(Command::TextCut),
            "paste" => Ok(Command::TextPaste),
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
