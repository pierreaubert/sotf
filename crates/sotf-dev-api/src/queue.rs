use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use thiserror::Error;

use crate::protocol::QueueMetadata;

#[derive(Debug, Default)]
struct QueueState {
    depth: AtomicUsize,
    high_water: AtomicUsize,
    rejected: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct QueueTelemetry(Arc<QueueState>);

impl QueueTelemetry {
    pub fn snapshot(&self) -> QueueMetadata {
        QueueMetadata {
            depth: self.0.depth.load(Ordering::Relaxed),
            high_water: self.0.high_water.load(Ordering::Relaxed),
            rejected: self.0.rejected.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
pub struct BoundedSender<T> {
    sender: mpsc::SyncSender<T>,
    telemetry: QueueTelemetry,
}

impl<T> Clone for BoundedSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            telemetry: self.telemetry.clone(),
        }
    }
}

#[derive(Debug)]
pub struct BoundedReceiver<T> {
    receiver: mpsc::Receiver<T>,
    telemetry: QueueTelemetry,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
    #[error("bounded command queue is full")]
    Full,
    #[error("bounded command queue is disconnected")]
    Disconnected,
}

pub fn bounded_channel<T>(capacity: usize) -> (BoundedSender<T>, BoundedReceiver<T>) {
    assert!(capacity > 0, "bounded queue capacity must be non-zero");
    let (sender, receiver) = mpsc::sync_channel(capacity);
    let telemetry = QueueTelemetry(Arc::new(QueueState::default()));
    (
        BoundedSender {
            sender,
            telemetry: telemetry.clone(),
        },
        BoundedReceiver {
            receiver,
            telemetry,
        },
    )
}

impl<T> BoundedSender<T> {
    pub fn try_send(&self, value: T) -> Result<(), QueueError> {
        let depth = self.telemetry.0.depth.fetch_add(1, Ordering::Relaxed) + 1;
        match self.sender.try_send(value) {
            Ok(()) => {
                self.telemetry
                    .0
                    .high_water
                    .fetch_max(depth, Ordering::Relaxed);
                Ok(())
            }
            Err(mpsc::TrySendError::Full(_)) => {
                self.telemetry.0.depth.fetch_sub(1, Ordering::Relaxed);
                self.telemetry.0.rejected.fetch_add(1, Ordering::Relaxed);
                Err(QueueError::Full)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.telemetry.0.depth.fetch_sub(1, Ordering::Relaxed);
                Err(QueueError::Disconnected)
            }
        }
    }

    pub fn telemetry(&self) -> QueueTelemetry {
        self.telemetry.clone()
    }
}

impl<T> BoundedReceiver<T> {
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, mpsc::RecvTimeoutError> {
        let value = self.receiver.recv_timeout(timeout)?;
        self.telemetry.0.depth.fetch_sub(1, Ordering::Relaxed);
        Ok(value)
    }

    pub fn telemetry(&self) -> QueueTelemetry {
        self.telemetry.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_depth_high_water_and_rejections() {
        let (sender, receiver) = bounded_channel(1);
        sender.try_send(1).unwrap();
        assert_eq!(sender.try_send(2), Err(QueueError::Full));
        assert_eq!(sender.telemetry().snapshot().depth, 1);
        assert_eq!(sender.telemetry().snapshot().high_water, 1);
        assert_eq!(sender.telemetry().snapshot().rejected, 1);
        assert_eq!(receiver.recv_timeout(Duration::from_millis(1)).unwrap(), 1);
        assert_eq!(receiver.telemetry().snapshot().depth, 0);
    }
}
