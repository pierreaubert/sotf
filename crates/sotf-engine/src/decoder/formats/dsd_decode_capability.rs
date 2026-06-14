/// User-facing decode capability for DSD/SACD containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DsdDecodeCapability {
    /// The format is not a DSD/SACD container.
    NotDsd,
    /// The engine can decode the DSD container to PCM with `DsdOutputMode::PcmDecode`.
    PcmDecodeAvailable,
    /// The engine can decode this DSD container to PCM only for uncompressed streams.
    PcmDecodeAvailableUncompressedOnly,
    /// The container is recognized but this build cannot decode it.
    UnsupportedContainer,
}

impl DsdDecodeCapability {
    pub fn description(self) -> &'static str {
        match self {
            DsdDecodeCapability::NotDsd => "not a DSD/SACD container",
            DsdDecodeCapability::PcmDecodeAvailable => "PCM decode available",
            DsdDecodeCapability::PcmDecodeAvailableUncompressedOnly => {
                "PCM decode available for uncompressed DSD; compressed DST is unsupported"
            }
            DsdDecodeCapability::UnsupportedContainer => "recognized but unsupported",
        }
    }
}
