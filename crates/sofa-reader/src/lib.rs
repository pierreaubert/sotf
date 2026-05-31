pub mod error;
mod hdf5;
mod hrtf;

pub use error::{Result, SofaError};
pub use hdf5::{AttrValue, DType, Hdf5File};
pub use hrtf::{
    CoordinateSystem, HrtfData, SimpleFreeFieldHrtf, SofaFile, SourcePosition,
    write_simple_free_field_hrtf,
};

use std::path::Path;

pub struct SofaReader {
    hdf5: Hdf5File,
}

impl SofaReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let hdf5 = Hdf5File::open(path)?;
        Ok(Self { hdf5 })
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let hdf5 = Hdf5File::from_bytes(data)?;
        Ok(Self { hdf5 })
    }

    pub fn attribute_string(&self, name: &str) -> Result<String> {
        self.hdf5.attribute_string(name)
    }

    pub fn attribute_f64(&self, name: &str) -> Result<f64> {
        self.hdf5.attribute_f64(name)
    }

    pub fn dimension(&self, name: &str) -> Result<usize> {
        self.hdf5.dimension(name)
    }

    pub fn read_f32(&self, name: &str) -> Result<Vec<f32>> {
        self.hdf5.read_f32(name)
    }

    pub fn read_f64(&self, name: &str) -> Result<Vec<f64>> {
        self.hdf5.read_f64(name)
    }

    pub fn read_scalar_f32(&self, name: &str) -> Result<f32> {
        self.hdf5.read_scalar_f32(name)
    }

    pub fn read_scalar_f64(&self, name: &str) -> Result<f64> {
        self.hdf5.read_scalar_f64(name)
    }

    pub fn attribute(&self, name: &str) -> Option<&AttrValue> {
        self.hdf5.attribute(name)
    }
}

pub struct SofaWriter {
    inner: hdf5::Hdf5Writer,
}

impl SofaWriter {
    pub fn new() -> Self {
        Self {
            inner: hdf5::Hdf5Writer::new(),
        }
    }

    pub fn add_attribute_str(&mut self, name: &str, value: &str) {
        self.inner.add_attribute_str(name, value);
    }

    pub fn add_attribute_f32(&mut self, name: &str, value: f32) {
        self.inner.add_attribute_f32(name, value);
    }

    pub fn add_attribute_f64(&mut self, name: &str, value: f64) {
        self.inner.add_attribute_f64(name, value);
    }

    pub fn add_dimension(&mut self, name: &str, size: usize) {
        self.inner.add_dimension(name, size);
    }

    pub fn add_variable_f32(&mut self, name: &str, dims: &[&str]) {
        self.inner.add_variable_f32(name, dims);
    }

    pub fn add_variable_f64(&mut self, name: &str, dims: &[&str]) {
        self.inner.add_variable_f64(name, dims);
    }

    pub fn add_variable_attribute_str(&mut self, variable: &str, name: &str, value: &str) {
        self.inner.add_variable_attribute_str(variable, name, value);
    }

    pub fn add_variable_attribute_f32(&mut self, variable: &str, name: &str, value: f32) {
        self.inner.add_variable_attribute_f32(variable, name, value);
    }

    pub fn add_variable_attribute_f64(&mut self, variable: &str, name: &str, value: f64) {
        self.inner.add_variable_attribute_f64(variable, name, value);
    }

    pub fn write_scalar_f32(&mut self, name: &str, value: f32) -> Result<()> {
        self.inner.write_scalar_f32(name, value)
    }

    pub fn write_scalar_f64(&mut self, name: &str, value: f64) -> Result<()> {
        self.inner.write_scalar_f64(name, value)
    }

    pub fn write_f32(&mut self, name: &str, data: &[f32]) -> Result<()> {
        self.inner.write_f32(name, data)
    }

    pub fn write_f64(&mut self, name: &str, data: &[f64]) -> Result<()> {
        self.inner.write_f64(name, data)
    }

    pub fn finish<P: AsRef<Path>>(self, path: P) -> Result<()> {
        self.inner.finish(path)
    }
}

