use sotf_host::{CountingAlloc, InPlacePluginAdapter, run_standard_tests};
use sotf_plugin_speech_denoiser::SpeechDenoiserPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let plugin = SpeechDenoiserPlugin::new(2);
    let mut plugin = InPlacePluginAdapter::new(plugin);
    run_standard_tests(&mut plugin, "SpeechDenoiserPlugin");
}
