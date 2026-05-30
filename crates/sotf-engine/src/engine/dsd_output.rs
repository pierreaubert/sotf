use serde::{Deserialize, Serialize};
use sotf_types::{DsdOutputMode, DsdOutputStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DsdOutputBackend {
    Disabled,
    PcmDecoder,
    DopBitstream,
    NativeBitstream,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DsdOutputPlan {
    pub requested: DsdOutputMode,
    pub backend: DsdOutputBackend,
    pub status: DsdOutputStatus,
    pub reason: Option<String>,
}

impl DsdOutputPlan {
    fn new(
        requested: DsdOutputMode,
        backend: DsdOutputBackend,
        status: DsdOutputStatus,
        reason: Option<String>,
    ) -> Self {
        Self {
            requested,
            backend,
            status,
            reason,
        }
    }
}

pub fn plan_dsd_output(requested: DsdOutputMode) -> DsdOutputPlan {
    match requested {
        DsdOutputMode::Disabled => DsdOutputPlan::new(
            requested,
            DsdOutputBackend::Disabled,
            DsdOutputStatus::Disabled,
            None,
        ),
        DsdOutputMode::PcmDecode => DsdOutputPlan::new(
            requested,
            DsdOutputBackend::PcmDecoder,
            DsdOutputStatus::PcmDecodeAvailable,
            Some("DSF and uncompressed DFF containers decode to PCM in this build".to_string()),
        ),
        DsdOutputMode::DopPreferred => DsdOutputPlan::new(
            requested,
            DsdOutputBackend::PcmDecoder,
            DsdOutputStatus::DopFallbackPcm,
            Some(
                "DoP output is unavailable in this build; supported DSD containers will decode to PCM"
                    .to_string(),
            ),
        ),
        DsdOutputMode::DopRequired => DsdOutputPlan::new(
            requested,
            DsdOutputBackend::DopBitstream,
            DsdOutputStatus::DopUnavailable,
            Some(
                "DoP output is required, but the current playback backend cannot carry bit-perfect DoP frames"
                    .to_string(),
            ),
        ),
        DsdOutputMode::NativePreferred => DsdOutputPlan::new(
            requested,
            DsdOutputBackend::PcmDecoder,
            DsdOutputStatus::NativeFallbackPcm,
            Some(
                "Native DSD output is unavailable in this build; supported DSD containers will decode to PCM"
                    .to_string(),
            ),
        ),
        DsdOutputMode::NativeRequired => DsdOutputPlan::new(
            requested,
            DsdOutputBackend::NativeBitstream,
            DsdOutputStatus::NativeUnavailable,
            Some(
                "Native DSD output is required, but the current playback backend cannot carry native DSD frames"
                    .to_string(),
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_decode_reports_available_pcm_decoder() {
        let plan = plan_dsd_output(DsdOutputMode::PcmDecode);

        assert_eq!(plan.backend, DsdOutputBackend::PcmDecoder);
        assert_eq!(plan.status, DsdOutputStatus::PcmDecodeAvailable);
        assert!(plan.reason.as_deref().unwrap().contains("DSF"));
    }

    #[test]
    fn preferred_bitstream_modes_fallback_to_pcm_decode() {
        let dop = plan_dsd_output(DsdOutputMode::DopPreferred);
        let native = plan_dsd_output(DsdOutputMode::NativePreferred);

        assert_eq!(dop.backend, DsdOutputBackend::PcmDecoder);
        assert_eq!(dop.status, DsdOutputStatus::DopFallbackPcm);
        assert_eq!(native.backend, DsdOutputBackend::PcmDecoder);
        assert_eq!(native.status, DsdOutputStatus::NativeFallbackPcm);
    }

    #[test]
    fn required_bitstream_modes_report_unavailable_with_reasons() {
        let dop = plan_dsd_output(DsdOutputMode::DopRequired);
        let native = plan_dsd_output(DsdOutputMode::NativeRequired);

        assert_eq!(dop.backend, DsdOutputBackend::DopBitstream);
        assert_eq!(dop.status, DsdOutputStatus::DopUnavailable);
        assert!(
            dop.reason
                .as_deref()
                .unwrap()
                .contains("DoP output is required")
        );
        assert_eq!(native.backend, DsdOutputBackend::NativeBitstream);
        assert_eq!(native.status, DsdOutputStatus::NativeUnavailable);
        assert!(
            native
                .reason
                .as_deref()
                .unwrap()
                .contains("Native DSD output is required")
        );
    }
}
