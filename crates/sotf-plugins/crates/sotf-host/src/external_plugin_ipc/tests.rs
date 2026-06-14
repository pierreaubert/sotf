use super::plugin_ipc_layout::PluginIpcLayout;
use super::plugin_ipc_state::PluginIpcState;
use super::secure_plugin_shared_memory::SecurePluginSharedMemory;
use crate::plugin::{Plugin, ProcessContext};

use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{PluginInfo, PluginResult};

struct ScalePlugin {
    channels: usize,
    factor: f32,
}

impl Plugin for ScalePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Scale", "0.1", "test")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> PluginResult<()> {
        Ok(())
    }

    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let samples = context.num_frames * self.channels;
        for idx in 0..samples {
            output[idx] = input[idx] * self.factor;
        }
        Ok(context.num_frames)
    }
}

#[test]
fn test_secure_plugin_shared_memory_roundtrip() {
    let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
    let mut shared = SecurePluginSharedMemory::create(layout).unwrap();
    assert_eq!(shared.layout(), layout);
    assert!(shared.path().exists());

    shared.publish_host_sequence(7);
    shared.publish_worker_sequence(6);
    assert_eq!(shared.host_sequence(), 7);
    assert_eq!(shared.worker_sequence(), 6);

    let (input, output) = shared.audio_slices_mut();
    assert_eq!(input.len(), 256);
    assert_eq!(output.len(), 256);
    input[0] = 0.25;
    output[0] = -0.5;
    assert_eq!(input[0], 0.25);
    assert_eq!(output[0], -0.5);
}

#[test]
fn test_publish_host_block_clears_only_current_output_block() {
    let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
    let mut shared = SecurePluginSharedMemory::create(layout).unwrap();
    let input = vec![0.25, -0.5, 1.0, -1.0];

    {
        let (_input, output) = shared.audio_slices_mut();
        output.fill(9.0);
    }

    shared.publish_host_block(42, 2, &input).unwrap();

    let (_input, output) = shared.audio_slices_mut();
    assert_eq!(&output[..4], &[0.0, 0.0, 0.0, 0.0]);
    assert_eq!(output[4], 9.0);
    assert_eq!(output[output.len() - 1], 9.0);
}

#[test]
fn test_worker_processes_request_through_private_buffers() {
    let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
    let mut host_shared = SecurePluginSharedMemory::create(layout).unwrap();
    let mut worker_shared = SecurePluginSharedMemory::open_existing(host_shared.path()).unwrap();
    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];

    host_shared.publish_host_block(42, 2, &input).unwrap();
    assert_eq!(worker_shared.host_state(), PluginIpcState::HostReady);
    let request = worker_shared
        .take_worker_request()
        .unwrap()
        .expect("request should be ready");

    let mut plugin = ScalePlugin {
        channels: 2,
        factor: 2.0,
    };
    let mut input_scratch = Vec::new();
    let mut output_scratch = Vec::new();
    let frames = worker_shared
        .process_worker_request(
            &mut plugin,
            request,
            &mut input_scratch,
            &mut output_scratch,
        )
        .unwrap();

    assert_eq!(frames, 2);
    assert_eq!(worker_shared.worker_state(), PluginIpcState::WorkerReady);
    assert_eq!(host_shared.copy_worker_output(&mut output).unwrap(), 2);
    assert_eq!(output, vec![0.5, -1.0, 2.0, -2.0]);
    assert_eq!(host_shared.worker_sequence(), 42);
}

#[test]
fn test_open_existing_validates_header() {
    let layout = PluginIpcLayout::new(48_000, 64, 1, 2).unwrap();
    let shared = SecurePluginSharedMemory::create(layout).unwrap();
    let reopened = SecurePluginSharedMemory::open_existing(shared.path()).unwrap();
    assert_eq!(reopened.layout(), layout);
}

#[cfg(unix)]
#[test]
fn test_secure_plugin_shared_memory_uses_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let layout = PluginIpcLayout::new(48_000, 64, 1, 1).unwrap();
    let shared = SecurePluginSharedMemory::create(layout).unwrap();

    let file_mode = std::fs::metadata(shared.path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(file_mode, 0o600);

    let parent_mode = std::fs::metadata(shared.path().parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(parent_mode, 0o700);
}

#[cfg(unix)]
#[test]
fn test_open_existing_rejects_symlink() {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "sotf-plugin-ipc-symlink-test-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();

    let target = root.join("target.shm");
    std::fs::write(&target, b"not a valid mapping").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    let link = root.join("link.shm");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    assert!(SecurePluginSharedMemory::open_existing(&link).is_err());

    let _ = std::fs::remove_file(link);
    let _ = std::fs::remove_file(target);
    let _ = std::fs::remove_dir(root);
}
