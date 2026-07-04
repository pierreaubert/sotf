use super::consts::PENDING_DYNAMIC_TYPE_SCALES;
use super::consts::PENDING_IMPORTED_FILES;
use super::consts::PENDING_QR_PAYLOADS;
use super::consts::PENDING_REMOTE_COMMANDS;
use super::types::RemoteCommand;
use crossbeam::queue::SegQueue;
use std::path::PathBuf;

/// Returns the lock-free remote-command queue.
pub(super) fn pending_queue() -> &'static SegQueue<RemoteCommand> {
    PENDING_REMOTE_COMMANDS.get_or_init(SegQueue::new)
}

/// Returns the lock-free imported-file queue.
pub(super) fn pending_imports() -> &'static SegQueue<PathBuf> {
    PENDING_IMPORTED_FILES.get_or_init(SegQueue::new)
}

/// Returns the lock-free QR-payload queue.
pub(super) fn pending_qr_payloads() -> &'static SegQueue<String> {
    PENDING_QR_PAYLOADS.get_or_init(SegQueue::new)
}

/// Returns the lock-free Dynamic Type scale queue.
pub(super) fn pending_dynamic_type_scales() -> &'static SegQueue<f32> {
    PENDING_DYNAMIC_TYPE_SCALES.get_or_init(SegQueue::new)
}

#[cfg(test)]
mod tests {
    use super::super::consts::QUEUE_TEST_LOCK;
    use super::*;

    #[test]
    fn pending_remote_command_queue_is_fifo_and_lock_free() {
        let _guard = QUEUE_TEST_LOCK.lock();
        // Drain any stale commands left by other tests.
        while pending_queue().pop().is_some() {}
        while pending_dynamic_type_scales().pop().is_some() {}

        pending_queue().push(RemoteCommand::NextTrack);
        pending_queue().push(RemoteCommand::PrevTrack);
        pending_queue().push(RemoteCommand::QrPayloadScanned);
        pending_queue().push(RemoteCommand::MemoryWarning);

        assert!(matches!(
            pending_queue().pop(),
            Some(RemoteCommand::NextTrack)
        ));
        assert!(matches!(
            pending_queue().pop(),
            Some(RemoteCommand::PrevTrack)
        ));
        assert!(matches!(
            pending_queue().pop(),
            Some(RemoteCommand::QrPayloadScanned)
        ));
        assert!(matches!(
            pending_queue().pop(),
            Some(RemoteCommand::MemoryWarning)
        ));
        assert!(pending_queue().pop().is_none());
    }

    #[test]
    fn pending_imported_files_queue_is_fifo_and_lock_free() {
        let _guard = QUEUE_TEST_LOCK.lock();
        while pending_imports().pop().is_some() {}

        pending_imports().push(PathBuf::from("/tmp/a.mp3"));
        pending_imports().push(PathBuf::from("/tmp/b.flac"));

        assert_eq!(
            pending_imports().pop().unwrap(),
            PathBuf::from("/tmp/a.mp3")
        );
        assert_eq!(
            pending_imports().pop().unwrap(),
            PathBuf::from("/tmp/b.flac")
        );
        assert!(pending_imports().pop().is_none());
    }

    #[test]
    fn pending_qr_payload_queue_is_fifo_and_lock_free() {
        let _guard = QUEUE_TEST_LOCK.lock();
        while pending_qr_payloads().pop().is_some() {}

        pending_qr_payloads().push("payload-1".to_string());
        pending_qr_payloads().push("payload-2".to_string());

        assert_eq!(pending_qr_payloads().pop().unwrap(), "payload-1");
        assert_eq!(pending_qr_payloads().pop().unwrap(), "payload-2");
        assert!(pending_qr_payloads().pop().is_none());
    }

    #[test]
    fn pending_dynamic_type_scale_queue_is_fifo_and_lock_free() {
        let _guard = QUEUE_TEST_LOCK.lock();
        while pending_dynamic_type_scales().pop().is_some() {}

        pending_dynamic_type_scales().push(1.2);
        pending_dynamic_type_scales().push(0.9);

        assert_eq!(pending_dynamic_type_scales().pop().unwrap(), 1.2);
        assert_eq!(pending_dynamic_type_scales().pop().unwrap(), 0.9);
        assert!(pending_dynamic_type_scales().pop().is_none());
    }
}
