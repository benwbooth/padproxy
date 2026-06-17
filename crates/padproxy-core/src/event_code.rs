use evdev::{AbsoluteAxisCode, EventType, InputEvent, KeyCode};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum EventKind {
    Key,
    Absolute,
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
            (EventKind::Key, code) => format!("key:{code}"),
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
        "abs:x" | "axis:x" => abs(AbsoluteAxisCode::ABS_X),
        "abs:y" | "axis:y" => abs(AbsoluteAxisCode::ABS_Y),
        "abs:rx" | "axis:rx" => abs(AbsoluteAxisCode::ABS_RX),
        "abs:ry" | "axis:ry" => abs(AbsoluteAxisCode::ABS_RY),
        "abs:z" | "axis:z" => abs(AbsoluteAxisCode::ABS_Z),
        "abs:rz" | "axis:rz" => abs(AbsoluteAxisCode::ABS_RZ),
        "abs:hat0x" | "axis:hat0x" => abs(AbsoluteAxisCode::ABS_HAT0X),
        "abs:hat0y" | "axis:hat0y" => abs(AbsoluteAxisCode::ABS_HAT0Y),
        _ => return None,
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
    }
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
    } else {
        None
    }
}

pub fn capture_event_code(event: InputEvent) -> Option<EventCode> {
    let code = event_from_input(event)?;
    match code.kind {
        EventKind::Key if event.value() != 0 => Some(code),
        EventKind::Absolute if event.value() != 0 => Some(code),
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
