use evdev::{AbsoluteAxisCode, EventType, InputEvent, KeyCode, RelativeAxisCode};
use serde::Serialize;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum EventKind {
    Key,
    Absolute,
    Relative,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct EventCode {
    pub kind: EventKind,
    pub code: u16,
}

impl EventCode {
    pub fn event_type(self) -> EventType {
        match self.kind {
            EventKind::Key => EventType::KEY,
            EventKind::Absolute => EventType::ABSOLUTE,
            EventKind::Relative => EventType::RELATIVE,
        }
    }

    pub fn name(self) -> String {
        match (self.kind, self.code) {
            (EventKind::Key, code) if code == KeyCode::BTN_SOUTH.0 => "btn:south".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_EAST.0 => "btn:east".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_NORTH.0 => "btn:north".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_WEST.0 => "btn:west".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_TL.0 => "btn:tl".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_TR.0 => "btn:tr".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_TL2.0 => "btn:tl2".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_TR2.0 => "btn:tr2".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_SELECT.0 => "btn:select".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_START.0 => "btn:start".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_MODE.0 => "btn:mode".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_THUMBL.0 => "btn:thumbl".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_THUMBR.0 => "btn:thumbr".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_LEFT.0 => "mouse:left".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_RIGHT.0 => "mouse:right".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_MIDDLE.0 => "mouse:middle".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_SIDE.0 => "mouse:side".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_EXTRA.0 => "mouse:extra".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_FORWARD.0 => "mouse:forward".to_string(),
            (EventKind::Key, code) if code == KeyCode::BTN_BACK.0 => "mouse:back".to_string(),
            (EventKind::Key, code) => {
                keyboard_key_name(code).unwrap_or_else(|| format!("key:{code}"))
            }
            (EventKind::Absolute, code) if code == AbsoluteAxisCode::ABS_X.0 => "abs:x".to_string(),
            (EventKind::Absolute, code) if code == AbsoluteAxisCode::ABS_Y.0 => "abs:y".to_string(),
            (EventKind::Absolute, code) if code == AbsoluteAxisCode::ABS_RX.0 => {
                "abs:rx".to_string()
            }
            (EventKind::Absolute, code) if code == AbsoluteAxisCode::ABS_RY.0 => {
                "abs:ry".to_string()
            }
            (EventKind::Absolute, code) if code == AbsoluteAxisCode::ABS_Z.0 => "abs:z".to_string(),
            (EventKind::Absolute, code) if code == AbsoluteAxisCode::ABS_RZ.0 => {
                "abs:rz".to_string()
            }
            (EventKind::Absolute, code) if code == AbsoluteAxisCode::ABS_HAT0X.0 => {
                "abs:hat0x".to_string()
            }
            (EventKind::Absolute, code) if code == AbsoluteAxisCode::ABS_HAT0Y.0 => {
                "abs:hat0y".to_string()
            }
            (EventKind::Absolute, code) => format!("abs:{code}"),
            (EventKind::Relative, code) if code == RelativeAxisCode::REL_X.0 => "rel:x".to_string(),
            (EventKind::Relative, code) if code == RelativeAxisCode::REL_Y.0 => "rel:y".to_string(),
            (EventKind::Relative, code) if code == RelativeAxisCode::REL_WHEEL.0 => {
                "rel:wheel".to_string()
            }
            (EventKind::Relative, code) if code == RelativeAxisCode::REL_HWHEEL.0 => {
                "rel:hwheel".to_string()
            }
            (EventKind::Relative, code) => format!("rel:{code}"),
        }
    }
}

pub fn parse_event_code(value: &str) -> Option<EventCode> {
    let normalized = value.trim().to_ascii_lowercase().replace([' ', '-'], "_");

    let event = match normalized.as_str() {
        "btn:south" | "btn:a" | "button:south" | "button:a" => key(KeyCode::BTN_SOUTH),
        "btn:east" | "btn:b" | "button:east" | "button:b" => key(KeyCode::BTN_EAST),
        "btn:north" | "btn:y" | "button:north" | "button:y" => key(KeyCode::BTN_NORTH),
        "btn:west" | "btn:x" | "button:west" | "button:x" => key(KeyCode::BTN_WEST),
        "btn:tl" | "btn:l1" => key(KeyCode::BTN_TL),
        "btn:tr" | "btn:r1" => key(KeyCode::BTN_TR),
        "btn:tl2" | "btn:l2" => key(KeyCode::BTN_TL2),
        "btn:tr2" | "btn:r2" => key(KeyCode::BTN_TR2),
        "btn:select" | "btn:back" => key(KeyCode::BTN_SELECT),
        "btn:start" => key(KeyCode::BTN_START),
        "btn:mode" => key(KeyCode::BTN_MODE),
        "btn:thumbl" | "btn:l3" => key(KeyCode::BTN_THUMBL),
        "btn:thumbr" | "btn:r3" => key(KeyCode::BTN_THUMBR),
        "mouse:left" | "mouse:primary" | "btn:left" => key(KeyCode::BTN_LEFT),
        "mouse:right" | "mouse:secondary" | "btn:right" => key(KeyCode::BTN_RIGHT),
        "mouse:middle" | "mouse:wheel_button" | "btn:middle" => key(KeyCode::BTN_MIDDLE),
        "mouse:side" | "mouse:button4" => key(KeyCode::BTN_SIDE),
        "mouse:extra" | "mouse:button5" => key(KeyCode::BTN_EXTRA),
        "mouse:forward" => key(KeyCode::BTN_FORWARD),
        "mouse:back" => key(KeyCode::BTN_BACK),
        "abs:x" | "axis:x" => abs(AbsoluteAxisCode::ABS_X),
        "abs:y" | "axis:y" => abs(AbsoluteAxisCode::ABS_Y),
        "abs:rx" | "axis:rx" => abs(AbsoluteAxisCode::ABS_RX),
        "abs:ry" | "axis:ry" => abs(AbsoluteAxisCode::ABS_RY),
        "abs:z" | "axis:z" => abs(AbsoluteAxisCode::ABS_Z),
        "abs:rz" | "axis:rz" => abs(AbsoluteAxisCode::ABS_RZ),
        "abs:hat0x" | "axis:hat0x" => abs(AbsoluteAxisCode::ABS_HAT0X),
        "abs:hat0y" | "axis:hat0y" => abs(AbsoluteAxisCode::ABS_HAT0Y),
        "rel:x" | "mouse:x" => rel(RelativeAxisCode::REL_X),
        "rel:y" | "mouse:y" => rel(RelativeAxisCode::REL_Y),
        "rel:wheel" | "mouse:wheel" | "wheel:vertical" => rel(RelativeAxisCode::REL_WHEEL),
        "rel:hwheel" | "mouse:hwheel" | "wheel:horizontal" => rel(RelativeAxisCode::REL_HWHEEL),
        _ => parse_keyboard_key(&normalized)?,
    };

    Some(event)
}

pub fn virtual_xbox_supports(event: EventCode) -> bool {
    match event.kind {
        EventKind::Key => [
            KeyCode::BTN_SOUTH.0,
            KeyCode::BTN_EAST.0,
            KeyCode::BTN_NORTH.0,
            KeyCode::BTN_WEST.0,
            KeyCode::BTN_TL.0,
            KeyCode::BTN_TR.0,
            KeyCode::BTN_TL2.0,
            KeyCode::BTN_TR2.0,
            KeyCode::BTN_SELECT.0,
            KeyCode::BTN_START.0,
            KeyCode::BTN_MODE.0,
            KeyCode::BTN_THUMBL.0,
            KeyCode::BTN_THUMBR.0,
        ]
        .contains(&event.code),
        EventKind::Absolute => [
            AbsoluteAxisCode::ABS_X.0,
            AbsoluteAxisCode::ABS_Y.0,
            AbsoluteAxisCode::ABS_RX.0,
            AbsoluteAxisCode::ABS_RY.0,
            AbsoluteAxisCode::ABS_Z.0,
            AbsoluteAxisCode::ABS_RZ.0,
            AbsoluteAxisCode::ABS_HAT0X.0,
            AbsoluteAxisCode::ABS_HAT0Y.0,
        ]
        .contains(&event.code),
        EventKind::Relative => false,
    }
}

pub fn virtual_keyboard_mouse_supports(event: EventCode) -> bool {
    match event.kind {
        EventKind::Key => is_keyboard_key(event) || is_mouse_button(event),
        EventKind::Relative => [
            RelativeAxisCode::REL_X.0,
            RelativeAxisCode::REL_Y.0,
            RelativeAxisCode::REL_WHEEL.0,
            RelativeAxisCode::REL_HWHEEL.0,
        ]
        .contains(&event.code),
        EventKind::Absolute => false,
    }
}

pub fn virtual_output_supports(event: EventCode) -> bool {
    virtual_xbox_supports(event) || virtual_keyboard_mouse_supports(event)
}

pub fn is_mouse_button(event: EventCode) -> bool {
    event.kind == EventKind::Key
        && [
            KeyCode::BTN_LEFT.0,
            KeyCode::BTN_RIGHT.0,
            KeyCode::BTN_MIDDLE.0,
            KeyCode::BTN_SIDE.0,
            KeyCode::BTN_EXTRA.0,
            KeyCode::BTN_FORWARD.0,
            KeyCode::BTN_BACK.0,
        ]
        .contains(&event.code)
}

pub fn is_keyboard_key(event: EventCode) -> bool {
    event.kind == EventKind::Key && !virtual_xbox_supports(event) && !is_mouse_button(event)
}

pub fn event_from_input(event: InputEvent) -> Option<EventCode> {
    if event.event_type() == EventType::KEY {
        Some(EventCode {
            kind: EventKind::Key,
            code: event.code(),
        })
    } else if event.event_type() == EventType::ABSOLUTE {
        Some(EventCode {
            kind: EventKind::Absolute,
            code: event.code(),
        })
    } else if event.event_type() == EventType::RELATIVE {
        Some(EventCode {
            kind: EventKind::Relative,
            code: event.code(),
        })
    } else {
        None
    }
}

pub fn capture_event_code(event: InputEvent) -> Option<EventCode> {
    let code = event_from_input(event)?;
    match code.kind {
        EventKind::Key if event.value() != 0 => Some(code),
        EventKind::Absolute if event.value() != 0 => Some(code),
        EventKind::Relative if event.value() != 0 => Some(code),
        _ => None,
    }
}

fn key(code: KeyCode) -> EventCode {
    EventCode {
        kind: EventKind::Key,
        code: code.0,
    }
}

fn abs(code: AbsoluteAxisCode) -> EventCode {
    EventCode {
        kind: EventKind::Absolute,
        code: code.0,
    }
}

fn rel(code: RelativeAxisCode) -> EventCode {
    EventCode {
        kind: EventKind::Relative,
        code: code.0,
    }
}

fn parse_keyboard_key(normalized: &str) -> Option<EventCode> {
    let suffix = normalized
        .strip_prefix("key:")
        .or_else(|| normalized.strip_prefix("keyboard:"))?;
    let code = keyboard_key_from_suffix(suffix)?;
    Some(key(code))
}

fn keyboard_key_from_suffix(suffix: &str) -> Option<KeyCode> {
    let alias = match suffix {
        "esc" => "escape",
        "return" => "enter",
        "ctrl" => "leftctrl",
        "control" => "leftctrl",
        "shift" => "leftshift",
        "alt" => "leftalt",
        "super" | "meta" | "win" => "leftmeta",
        other => other,
    };
    let key_name = if alias.len() == 1 && alias.chars().all(|character| character.is_ascii_digit())
    {
        format!("KEY_{alias}")
    } else {
        format!("KEY_{}", alias.to_ascii_uppercase())
    };
    KeyCode::from_str(&key_name).ok()
}

fn keyboard_key_name(code: u16) -> Option<String> {
    let name = match KeyCode(code) {
        KeyCode::KEY_SPACE => "space",
        KeyCode::KEY_ENTER => "enter",
        KeyCode::KEY_ESC => "escape",
        KeyCode::KEY_TAB => "tab",
        KeyCode::KEY_BACKSPACE => "backspace",
        KeyCode::KEY_LEFTCTRL => "leftctrl",
        KeyCode::KEY_RIGHTCTRL => "rightctrl",
        KeyCode::KEY_LEFTSHIFT => "leftshift",
        KeyCode::KEY_RIGHTSHIFT => "rightshift",
        KeyCode::KEY_LEFTALT => "leftalt",
        KeyCode::KEY_RIGHTALT => "rightalt",
        KeyCode::KEY_LEFTMETA => "leftmeta",
        KeyCode::KEY_RIGHTMETA => "rightmeta",
        KeyCode::KEY_UP => "up",
        KeyCode::KEY_DOWN => "down",
        KeyCode::KEY_LEFT => "left",
        KeyCode::KEY_RIGHT => "right",
        KeyCode::KEY_HOME => "home",
        KeyCode::KEY_END => "end",
        KeyCode::KEY_PAGEUP => "pageup",
        KeyCode::KEY_PAGEDOWN => "pagedown",
        KeyCode::KEY_DELETE => "delete",
        KeyCode::KEY_INSERT => "insert",
        KeyCode::KEY_MINUS => "minus",
        KeyCode::KEY_EQUAL => "equal",
        KeyCode::KEY_LEFTBRACE => "leftbrace",
        KeyCode::KEY_RIGHTBRACE => "rightbrace",
        KeyCode::KEY_BACKSLASH => "backslash",
        KeyCode::KEY_SEMICOLON => "semicolon",
        KeyCode::KEY_APOSTROPHE => "apostrophe",
        KeyCode::KEY_GRAVE => "grave",
        KeyCode::KEY_COMMA => "comma",
        KeyCode::KEY_DOT => "dot",
        KeyCode::KEY_SLASH => "slash",
        KeyCode::KEY_1 => "1",
        KeyCode::KEY_2 => "2",
        KeyCode::KEY_3 => "3",
        KeyCode::KEY_4 => "4",
        KeyCode::KEY_5 => "5",
        KeyCode::KEY_6 => "6",
        KeyCode::KEY_7 => "7",
        KeyCode::KEY_8 => "8",
        KeyCode::KEY_9 => "9",
        KeyCode::KEY_0 => "0",
        KeyCode::KEY_A => "a",
        KeyCode::KEY_B => "b",
        KeyCode::KEY_C => "c",
        KeyCode::KEY_D => "d",
        KeyCode::KEY_E => "e",
        KeyCode::KEY_F => "f",
        KeyCode::KEY_G => "g",
        KeyCode::KEY_H => "h",
        KeyCode::KEY_I => "i",
        KeyCode::KEY_J => "j",
        KeyCode::KEY_K => "k",
        KeyCode::KEY_L => "l",
        KeyCode::KEY_M => "m",
        KeyCode::KEY_N => "n",
        KeyCode::KEY_O => "o",
        KeyCode::KEY_P => "p",
        KeyCode::KEY_Q => "q",
        KeyCode::KEY_R => "r",
        KeyCode::KEY_S => "s",
        KeyCode::KEY_T => "t",
        KeyCode::KEY_U => "u",
        KeyCode::KEY_V => "v",
        KeyCode::KEY_W => "w",
        KeyCode::KEY_X => "x",
        KeyCode::KEY_Y => "y",
        KeyCode::KEY_Z => "z",
        KeyCode::KEY_F1 => "f1",
        KeyCode::KEY_F2 => "f2",
        KeyCode::KEY_F3 => "f3",
        KeyCode::KEY_F4 => "f4",
        KeyCode::KEY_F5 => "f5",
        KeyCode::KEY_F6 => "f6",
        KeyCode::KEY_F7 => "f7",
        KeyCode::KEY_F8 => "f8",
        KeyCode::KEY_F9 => "f9",
        KeyCode::KEY_F10 => "f10",
        KeyCode::KEY_F11 => "f11",
        KeyCode::KEY_F12 => "f12",
        _ => return None,
    };
    Some(format!("key:{name}"))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_event_code, virtual_keyboard_mouse_supports, virtual_output_supports,
        virtual_xbox_supports, EventKind,
    };

    #[test]
    fn parses_keyboard_mouse_and_relative_events() {
        let key = parse_event_code("key:space").unwrap();
        assert_eq!(key.kind, EventKind::Key);
        assert_eq!(key.name(), "key:space");
        assert!(virtual_keyboard_mouse_supports(key));
        assert!(virtual_output_supports(key));
        assert!(!virtual_xbox_supports(key));

        let letter = parse_event_code("keyboard:a").unwrap();
        assert_eq!(letter.name(), "key:a");

        let digit = parse_event_code("key:1").unwrap();
        assert_eq!(digit.name(), "key:1");

        let mouse = parse_event_code("mouse:left").unwrap();
        assert_eq!(mouse.kind, EventKind::Key);
        assert_eq!(mouse.name(), "mouse:left");
        assert!(virtual_keyboard_mouse_supports(mouse));

        let relative = parse_event_code("rel:x").unwrap();
        assert_eq!(relative.kind, EventKind::Relative);
        assert_eq!(relative.name(), "rel:x");
        assert!(virtual_keyboard_mouse_supports(relative));
    }
}
