//! Result identity and successful export history for correction workflows.

use std::path::PathBuf;

/// A successful file write, attributed to the result captured before saving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionExport {
    pub revision: u64,
    pub format: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
struct CorrectionApplication {
    revision: u64,
    graph: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionApplicationStatus {
    NotApplied,
    Pending,
    Applied,
    PreviousResult,
    ChangedGraph,
    Failed,
}

/// Session-local delivery history. Calculating does not imply applying or saving.
#[derive(Debug, Clone, Default)]
pub struct CorrectionDelivery {
    revision: u64,
    last_export: Option<CorrectionExport>,
    pending_application: Option<CorrectionApplication>,
    last_application: Option<CorrectionApplication>,
    failed_application: Option<CorrectionApplication>,
}

impl CorrectionDelivery {
    /// Call only when a completed calculation is installed as the review result.
    pub fn calculated(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        // Do not let a wrapped counter identify an ancient export as current.
        if self.revision == 0 {
            self.last_export = None;
            self.pending_application = None;
            self.last_application = None;
            self.failed_application = None;
        }
    }

    /// Retry only the exact failed result and graph, without adding another correction.
    pub fn can_retry_application(&self, graph: &serde_json::Value) -> bool {
        self.pending_application.is_none()
            && self
                .failed_application
                .as_ref()
                .is_some_and(|failed| failed.revision == self.revision && &failed.graph == graph)
    }

    pub fn request_application(&mut self, graph: serde_json::Value) {
        self.pending_application = Some(CorrectionApplication {
            revision: self.revision,
            graph,
        });
        self.failed_application = None;
    }

    /// An acknowledgement is attributed to its submitted graph and result revision.
    pub fn pending_application_revision(&self, graph: &serde_json::Value) -> Option<u64> {
        self.pending_application
            .as_ref()
            .filter(|pending| &pending.graph == graph)
            .map(|pending| pending.revision)
    }

    pub fn acknowledge_application(
        &mut self,
        revision: u64,
        graph: &serde_json::Value,
        succeeded: bool,
    ) -> bool {
        if !self
            .pending_application
            .as_ref()
            .is_some_and(|pending| pending.revision == revision && &pending.graph == graph)
        {
            return false;
        }
        let request = self.pending_application.take();
        if succeeded {
            self.last_application = request;
            self.failed_application = None;
        } else {
            self.failed_application = request;
        }
        true
    }

