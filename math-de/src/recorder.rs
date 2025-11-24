use crate::{CallbackAction, DEIntermediate};
use ndarray::Array1;
use std::fs::{File, create_dir_all};
use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex};

/// Records optimization progress for every function evaluation
#[derive(Debug)]
pub struct OptimizationRecorder {
    /// Function name (used for CSV filename)
    function_name: String,
    /// Output directory for CSV files
    output_dir: String,
    /// Shared evaluation records storage
    records: Arc<Mutex<Vec<EvaluationRecord>>>,
    /// Best function value seen so far
    best_value: Arc<Mutex<Option<f64>>>,
    /// Counter for function evaluations
    eval_counter: Arc<Mutex<usize>>,
    /// Current generation number
    current_generation: Arc<Mutex<usize>>,
    /// Block counter for periodic saves
    block_counter: Arc<Mutex<usize>>,
}

/// A single function evaluation record
#[derive(Debug, Clone)]
pub struct EvaluationRecord {
    /// Function evaluation number
    pub eval_id: usize,
    /// Generation number
    pub generation: usize,
    /// Input parameters x
    pub x: Vec<f64>,
    /// Function value f(x)
    pub f_value: f64,
    /// Current best function value so far
    pub best_so_far: f64,
    /// Whether this evaluation improved the global best
    pub is_improvement: bool,
}

/// Legacy record type for compatibility
#[derive(Debug, Clone)]
pub struct OptimizationRecord {
    /// Iteration number
    pub iteration: usize,
    /// Best x found so far
    pub x: Vec<f64>,
    /// Best function result so far
    pub best_result: f64,
    /// Convergence measure (standard deviation of population)
    pub convergence: f64,
    /// Whether this iteration improved the best known result
    pub is_improvement: bool,
}

impl OptimizationRecorder {
    /// Create a new optimization recorder for the given function
    /// Uses the default records directory under AUTOEQ_DIR/data_generated/records
    pub fn new(function_name: String) -> Self {
        Self::with_output_dir(function_name, "./data_generated/records".to_string())
    }

