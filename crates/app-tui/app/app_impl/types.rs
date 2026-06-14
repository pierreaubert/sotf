/// Result of a background federation source scan.
#[derive(Debug)]
pub struct FederationScanResult {
    pub source_id: String,
    pub albums: usize,
    pub tracks: usize,
    pub error: Option<String>,
}
