use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::model::{Action, FUZZ_SCHEMA_VERSION, TraceEvent};

pub struct TraceWriter {
    path: PathBuf,
    writer: BufWriter<File>,
    sync_data: bool,
}

impl TraceWriter {
    pub fn create(path: impl Into<PathBuf>, sync_data: bool) -> Result<Self, TraceError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;
        Ok(Self {
            path,
            writer: BufWriter::new(file),
            sync_data,
        })
    }

    pub fn append(&mut self, event: &TraceEvent) -> Result<(), TraceError> {
        serde_json::to_writer(&mut self.writer, event)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        if self.sync_data {
            self.writer.get_ref().sync_data()?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn read_trace(path: &Path) -> Result<Vec<TraceEvent>, TraceError> {
    let file = File::open(path)?;
    let mut events = Vec::new();
    let mut lines = BufReader::new(file).split(b'\n').peekable();
    while let Some(line) = lines.next() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        match serde_json::from_slice(&line) {
            Ok(event) => events.push(event),
            Err(_) if lines.peek().is_none() => break,
            Err(error) => return Err(TraceError::Json(error)),
        }
    }
    // Reject an incompatible trace schema explicitly; old traces are never
    // silently reinterpreted (a torn trace may legitimately lack run_start).
    if let Some(TraceEvent::RunStart { schema_version, .. }) = events.first()
        && *schema_version != FUZZ_SCHEMA_VERSION
    {
        return Err(TraceError::SchemaVersion {
            expected: FUZZ_SCHEMA_VERSION,
            found: *schema_version,
        });
    }
    Ok(events)
}

pub fn resolved_actions(events: &[TraceEvent]) -> Vec<Action> {
    events
        .iter()
        .filter_map(|event| match event {
            TraceEvent::ActionIntent { action, .. } => Some(action.as_ref().clone()),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum TraceError {
    #[error("trace I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("trace JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("trace schema version {found} is unsupported (expected {expected})")]
    SchemaVersion { expected: u16, found: u16 },
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;
    use crate::fuzz::model::{
        ActionClass, ActionPayload, AdapterKind, EndpointSpec, FUZZ_SCHEMA_VERSION, TargetId,
        TargetSpec,
    };

    fn action(sequence: u64) -> Action {
        Action {
            schema_version: FUZZ_SCHEMA_VERSION,
            sequence,
            id: "test".into(),
            family: "test".into(),
            class: ActionClass::StateValid,
            precondition_id: None,
            precondition_satisfied: true,
            payload: ActionPayload::Wait { duration_ms: 1 },
            timeout_ms: 10,
            rng_cursor: sequence,
        }
    }

    #[test]
    fn recovers_a_torn_final_line_without_hiding_middle_corruption() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trace.ndjson");
        let mut writer = TraceWriter::create(&path, false).unwrap();
        writer
            .append(&TraceEvent::ActionIntent {
                action: Box::new(action(1)),
                preceding_revision: 0,
                preceding_state_hash: "hash".into(),
            })
            .unwrap();
        drop(writer);
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"event\":\"action")
            .unwrap();
        let events = read_trace(&path).unwrap();
        assert_eq!(resolved_actions(&events), vec![action(1)]);

        fs::write(&path, b"not-json\n{}\n").unwrap();
        assert!(read_trace(&path).is_err());
    }

    #[test]
    fn rejects_a_mismatched_trace_schema_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trace.ndjson");
        let target = TargetSpec {
            schema_version: FUZZ_SCHEMA_VERSION,
            target_id: TargetId::Tui,
            adapter: AdapterKind::Synthetic,
            executable: None,
            app_identity: None,
            platform: "test".into(),
            fixture_profile: "none".into(),
            environment_names: vec![],
            endpoints: vec![EndpointSpec {
                name: "synthetic".into(),
                address: "in-memory".into(),
                protocol: "synthetic".into(),
            }],
            run_id_hash: String::new(),
            capability_fingerprint: String::new(),
            build_id: "test".into(),
        };
        let mut writer = TraceWriter::create(&path, false).unwrap();
        writer
            .append(&TraceEvent::RunStart {
                schema_version: FUZZ_SCHEMA_VERSION + 1,
                target: Box::new(target.clone()),
                seed: 1,
                worker: 0,
                worker_seed: 1,
                generator_version: 1,
                capabilities: Box::new(sotf_dev_api::Capabilities::new("tui", "test")),
                manifest_digest: "m".into(),
                fixture_digest: "f".into(),
            })
            .unwrap();
        drop(writer);
        assert!(matches!(
            read_trace(&path),
            Err(TraceError::SchemaVersion {
                expected: FUZZ_SCHEMA_VERSION,
                ..
            })
        ));

        // The current schema version loads fine.
        let mut writer = TraceWriter::create(&path, false).unwrap();
        writer
            .append(&TraceEvent::RunStart {
                schema_version: FUZZ_SCHEMA_VERSION,
                target: Box::new(target),
                seed: 1,
                worker: 0,
                worker_seed: 1,
                generator_version: 1,
                capabilities: Box::new(sotf_dev_api::Capabilities::new("tui", "test")),
                manifest_digest: "m".into(),
                fixture_digest: "f".into(),
            })
            .unwrap();
        drop(writer);
        assert_eq!(read_trace(&path).unwrap().len(), 1);
    }
}
