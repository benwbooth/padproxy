use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct OutputDeviceInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub supported: bool,
    pub note: &'static str,
}

pub const OUTPUT_DEVICES: &[OutputDeviceInfo] = &[
    OutputDeviceInfo {
        id: "xbox360",
        label: "Xbox 360",
        supported: true,
        note: "Implemented through Linux uinput.",
    },
    OutputDeviceInfo {
        id: "xboxone",
        label: "Xbox One / Series",
        supported: false,
        note: "Planned virtual output.",
    },
    OutputDeviceInfo {
        id: "ds4",
        label: "DualShock 4",
        supported: false,
        note: "Planned virtual output.",
    },
    OutputDeviceInfo {
        id: "dualsense",
        label: "DualSense",
        supported: false,
        note: "Planned virtual output.",
    },
    OutputDeviceInfo {
        id: "switchpro",
        label: "Switch Pro",
        supported: false,
        note: "Planned virtual output.",
    },
];

pub fn output_devices() -> &'static [OutputDeviceInfo] {
    OUTPUT_DEVICES
}

pub fn normalize_output_type(value: &str) -> String {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "xbox360" | "x360" | "xinput" => "xbox360".to_string(),
        "xboxone" | "xboxseries" | "xboxseriesxs" | "seriesxs" => "xboxone".to_string(),
        "dualshock4" | "dualschock" | "ds4" => "ds4".to_string(),
        "dualsense" | "dualsense5" | "ps5" => "dualsense".to_string(),
        "switchpro" | "switch" | "nintendoswitchpro" => "switchpro".to_string(),
        _ => value.trim().to_ascii_lowercase(),
    }
}

pub fn output_device(id: &str) -> Option<&'static OutputDeviceInfo> {
    let normalized = normalize_output_type(id);
    output_devices()
        .iter()
        .find(|output| output.id == normalized)
}

pub fn output_type_supported(id: &str) -> bool {
    output_device(id)
        .map(|output| output.supported)
        .unwrap_or(false)
}

pub fn supported_output_ids() -> Vec<&'static str> {
    output_devices()
        .iter()
        .filter(|output| output.supported)
        .map(|output| output.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{normalize_output_type, output_device, output_type_supported};

    #[test]
    fn normalizes_common_output_aliases() {
        assert_eq!(normalize_output_type("Xbox 360"), "xbox360");
        assert_eq!(normalize_output_type("xinput"), "xbox360");
        assert_eq!(normalize_output_type("DualShock 4"), "ds4");
        assert_eq!(normalize_output_type("Switch Pro"), "switchpro");
    }

    #[test]
    fn reports_supported_and_planned_outputs() {
        assert!(output_type_supported("xbox360"));
        assert!(!output_type_supported("dualsense"));
        assert_eq!(
            output_device("xboxone").map(|output| output.label),
            Some("Xbox One / Series")
        );
    }
}
