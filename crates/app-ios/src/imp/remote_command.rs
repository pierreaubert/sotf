use super::pending::pending_queue;
use super::types::RemoteCommand;

/// Push a remote command for the GPUI loop to drain.
///
/// The sole drain is the `sotf_ios_pop_remote_command` FFI function (one code
/// per call, consumed by `app-gpui` without depending on this crate). Do not
/// add a second typed drain here: two parallel consumers would split the FIFO
/// and silently drop or reorder commands.
pub(super) fn push_remote_command(cmd: RemoteCommand) {
    pending_queue().push(cmd);
}
