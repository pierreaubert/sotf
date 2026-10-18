//! Hosts the shared AutoEQ report without duplicating dashboard rendering.
//!
//! Reports are served from an ephemeral loopback origin so embedded browsers
//! can load the self-contained WASM assets offline. The server owns only one
//! immutable document, never exposes arbitrary filesystem paths, and stops
//! when its owner is dropped. Generation delegates to AutoEQ's bundled producer.

// Rust guideline compliant 2026-02-21

mod prepare;
mod producer;
mod server;

#[doc(inline)]
pub use autoeq_report_wasm::capture_quality;
#[doc(inline)]
pub use producer::{GeneratedReport, generate_report, generate_report_with_quality};
#[doc(inline)]
pub use server::ReportServer;

use serde::{Deserialize, Serialize};

/// Version of messages exchanged with the embedded report.
pub const HOST_PROTOCOL_VERSION: u32 = 1;

/// A selection originating from the shared viewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSelection {
    /// Logical excitation source identity.
    pub source_id: String,
    /// Optional microphone identity.
    pub microphone_id: Option<String>,
    /// Optional immutable take identity.
    pub take_id: Option<String>,
}

/// Requests from a report bound to one immutable result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ReportAction {
    /// Notify the application of a viewer selection.
    Select { selection: ReportSelection },
    /// Request a retake; the application validates eligibility before recording.
    Retake { selection: ReportSelection },
    /// Request saving the current self-contained document.
    Export,
    /// Announce viewer readiness and renderer capability.
    Ready { renderer: String },
}

/// A versioned viewer message with result identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportMessage {
    /// Must equal [`HOST_PROTOCOL_VERSION`].
    pub version: u32,
    /// SHA-256 identity supplied by the host for this report.
    pub report_id: String,
    /// Requested interaction.
    #[serde(flatten)]
    pub action: ReportAction,
}

/// Parse a bounded message belonging to the currently displayed result.
///
/// # Errors
/// Returns an error for oversized, malformed, unsupported, or stale messages.
pub fn parse_message(message: &str, report_id: &str) -> anyhow::Result<ReportAction> {
    // IPC should contain identities and actions, never report arrays or audio.
    anyhow::ensure!(message.len() <= 8192, "Report message exceeds 8 KiB");
    let message: ReportMessage = serde_json::from_str(message)?;
    anyhow::ensure!(
        message.version == HOST_PROTOCOL_VERSION,
        "Unsupported report host protocol"
    );
    anyhow::ensure!(
        message.report_id == report_id,
        "Report interaction belongs to a stale result"
    );
    Ok(message.action)
}

/// Build the viewer bridge without recomputing or rendering report content.
pub fn initialization_script(report_id: &str, dark: bool) -> String {
    let identity = serde_json::to_string(report_id).expect("A string is always JSON serializable");
    let theme = if dark { "dark" } else { "light" };
    format!(
        r#"(() => {{
      const reportId = {identity};
      window.sotfReportHost = Object.freeze({{
        version: 1, reportId,
        send(action) {{ window.ipc.postMessage(JSON.stringify({{...action, version: 1, report_id: reportId}})); }}
      }});
      window.addEventListener('DOMContentLoaded', () => {{
        document.documentElement.style.colorScheme = '{theme}';
        window.dispatchEvent(new CustomEvent('sotf-report-host-ready', {{detail: {{version: 1, reportId, theme: '{theme}'}}}}));
      }});
    }})();"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_report_cannot_request_a_retake() {
        let message = r#"{"version":1,"report_id":"old","action":"retake","selection":{"source_id":"L","microphone_id":"mic-1","take_id":"take-1"}}"#;
        assert!(parse_message(message, "new").is_err());
        assert!(matches!(
            parse_message(message, "old").unwrap(),
            ReportAction::Retake { .. }
        ));
    }

    #[test]
    fn bridge_encodes_identity_as_data() {
        let script = initialization_script("\";window.injected=true;\"", false);
        assert!(script.contains("const reportId = \"\\\";window.injected=true;\\\"\""));
    }
}
