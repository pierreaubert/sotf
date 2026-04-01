// ============================================================================
// UPnP Device Description
// ============================================================================

use uuid::Uuid;

/// Type of DLNA device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlnaDeviceType {
    MediaRenderer,
    MediaServer,
}

impl DlnaDeviceType {
    pub fn urn(&self) -> &str {
        match self {
            DlnaDeviceType::MediaRenderer => "urn:schemas-upnp-org:device:MediaRenderer:1",
            DlnaDeviceType::MediaServer => "urn:schemas-upnp-org:device:MediaServer:1",
        }
    }
}

/// UPnP device description for SSDP announcements.
#[derive(Debug, Clone)]
pub struct DlnaDevice {
    pub device_type: DlnaDeviceType,
    pub friendly_name: String,
    pub manufacturer: String,
    pub model_name: String,
    pub model_number: String,
    pub uuid: Uuid,
    /// HTTP port for serving device description + control
    pub http_port: u16,
}

impl DlnaDevice {
    pub fn new_renderer(name: &str, port: u16) -> Self {
        Self {
            device_type: DlnaDeviceType::MediaRenderer,
            friendly_name: name.to_string(),
            manufacturer: "SOTF".to_string(),
            model_name: "SOTF Audio Player".to_string(),
            model_number: env!("CARGO_PKG_VERSION").to_string(),
            uuid: Uuid::new_v4(),
            http_port: port,
        }
    }

    pub fn new_server(name: &str, port: u16) -> Self {
        Self {
            device_type: DlnaDeviceType::MediaServer,
            friendly_name: name.to_string(),
            manufacturer: "SOTF".to_string(),
            model_name: "SOTF Media Server".to_string(),
            model_number: env!("CARGO_PKG_VERSION").to_string(),
            uuid: Uuid::new_v4(),
            http_port: port,
        }
    }

    /// UPnP USN (Unique Service Name).
    pub fn usn(&self) -> String {
        format!("uuid:{}::{}", self.uuid, self.device_type.urn())
    }

    /// Generate the UPnP device description XML.
    pub fn description_xml(&self, base_url: &str) -> String {
        let services_xml = match self.device_type {
            DlnaDeviceType::MediaRenderer => renderer_services_xml(),
            DlnaDeviceType::MediaServer => server_services_xml(),
        };

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <URLBase>{base_url}</URLBase>
  <device>
    <deviceType>{device_type}</deviceType>
    <friendlyName>{name}</friendlyName>
    <manufacturer>{manufacturer}</manufacturer>
    <modelName>{model}</modelName>
    <modelNumber>{version}</modelNumber>
    <UDN>uuid:{uuid}</UDN>
{services}
  </device>
</root>"#,
            base_url = base_url,
            device_type = self.device_type.urn(),
            name = xml_escape(&self.friendly_name),
            manufacturer = xml_escape(&self.manufacturer),
            model = xml_escape(&self.model_name),
            version = xml_escape(&self.model_number),
            uuid = self.uuid,
            services = services_xml,
        )
    }
}

fn renderer_services_xml() -> String {
    r#"    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:AVTransport</serviceId>
        <controlURL>/AVTransport/control</controlURL>
        <eventSubURL>/AVTransport/event</eventSubURL>
        <SCPDURL>/AVTransport/scpd.xml</SCPDURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:RenderingControl</serviceId>
        <controlURL>/RenderingControl/control</controlURL>
        <eventSubURL>/RenderingControl/event</eventSubURL>
        <SCPDURL>/RenderingControl/scpd.xml</SCPDURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:ConnectionManager</serviceId>
        <controlURL>/ConnectionManager/control</controlURL>
        <eventSubURL>/ConnectionManager/event</eventSubURL>
        <SCPDURL>/ConnectionManager/scpd.xml</SCPDURL>
      </service>
    </serviceList>"#
        .to_string()
}

fn server_services_xml() -> String {
    r#"    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:ContentDirectory</serviceId>
        <controlURL>/ContentDirectory/control</controlURL>
        <eventSubURL>/ContentDirectory/event</eventSubURL>
        <SCPDURL>/ContentDirectory/scpd.xml</SCPDURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:ConnectionManager</serviceId>
        <controlURL>/ConnectionManager/control</controlURL>
        <eventSubURL>/ConnectionManager/event</eventSubURL>
        <SCPDURL>/ConnectionManager/scpd.xml</SCPDURL>
      </service>
    </serviceList>"#
        .to_string()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_description_xml() {
        let device = DlnaDevice::new_renderer("SOTF Test", 8200);
        let xml = device.description_xml("http://192.168.1.100:8200");
        assert!(xml.contains("MediaRenderer:1"));
        assert!(xml.contains("SOTF Test"));
        assert!(xml.contains("AVTransport"));
        assert!(xml.contains("RenderingControl"));
        assert!(xml.contains("ConnectionManager"));
        assert!(xml.contains(&device.uuid.to_string()));
    }

    #[test]
    fn test_server_description_xml() {
        let device = DlnaDevice::new_server("SOTF Library", 8201);
        let xml = device.description_xml("http://192.168.1.100:8201");
        assert!(xml.contains("MediaServer:1"));
        assert!(xml.contains("ContentDirectory"));
        assert!(xml.contains("ConnectionManager"));
    }

    #[test]
    fn test_usn() {
        let device = DlnaDevice::new_renderer("Test", 8200);
        let usn = device.usn();
        assert!(usn.starts_with("uuid:"));
        assert!(usn.contains("MediaRenderer:1"));
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a<b>c&d"), "a&lt;b&gt;c&amp;d");
    }
}