impl Default for SofaWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sofa");

        // Write
        let mut w = SofaWriter::new();
        w.add_attribute_str("Conventions", "SOFA");
        w.add_attribute_str("Version", "2.1");
        w.add_attribute_str("SOFAConventions", "SimpleFreeFieldHRIR");
        w.add_attribute_str("SOFAConventionsVersion", "1.0");
        w.add_attribute_str("DataType", "FIR");
        w.add_dimension("M", 3);
        w.add_dimension("R", 2);
        w.add_dimension("N", 4);
        w.add_dimension("C", 3);

        w.add_variable_f32("Data.SamplingRate", &[]);
        w.write_scalar_f32("Data.SamplingRate", 48000.0).unwrap();

        let positions = vec![
            0.0, 0.0, 1.0, // pos 0
            90.0, 0.0, 1.0, // pos 1
            180.0, 0.0, 1.0, // pos 2
        ];
        w.add_variable_f32("SourcePosition", &["M", "C"]);
        w.write_f32("SourcePosition", &positions).unwrap();

        let ir_data = vec![0.1f32; 3 * 2 * 4];
        w.add_variable_f32("Data.IR", &["M", "R", "N"]);
        w.write_f32("Data.IR", &ir_data).unwrap();

        w.finish(&path).unwrap();

        // Read back
        let r = SofaReader::open(&path).unwrap();
        assert_eq!(
            r.attribute_string("SOFAConventions").unwrap(),
            "SimpleFreeFieldHRIR"
        );
        assert_eq!(r.dimension("M").unwrap(), 3);
        assert_eq!(r.dimension("R").unwrap(), 2);
        assert_eq!(r.dimension("N").unwrap(), 4);

        let sr = r.read_scalar_f32("Data.SamplingRate").unwrap();
        assert!((sr - 48000.0).abs() < 0.01);

        let pos = r.read_f32("SourcePosition").unwrap();
        assert_eq!(pos.len(), 9);
        assert!((pos[3] - 90.0).abs() < 0.01);

        let ir = r.read_f32("Data.IR").unwrap();
        assert_eq!(ir.len(), 24);
        assert!((ir[0] - 0.1).abs() < 0.001);

        let sofa = SofaFile::strict_load(&path).unwrap();
        assert_eq!(sofa.num_measurements, 3);
        assert_eq!(sofa.ir_length, 4);
    }

    #[test]
    fn test_strict_load_rejects_missing_conventions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing-conventions.sofa");

        let mut w = SofaWriter::new();
        w.add_attribute_str("SOFAConventions", "SimpleFreeFieldHRIR");
        w.add_attribute_str("SOFAConventionsVersion", "1.0");
        w.add_attribute_str("Version", "2.1");
        w.add_attribute_str("DataType", "FIR");
        w.add_dimension("M", 1);
        w.add_dimension("R", 2);
        w.add_dimension("N", 2);
        w.add_dimension("C", 3);
        w.add_variable_f32("Data.SamplingRate", &[]);
        w.write_scalar_f32("Data.SamplingRate", 48000.0).unwrap();
        w.add_variable_f32("SourcePosition", &["M", "C"]);
        w.write_f32("SourcePosition", &[0.0, 0.0, 1.0]).unwrap();
        w.add_variable_f32("Data.IR", &["M", "R", "N"]);
        w.write_f32("Data.IR", &[0.0; 4]).unwrap();
        w.finish(&path).unwrap();

        let error = match SofaFile::strict_load(&path) {
            Ok(_) => panic!("strict_load should reject missing Conventions"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("Conventions"));
    }

    #[test]
    fn test_write_simple_free_field_hrtf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hrtf.sofa");

        let hrtf = SimpleFreeFieldHrtf {
            real: &[1.0, 2.0, 3.0, 4.0],
            imag: &[0.1, 0.2, 0.3, 0.4],
            frequencies: &[100.0, 200.0],
            source_position: &[1.0, 0.0, 0.0],
            source_coords: CoordinateSystem::Cartesian,
            receiver_position: &[0.0, 0.09, 0.0, 0.0, -0.09, 0.0],
            receiver_coords: CoordinateSystem::Cartesian,
            listener_position: None,
            title: Some("frequency response"),
            measurements: 1,
            receivers: 2,
        };

        write_simple_free_field_hrtf(&path, &hrtf).unwrap();

        let r = SofaReader::open(&path).unwrap();
        assert_eq!(
            r.attribute_string("SOFAConventions").unwrap(),
            "SimpleFreeFieldHRTF"
        );
        assert_eq!(r.attribute_string("DataType").unwrap(), "TF");
        assert_eq!(r.dimension("M").unwrap(), 1);
        assert_eq!(r.dimension("R").unwrap(), 2);
        assert_eq!(r.dimension("N").unwrap(), 2);
        assert_eq!(r.read_f64("N").unwrap(), vec![100.0, 200.0]);
        assert_eq!(r.read_f64("Data.Real").unwrap(), vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            r.attribute("SourcePosition:Type"),
            Some(&AttrValue::String("cartesian".to_string()))
        );
        assert_eq!(
            r.attribute("N:Units"),
            Some(&AttrValue::String("hertz".to_string()))
        );
    }

    #[test]
    fn test_read_real_sofa() {
        // Try multiple relative paths since test cwd varies
        let candidates = [
            "data_cached/org.sofacoustics/mit/kemar_normal_pinna.sofa",
            "../../data_cached/org.sofacoustics/mit/kemar_normal_pinna.sofa",
            "../../../data_cached/org.sofacoustics/mit/kemar_normal_pinna.sofa",
        ];
        let path = match candidates.iter().find(|p| std::path::Path::new(p).exists()) {
            Some(p) => *p,
            None => {
                eprintln!("Skipping real SOFA test (file not found)");
                return;
            }
        };
        if std::fs::metadata(path).map_or(true, |metadata| metadata.len() == 0) {
            eprintln!("Skipping real SOFA test (file is empty)");
            return;
        }

        let r = SofaReader::open(path).unwrap();

        // KEMAR dataset should have these dimensions
        let m = r.dimension("M").unwrap();
        let n = r.dimension("N").unwrap();
        let r_dim = r.dimension("R").unwrap();
        assert!(m > 0, "M should be > 0, got {}", m);
        assert!(n > 0, "N should be > 0, got {}", n);
        assert_eq!(r_dim, 2, "R should be 2 (binaural)");

        let convention = r.attribute_string("SOFAConventions").unwrap();
        assert_eq!(convention, "SimpleFreeFieldHRIR");

        let sr = r.read_scalar_f32("Data.SamplingRate").unwrap();
        assert!(sr > 0.0, "Sample rate should be positive");

        let ir = r.read_f32("Data.IR").unwrap();
        assert_eq!(ir.len(), m * 2 * n);

        let positions = r.read_f32("SourcePosition").unwrap();
        assert_eq!(positions.len(), m * 3);

        eprintln!(
            "KEMAR: M={}, R={}, N={}, SR={}, IR len={}, Pos len={}",
            m,
            r_dim,
            n,
            sr,
            ir.len(),
            positions.len()
        );
    }
}
