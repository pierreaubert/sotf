use super::PluginDescriptor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Persistable metadata returned by a quarantined native-probe process.
///
/// The cache key covers the canonical bundle tree, sizes, and modification
/// times. A changed binary or bundle member therefore cannot silently reuse a
/// stale bus layout. The caller-supplied probe is deliberately explicit: it
/// must cross the worker/process sandbox boundary; this type never loads a
/// third-party binary in the scanner process.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginDescriptorProbeCache {
    entries: BTreeMap<String, PluginDescriptor>,
}

impl PluginDescriptorProbeCache {
    pub fn resolve_with_quarantined_probe(
        &mut self,
        discovered: &PluginDescriptor,
        probe: impl FnOnce(&PluginDescriptor) -> Result<PluginDescriptor, String>,
    ) -> Result<PluginDescriptor, String> {
        discovered.validate_for_native_probe()?;
        let fingerprint = descriptor_fingerprint(discovered)?;
        if let Some(cached) = self.entries.get(&fingerprint) {
            validate_probe_result(discovered, cached)?;
            return Ok(cached.clone());
        }

        let resolved = probe(discovered)?;
        validate_probe_result(discovered, &resolved)?;
        self.entries.insert(fingerprint, resolved.clone());
        Ok(resolved)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid external-plugin probe cache: {error}"))
    }

    pub fn to_json(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self)
            .map_err(|error| format!("could not serialize external-plugin probe cache: {error}"))
    }
}

fn validate_probe_result(
    discovered: &PluginDescriptor,
    resolved: &PluginDescriptor,
) -> Result<(), String> {
    resolved.validate()?;
    let discovered_path = discovered
        .path
        .canonicalize()
        .map_err(|error| format!("could not canonicalize discovered plugin: {error}"))?;
    let resolved_path = resolved
        .path
        .canonicalize()
        .map_err(|error| format!("could not canonicalize probed plugin: {error}"))?;
    if discovered_path != resolved_path || discovered.format != resolved.format {
        return Err("quarantined probe returned metadata for a different plugin binary".into());
    }
    if resolved.is_instrument && resolved.audio_inputs != 0 {
        return Err("instrument probe must report zero audio inputs".into());
    }
    Ok(())
}

fn descriptor_fingerprint(descriptor: &PluginDescriptor) -> Result<String, String> {
    let root = descriptor
        .path
        .canonicalize()
        .map_err(|error| format!("could not canonicalize plugin path: {error}"))?;
    let mut paths = vec![root.clone()];
    let mut members = Vec::new();
    while let Some(path) = paths.pop() {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("could not fingerprint {}: {error}", path.display()))?;
        let relative = path.strip_prefix(&root).unwrap_or(Path::new(""));
        members.push((
            relative.to_path_buf(),
            metadata.len(),
            modified_nanos(&metadata),
        ));
        if metadata.is_dir() {
            let mut children = fs::read_dir(&path)
                .map_err(|error| {
                    format!("could not scan plugin bundle {}: {error}", path.display())
                })?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("could not scan plugin bundle: {error}"))?;
            children.sort();
            paths.extend(children.into_iter().rev());
        }
    }
    members.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hash = 0xcbf29ce484222325_u64;
    hash_bytes(&mut hash, descriptor.format.extension().as_bytes());
    hash_bytes(&mut hash, root.as_os_str().as_encoded_bytes());
    for (path, len, modified) in members {
        hash_bytes(&mut hash, path.as_os_str().as_encoded_bytes());
        hash_bytes(&mut hash, &len.to_le_bytes());
        hash_bytes(&mut hash, &modified.to_le_bytes());
    }
    Ok(format!("v1-{hash:016x}"))
}

fn modified_nanos(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_plugin::{PluginFormat, PluginScanStatus};

    fn descriptor(path: std::path::PathBuf) -> PluginDescriptor {
        PluginDescriptor {
            id: "unprobed".into(),
            name: "Unprobed".into(),
            vendor: "Unknown".into(),
            version: "Unknown".into(),
            format: PluginFormat::Clap,
            path,
            audio_inputs: 0,
            audio_outputs: 0,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: PluginScanStatus::Discovered,
        }
    }

    fn resolved(source: &PluginDescriptor, inputs: usize, outputs: usize) -> PluginDescriptor {
        let mut result = source.clone();
        result.id = format!("test.{inputs}.{outputs}");
        result.audio_inputs = inputs;
        result.audio_outputs = outputs;
        result.is_instrument = inputs == 0;
        result
    }

    #[test]
    fn cache_resolves_mono_multichannel_and_instrument_layouts() {
        for (inputs, outputs) in [(1, 1), (2, 6), (0, 2)] {
            let file = tempfile::Builder::new().suffix(".clap").tempfile().unwrap();
            let discovered = descriptor(file.path().to_path_buf());
            let mut cache = PluginDescriptorProbeCache::default();
            let first = cache
                .resolve_with_quarantined_probe(&discovered, |source| {
                    Ok(resolved(source, inputs, outputs))
                })
                .unwrap();
            assert_eq!((first.audio_inputs, first.audio_outputs), (inputs, outputs));
            let restored =
                PluginDescriptorProbeCache::from_json(&cache.to_json().unwrap()).unwrap();
            assert_eq!(restored.entries.len(), 1);
        }
    }

    #[test]
    fn fingerprint_change_forces_reprobe() {
        use std::io::Write;
        let mut file = tempfile::Builder::new().suffix(".clap").tempfile().unwrap();
        file.write_all(b"a").unwrap();
        let discovered = descriptor(file.path().to_path_buf());
        let mut cache = PluginDescriptorProbeCache::default();
        cache
            .resolve_with_quarantined_probe(&discovered, |source| Ok(resolved(source, 1, 1)))
            .unwrap();
        file.write_all(b"changed").unwrap();
        let mut called = false;
        let result = cache
            .resolve_with_quarantined_probe(&discovered, |source| {
                called = true;
                Ok(resolved(source, 2, 6))
            })
            .unwrap();
        assert!(called);
        assert_eq!((result.audio_inputs, result.audio_outputs), (2, 6));
    }
}
