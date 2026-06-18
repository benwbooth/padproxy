use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct OutputDeviceInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub supported: bool,
    pub note: &'static str,
    /// Name advertised by the virtual uinput device.
    pub virtual_name: &'static str,
    /// USB vendor id reported by the virtual device so games and Steam Input
    /// recognize it as the emulated controller type.
    pub vendor: u16,
    /// USB product id reported by the virtual device.
    pub product: u16,
    /// Device version reported by the virtual device.
    pub version: u16,
}

pub const OUTPUT_DEVICES: &[OutputDeviceInfo] = &[
    OutputDeviceInfo {
        id: "xbox360",
        label: "Xbox 360",
        supported: true,
        note: "Implemented through Linux uinput.",
        virtual_name: "PadProxy Virtual Xbox 360 Controller",
        vendor: 0x045e,
        product: 0x028e,
        version: 0x0114,
    },
    OutputDeviceInfo {
        id: "xboxone",
        label: "Xbox One / Series",
        supported: true,
        note: "Implemented through Linux uinput.",
        virtual_name: "PadProxy Virtual Xbox One Controller",
        vendor: 0x045e,
        product: 0x02ea,
        version: 0x0408,
    },
    OutputDeviceInfo {
        id: "ds4",
        label: "DualShock 4",
        supported: true,
        note: "Implemented through Linux uinput.",
        virtual_name: "PadProxy Virtual DualShock 4 Controller",
        vendor: 0x054c,
        product: 0x09cc,
        version: 0x8111,
    },
    OutputDeviceInfo {
        id: "dualsense",
        label: "DualSense",
        supported: true,
        note: "Implemented through Linux uinput.",
        virtual_name: "PadProxy Virtual DualSense Controller",
        vendor: 0x054c,
        product: 0x0ce6,
        version: 0x8111,
    },
    OutputDeviceInfo {
        id: "switchpro",
        label: "Switch Pro",
        supported: true,
        note: "Implemented through Linux uinput.",
        virtual_name: "PadProxy Virtual Switch Pro Controller",
        vendor: 0x057e,
        product: 0x2009,
        version: 0x0001,
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
    use super::{normalize_output_type, output_device, output_devices, output_type_supported};

    #[test]
    fn normalizes_common_output_aliases() {
        assert_eq!(normalize_output_type("Xbox 360"), "xbox360");
        assert_eq!(normalize_output_type("xinput"), "xbox360");
        assert_eq!(normalize_output_type("DualShock 4"), "ds4");
        assert_eq!(normalize_output_type("Switch Pro"), "switchpro");
    }

    #[test]
    fn reports_supported_outputs() {
        assert!(output_type_supported("xbox360"));
        assert!(output_type_supported("dualsense"));
        assert_eq!(
            output_device("xboxone").map(|output| output.label),
            Some("Xbox One / Series")
        );
    }

    #[test]
    fn carries_distinct_usb_identities_per_output() {
        let ds4 = output_device("ds4").expect("ds4 descriptor");
        assert_eq!(ds4.vendor, 0x054c);
        assert_eq!(ds4.product, 0x09cc);
        assert_eq!(ds4.virtual_name, "PadProxy Virtual DualShock 4 Controller");

        let switchpro = output_device("switchpro").expect("switchpro descriptor");
        assert_eq!(switchpro.vendor, 0x057e);

        // Every supported output advertises a unique vendor/product pair.
        let mut identities: Vec<(u16, u16)> = output_devices()
            .iter()
            .map(|output| (output.vendor, output.product))
            .collect();
        identities.sort_unstable();
        let unique = identities.len();
        identities.dedup();
        assert_eq!(identities.len(), unique, "duplicate USB identity");
    }
}