    /// Create a new optimization recorder with custom output directory
    pub fn with_output_dir(function_name: String, output_dir: String) -> Self {
        Self {
            function_name,
            output_dir,
            records: Arc::new(Mutex::new(Vec::new())),
            best_value: Arc::new(Mutex::new(None)),
            eval_counter: Arc::new(Mutex::new(0)),
            current_generation: Arc::new(Mutex::new(0)),
            block_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Record a single function evaluation
    pub fn record_evaluation(&self, x: &Array1<f64>, f_value: f64) {
        let mut eval_counter_guard = self.eval_counter.lock().unwrap();
        *eval_counter_guard += 1;
        let eval_id = *eval_counter_guard;

        drop(eval_counter_guard);

        // Update best value
        let mut best_guard = self.best_value.lock().unwrap();
        let is_improvement = match *best_guard {
            Some(best) => f_value < best,
            None => true,
        };

        let best_so_far = if is_improvement {
            *best_guard = Some(f_value);
            f_value
        } else {
            best_guard.unwrap_or(f_value)
        };
        drop(best_guard);

        // Record the evaluation
        let mut records_guard = self.records.lock().unwrap();
        let current_gen = *self.current_generation.lock().unwrap();
        records_guard.push(EvaluationRecord {
            eval_id,
            generation: current_gen,
            x: x.to_vec(),
            f_value,
            best_so_far,
            is_improvement,
        });

        // Check if we need to save a block (every 10k evaluations)
        if records_guard.len() >= 10_000 {
            let records_to_save = records_guard.clone();
            records_guard.clear();
            drop(records_guard);

            // Save block in background
            let mut block_counter = self.block_counter.lock().unwrap();
            *block_counter += 1;
            let block_id = *block_counter;
            drop(block_counter);

            if let Err(e) = self.save_block_to_csv(&records_to_save, block_id) {
                eprintln!(
                    "Warning: Failed to save evaluation block {}: {}",
                    block_id, e
                );
            }
        }
    }

    /// Set the current generation number
    pub fn set_generation(&self, generation: usize) {
        *self.current_generation.lock().unwrap() = generation;
    }

    /// Create a callback function that updates generation number
    pub fn create_callback(&self) -> Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send> {
        let current_generation = self.current_generation.clone();

        Box::new(move |intermediate: &DEIntermediate| -> CallbackAction {
            *current_generation.lock().unwrap() = intermediate.iter;
            CallbackAction::Continue
        })
    }

    /// Save a block of evaluations to CSV file
    fn save_block_to_csv(
        &self,
        records: &[EvaluationRecord],
        block_id: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create output directory if it doesn't exist
        create_dir_all(&self.output_dir)?;

        let filename = format!(
            "{}/{}_block_{:04}.csv",
            self.output_dir, self.function_name, block_id
        );
        let mut file = BufWriter::new(File::create(&filename)?);

        if records.is_empty() {
            return Ok(());
        }

        // Write CSV header
        let num_dimensions = records[0].x.len();
        write!(file, "eval_id,generation,")?;
        for i in 0..num_dimensions {
            write!(file, "x{},", i)?;
        }
        writeln!(file, "f_value,best_so_far,is_improvement")?;

        // Write data rows
        for record in records.iter() {
            write!(file, "{},{},", record.eval_id, record.generation)?;
            for &xi in &record.x {
                write!(file, "{:.16},", xi)?;
            }
            writeln!(
                file,
                "{:.16},{:.16},{}",
                record.f_value, record.best_so_far, record.is_improvement
            )?;
        }

        file.flush()?;
        Ok(())
    }

    /// Save any remaining records and finalize
    pub fn finalize(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // Save any remaining records
        let mut records_guard = self.records.lock().unwrap();
        if !records_guard.is_empty() {
            let records_to_save = records_guard.clone();
            records_guard.clear();
            drop(records_guard);

            let mut block_counter = self.block_counter.lock().unwrap();
            *block_counter += 1;
            let block_id = *block_counter;
            drop(block_counter);

            self.save_block_to_csv(&records_to_save, block_id)?;
        } else {
            drop(records_guard);
        }

        // Create a summary file with metadata
        self.save_summary(&[])?;

        // Return all saved CSV files
        let total_blocks = *self.block_counter.lock().unwrap();
        let mut saved_files = Vec::new();
        for block_id in 1..=total_blocks {
            saved_files.push(format!(
                "{}/{}_block_{:04}.csv",
                self.output_dir, self.function_name, block_id
            ));
        }

        Ok(saved_files)
    }

    /// Save summary file with metadata
    fn save_summary(&self, _block_files: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        let summary_filename = format!("{}/{}_summary.txt", self.output_dir, self.function_name);
        let mut file = File::create(&summary_filename)?;

        let total_evaluations = *self.eval_counter.lock().unwrap();
        let total_blocks = *self.block_counter.lock().unwrap();
        let best_value = *self.best_value.lock().unwrap();

        writeln!(file, "Function: {}", self.function_name)?;
        writeln!(file, "Total evaluations: {}", total_evaluations)?;
        writeln!(file, "Total blocks: {}", total_blocks)?;
        writeln!(file, "Best value found: {:?}", best_value)?;
        writeln!(file, "Block files:")?;

        // List all block files that were saved (from 1 to total_blocks)
        for block_id in 1..=total_blocks {
            writeln!(file, "  {}_block_{:04}.csv", self.function_name, block_id)?;
        }

        Ok(())
    }

    /// Get evaluation statistics
    pub fn get_stats(&self) -> (usize, Option<f64>, usize) {
        let total_evals = *self.eval_counter.lock().unwrap();
        let best_value = *self.best_value.lock().unwrap();
        let total_blocks = *self.block_counter.lock().unwrap();
        (total_evals, best_value, total_blocks)
    }

    /// Legacy method: Save all recorded iterations to a CSV file (for compatibility)
    pub fn save_to_csv(&self, output_dir: &str) -> Result<String, Box<dyn std::error::Error>> {
        // For backward compatibility, just finalize and return the first block filename
        let saved_files = self.finalize()?;
        if let Some(first_file) = saved_files.first() {
            Ok(first_file.clone())
        } else {
            Ok(format!("{}/{}_no_data.csv", output_dir, self.function_name))
        }
    }

    /// Get a copy of all recorded iterations (legacy compatibility - returns empty)
    pub fn get_records(&self) -> Vec<OptimizationRecord> {
        // Legacy compatibility: evaluation records are saved to disk, not kept in memory
        Vec::new()
    }

    /// Test-only method: Get evaluation records converted to legacy format
    #[cfg(test)]
    pub fn get_test_records(&self) -> Vec<OptimizationRecord> {
        let records_guard = self.records.lock().unwrap();
        records_guard
            .iter()
            .map(|eval_record| {
                OptimizationRecord {
                    iteration: eval_record.generation,
                    x: eval_record.x.clone(),
                    best_result: eval_record.best_so_far,
                    convergence: 0.0, // Not tracked in new system
                    is_improvement: eval_record.is_improvement,
                }
            })
            .collect()
    }

    /// Get the number of evaluations recorded
    pub fn num_iterations(&self) -> usize {
        *self.eval_counter.lock().unwrap()
    }

    /// Clear all recorded evaluations
    pub fn clear(&self) {
        self.records.lock().unwrap().clear();
        *self.best_value.lock().unwrap() = None;
        *self.eval_counter.lock().unwrap() = 0;
        *self.current_generation.lock().unwrap() = 0;
        *self.block_counter.lock().unwrap() = 0;
    }

    /// Get the final best solution if any evaluations were recorded
    pub fn get_best_solution(&self) -> Option<(Vec<f64>, f64)> {
        // Since we don't keep all records in memory, we can't return the exact solution
        // This would need to be reconstructed from the CSV files if needed
        (*self.best_value.lock().unwrap()).map(|best_val| (Vec::new(), best_val))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DEConfigBuilder, recorder::OptimizationRecorder, run_recorded_differential_evolution,
    };
    use autoeq_testfunctions::quadratic;
    use ndarray::Array1;

    #[test]
    fn test_optimization_recorder() {
        let recorder = OptimizationRecorder::new("test_function".to_string());

        // Test recording evaluations directly
        let x1 = Array1::from(vec![1.0, 2.0]);
        recorder.set_generation(0);
        recorder.record_evaluation(&x1, 5.0);

        let x2 = Array1::from(vec![0.5, 1.0]);
        recorder.set_generation(1);
        recorder.record_evaluation(&x2, 1.25);

        // Check records using test method
        let records = recorder.get_test_records();
        assert_eq!(records.len(), 2);

        assert_eq!(records[0].iteration, 0);
        assert_eq!(records[0].x, vec![1.0, 2.0]);
        assert_eq!(records[0].best_result, 5.0);
        assert!(records[0].is_improvement);

        assert_eq!(records[1].iteration, 1);
        assert_eq!(records[1].x, vec![0.5, 1.0]);
        assert_eq!(records[1].best_result, 1.25);
        assert!(records[1].is_improvement);
    }

    #[test]
    fn test_recorded_optimization() {
        // Test recording with simple quadratic function
        let bounds = vec![(-5.0, 5.0), (-5.0, 5.0)];
        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(50) // Keep it short for testing
            .popsize(10)
            .build();

        let result = run_recorded_differential_evolution("quadratic", quadratic, &bounds, config);

        match result {
            Ok((_de_report, csv_path)) => {
                // Check that CSV file was created
                assert!(std::path::Path::new(&csv_path).exists());
                println!("CSV saved to: {}", csv_path);

                // Read and verify CSV content
                let csv_content = std::fs::read_to_string(&csv_path).expect("Failed to read CSV");
                let lines: Vec<&str> = csv_content.trim().split('\n').collect();

                // Should have header plus at least a few iterations
                assert!(lines.len() > 1, "CSV should have header plus data rows");

                // Check header format
                let header = lines[0];
                assert!(
                    header
                        .starts_with("eval_id,generation,x0,x1,f_value,best_so_far,is_improvement")
                );

                println!(
                    "Recording test passed - {} iterations recorded",
                    lines.len() - 1
                );
            }
            Err(e) => {
                panic!(
                    "Test requires AUTOEQ_DIR to be set. Error: {}\nPlease run: export AUTOEQ_DIR=/path/to/autoeq",
                    e
                );
            }
        }
    }
}
