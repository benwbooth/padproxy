use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub phys: String,
    pub uniq: String,
    pub bus: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
    pub capabilities: Vec<String>,
}

impl DeviceInfo {
    pub fn hardware_id(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor, self.product)
    }
}
