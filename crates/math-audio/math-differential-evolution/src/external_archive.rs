//! External Archive for L-SHADE
//!
//! Stores recently discarded good solutions to maintain population diversity.
//! Used by SHADE, L-SHADE, and their variants.

use ndarray::Array1;

/// External archive storing discarded solutions
#[derive(Debug, Clone)]
pub struct ExternalArchive {
    /// Stored solutions
    solutions: Vec<Array1<f64>>,
    /// Maximum archive size (typically 2.1 * initial_population or arc_rate * NP)
    max_size: usize,
}

impl ExternalArchive {
    /// Create a new empty archive
    pub fn new(max_size: usize) -> Self {
        Self {
            solutions: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Create archive with size proportional to population
    pub fn with_population_size(np: usize, arc_rate: f64) -> Self {
        let max_size = (arc_rate * np as f64).ceil() as usize;
        Self::new(max_size.max(1))
    }

    /// Add a solution to the archive
    /// If archive is full, removes a random solution
    pub fn add(&mut self, solution: Array1<f64>) {
        if self.solutions.len() < self.max_size {
            self.solutions.push(solution);
        } else if self.max_size > 0 {
            use rand::Rng;
            let mut rng = rand::rng();
            let idx = rng.random_range(0..self.max_size);
            self.solutions[idx] = solution;
        }
    }

    /// Select a random solution from the archive
    /// Returns None if archive is empty
    pub fn random_select<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Option<&Array1<f64>> {
        if self.solutions.is_empty() {
            None
        } else {
            let idx = rng.random_range(0..self.solutions.len());
            Some(&self.solutions[idx])
        }
    }

    /// Get a random index for archive selection
    /// Returns None if archive is empty, otherwise returns a valid index into the archive
    pub fn random_index<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Option<usize> {
        if self.solutions.is_empty() {
            None
        } else {
            Some(rng.random_range(0..self.solutions.len()))
        }
    }

    /// Get solution at index (used after random_index)
    pub fn get(&self, idx: usize) -> Option<&Array1<f64>> {
        self.solutions.get(idx)
    }

    /// Number of solutions in archive
    pub fn len(&self) -> usize {
        self.solutions.len()
    }

    /// Check if archive is empty
    pub fn is_empty(&self) -> bool {
        self.solutions.is_empty()
    }

    /// Clear the archive
    pub fn clear(&mut self) {
        self.solutions.clear();
    }

    /// Resize the archive (keeps existing solutions if new size is larger)
    pub fn resize(&mut self, new_max_size: usize) {
        self.max_size = new_max_size;
        if self.solutions.len() > new_max_size {
            self.solutions.truncate(new_max_size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_archive_basic() {
        let mut archive = ExternalArchive::new(5);
        assert!(archive.is_empty());

        archive.add(array![1.0, 2.0]);
        archive.add(array![3.0, 4.0]);
        assert_eq!(archive.len(), 2);
    }

    #[test]
    fn test_archive_full() {
        let mut archive = ExternalArchive::new(2);
        archive.add(array![1.0]);
        archive.add(array![2.0]);
        archive.add(array![3.0]);
        assert_eq!(archive.len(), 2);
    }

    #[test]
    fn test_archive_select() {
        let mut archive = ExternalArchive::new(5);
        archive.add(array![1.0, 2.0]);
        archive.add(array![3.0, 4.0]);

        let mut rng = rand::rng();
        let selected = archive.random_select(&mut rng);
        assert!(selected.is_some());
    }
}
