#[cfg(test)]
mod tests {
    use sotf_plugins::{GainPlugin, CompressorPlugin, InPlacePluginAdapter, PerformanceProfiler};

    #[test]
    fn test_generate_performance_summary() {
        let sample_rate = 48000.0;
        let mut plugins: Vec<(String, Box<dyn sotf_plugins::Plugin>)> = vec![];
        
        let gain = GainPlugin::new(2, 0.0);
        plugins.push(("Gain".to_string(), Box::new(InPlacePluginAdapter::new(gain))));
        
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        plugins.push(("Compressor".to_string(), Box::new(InPlacePluginAdapter::new(compressor))));
        
        println!("
=== Plugin Performance Summary ===");
        println!("{:<20} | {:<15} | {:<10}", "Plugin", "CPU Usage (%)", "Latency");
        println!("{:-<20}-|-{:-<15}-|-{:-<10}", "", "", "");
        
        for (name, mut plugin) in plugins {
            plugin.initialize(sample_rate as u32).unwrap();
            let profiler = PerformanceProfiler::new(&name, sample_rate, 2, 512);
            let cpu = profiler.profile(plugin.as_mut(), 0.5);
            let latency = sotf_plugins::detect_latency(plugin.as_mut(), sample_rate);
            println!("{:<20} | {:<15.4} | {:<10}", name, cpu, latency);
            
            assert!(cpu >= 0.0);
        }
    }
}
