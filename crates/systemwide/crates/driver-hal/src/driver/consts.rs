use std::time::Duration;

pub(super) const DAEMON_CONFIG_ACK_TIMEOUT: Duration = Duration::from_millis(750);

pub(super) const DAEMON_CONFIG_ACK_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(super) const DAEMON_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(500);