    pub fn application_status(
        &self,
        result_is_current: bool,
        graph: &serde_json::Value,
        runtime_matches: bool,
    ) -> CorrectionApplicationStatus {
        use CorrectionApplicationStatus::*;
        if let Some(pending) = &self.pending_application {
            return if &pending.graph == graph {
                Pending
            } else {
                ChangedGraph
            };
        }
        if self
            .failed_application
            .as_ref()
            .is_some_and(|failed| &failed.graph == graph)
        {
            return Failed;
        }
        let Some(applied) = &self.last_application else {
            return NotApplied;
        };
        if &applied.graph != graph || !runtime_matches {
            ChangedGraph
        } else if applied.revision != self.revision || !result_is_current {
            PreviousResult
        } else {
            Applied
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Call after a successful write, using the revision captured with its data.
    pub fn exported(&mut self, revision: u64, format: String, path: PathBuf) {
        self.last_export = Some(CorrectionExport {
            revision,
            format,
            path,
        });
    }

    pub fn last_export(&self) -> Option<&CorrectionExport> {
        self.last_export.as_ref()
    }

    pub fn current_result_exported(&self, result_is_current: bool) -> bool {
        result_is_current
            && self
                .last_export
                .as_ref()
                .is_some_and(|export| export.revision == self.revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_export_does_not_export_later_calculations_or_changed_inputs() {
        let mut delivery = CorrectionDelivery::default();
        assert!(!delivery.current_result_exported(true));
        delivery.calculated();
        let saved_revision = delivery.revision();
        delivery.exported(saved_revision, "json".into(), "correction.json".into());
        assert!(delivery.current_result_exported(true));
        assert!(!delivery.current_result_exported(false));
        delivery.calculated();
        assert!(!delivery.current_result_exported(true));
        assert_eq!(delivery.last_export().unwrap().revision, saved_revision);
    }

    #[test]
    fn delayed_save_keeps_the_revision_and_format_of_the_written_data() {
        let mut delivery = CorrectionDelivery::default();
        delivery.calculated();
        let dialog_revision = delivery.revision();
        delivery.calculated();
        delivery.exported(dialog_revision, "apo".into(), "old-result.txt".into());
        assert!(!delivery.current_result_exported(true));
        let saved = delivery.last_export().unwrap();
        assert_eq!(saved.path, PathBuf::from("old-result.txt"));
        assert_eq!(saved.format, "apo");
    }

    #[test]
    fn application_requires_matching_acknowledgement_and_runtime_graph() {
        use CorrectionApplicationStatus::*;
        let mut delivery = CorrectionDelivery::default();
        delivery.calculated();
        let graph = serde_json::json!({"gain": -3});
        let other = serde_json::json!({"gain": -6});
        delivery.request_application(graph.clone());
        assert_eq!(delivery.application_status(true, &graph, true), Pending);
        assert!(!delivery.acknowledge_application(delivery.revision(), &other, true));
        assert_eq!(delivery.application_status(true, &graph, true), Pending);
        assert!(delivery.acknowledge_application(delivery.revision(), &graph, true));
        assert_eq!(
            delivery.application_status(true, &graph, false),
            ChangedGraph
        );
        assert_eq!(delivery.application_status(true, &graph, true), Applied);
        assert_eq!(
            delivery.application_status(false, &graph, true),
            PreviousResult
        );
        assert_eq!(
            delivery.application_status(true, &other, true),
            ChangedGraph
        );
        delivery.calculated();
        assert_eq!(
            delivery.application_status(true, &graph, true),
            PreviousResult
        );
    }

    #[test]
    fn delayed_application_keeps_its_revision_and_failure_can_be_retried() {
        use CorrectionApplicationStatus::*;
        let mut delivery = CorrectionDelivery::default();
        let graph = serde_json::json!({"filter": 1});
        delivery.calculated();
        delivery.request_application(graph.clone());
        let submitted_revision = delivery.revision();
        delivery.calculated();
        delivery.acknowledge_application(submitted_revision, &graph, true);
        assert_eq!(
            delivery.application_status(true, &graph, true),
            PreviousResult
        );
        delivery.request_application(graph.clone());
        delivery.acknowledge_application(delivery.revision(), &graph, false);
        assert_eq!(delivery.application_status(true, &graph, true), Failed);
        delivery.request_application(graph.clone());
        delivery.acknowledge_application(delivery.revision(), &graph, true);
        assert_eq!(delivery.application_status(true, &graph, true), Applied);
    }

    #[test]
    fn identical_graph_from_an_older_request_cannot_accept_a_newer_revision() {
        let mut delivery = CorrectionDelivery::default();
        let graph = serde_json::json!({"gain": 0});
        delivery.calculated();
        delivery.request_application(graph.clone());
        let old_revision = delivery.revision();
        delivery.calculated();
        delivery.request_application(graph.clone());
        assert!(!delivery.acknowledge_application(old_revision, &graph, true));
        assert_eq!(
            delivery.application_status(true, &graph, true),
            CorrectionApplicationStatus::Pending
        );
        assert!(delivery.acknowledge_application(delivery.revision(), &graph, true));
    }
    #[test]
    fn retry_requires_same_failed_graph_and_result_revision() {
        let mut delivery = CorrectionDelivery::default();
        let graph = serde_json::json!({"eq": ["notch"]});
        delivery.calculated();
        assert!(!delivery.can_retry_application(&graph));
        delivery.request_application(graph.clone());
        assert!(!delivery.can_retry_application(&graph));
        assert!(delivery.acknowledge_application(delivery.revision(), &graph, false));
        assert!(delivery.can_retry_application(&graph));
        assert!(!delivery.can_retry_application(&serde_json::json!({"eq": ["peak"]})));
        delivery.request_application(graph.clone());
        assert!(!delivery.can_retry_application(&graph));
        assert!(delivery.acknowledge_application(delivery.revision(), &graph, false));
        delivery.calculated();
        assert!(!delivery.can_retry_application(&graph));
    }
}
