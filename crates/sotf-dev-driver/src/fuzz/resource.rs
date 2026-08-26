use std::collections::BTreeMap;
use std::time::Instant;

use sysinfo::{Pid, ProcessesToUpdate, System};

use super::model::ResourceSample;

pub trait ResourceSampler {
    fn sample(&mut self) -> ResourceSample;
}

pub struct SystemResourceSampler {
    system: System,
    pid: Pid,
    started: Instant,
}

impl SystemResourceSampler {
    pub fn new(pid: u32) -> Self {
        Self {
            system: System::new(),
            pid: Pid::from_u32(pid),
            started: Instant::now(),
        }
    }
}

impl ResourceSampler for SystemResourceSampler {
    fn sample(&mut self) -> ResourceSample {
        self.system.refresh_processes(ProcessesToUpdate::All);
        let mut unavailable = BTreeMap::new();
        let process = self.system.process(self.pid);
        let (rss, virtual_bytes, cpu_percent) = match process {
            Some(process) => (
                Some(process.memory()),
                Some(process.virtual_memory()),
                Some(process.cpu_usage()),
            ),
            None => {
                unavailable.insert("process".into(), "process not found".into());
                (None, None, None)
            }
        };
        let children = self
            .system
            .processes()
            .values()
            .filter(|process| process.parent() == Some(self.pid))
            .count() as u64;
        unavailable.insert(
            "threads".into(),
            "portable provider does not expose a consistent thread count".into(),
        );
        unavailable.insert(
            "descriptors_or_handles".into(),
            "platform provider not installed".into(),
        );
        unavailable.insert(
            "cpu_time_ms".into(),
            "sysinfo provider exposes utilization but not accumulated CPU time".into(),
        );
        ResourceSample {
            monotonic_ms: self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            rss_bytes: rss,
            virtual_bytes,
            cpu_percent,
            cpu_time_ms: None,
            threads: None,
            descriptors_or_handles: None,
            children: Some(children),
            unavailable,
        }
    }
}

#[derive(Debug, Default)]
pub struct NullResourceSampler {
    started: Option<Instant>,
}

impl ResourceSampler for NullResourceSampler {
    fn sample(&mut self) -> ResourceSample {
        let started = *self.started.get_or_insert_with(Instant::now);
        ResourceSample {
            monotonic_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            rss_bytes: None,
            virtual_bytes: None,
            cpu_percent: None,
            cpu_time_ms: None,
            threads: None,
            descriptors_or_handles: None,
            children: None,
            unavailable: BTreeMap::from([("all".into(), "resource sampling unavailable".into())]),
        }
    }
}
