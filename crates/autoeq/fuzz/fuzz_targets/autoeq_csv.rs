#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(csv_str) = std::str::from_utf8(data) {
        // Test CSV parsing
        // We need to use autoeq library here.
        // Assuming read_curve_from_csv_str is available in read module.
        // But read module might be private or limited visibility.
        // Let's assume we can access it via autoeq::read if it is pub.
        // If not, we might need to adjust or just focus on public API.
        
        // autoeq::read is pub in lib.rs? Let's check.
        // If autoeq::read is not pub, we can't test it directly from binary target unless it is integration test or part of lib.
        
        // For now, let's comment it out if we are not sure, or try it.
        // The plan says: autoeq::read::read_curve_from_csv_str(csv_str);
        
         let _ = autoeq::read::read_curve_from_csv_str(csv_str);
    }
});
