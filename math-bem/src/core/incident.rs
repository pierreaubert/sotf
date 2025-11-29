//! Incident field computation
//!
//! Computes incident acoustic fields for BEM excitation sources:
//! - Plane waves
//! - Point sources (monopoles)
//!
//! These are used to form the right-hand side of the BEM system.

use ndarray::{Array1, Array2};
use num_complex::Complex64;
use std::f64::consts::PI;

use crate::core::types::PhysicsParams;

/// Incident field source type
#[derive(Debug, Clone)]
pub enum IncidentField {
    /// Plane wave: p = A * exp(i k·x)
    PlaneWave {
        /// Direction of propagation (unit vector)
        direction: [f64; 3],
        /// Complex amplitude
        amplitude: Complex64,
    },

    /// Point source (monopole): p = A * exp(ikr) / (4πr)
    PointSource {
        /// Source position
        position: [f64; 3],
        /// Source strength (volume velocity amplitude)
        strength: Complex64,
    },

    /// Multiple plane waves
    MultiplePlaneWaves(Vec<([f64; 3], Complex64)>),

    /// Multiple point sources
    MultiplePointSources(Vec<([f64; 3], Complex64)>),
}

impl IncidentField {
    /// Create a plane wave with unit amplitude traveling in +z direction
    ///
    /// This matches the standard convention for Mie scattering theory where
    /// the incident wave travels toward +z (positive z direction).
    pub fn plane_wave_z() -> Self {
        IncidentField::PlaneWave {
            direction: [0.0, 0.0, 1.0],
            amplitude: Complex64::new(1.0, 0.0),
        }
    }

    /// Create a plane wave with unit amplitude traveling in -z direction
    pub fn plane_wave_neg_z() -> Self {
        IncidentField::PlaneWave {
            direction: [0.0, 0.0, -1.0],
            amplitude: Complex64::new(1.0, 0.0),
        }
    }

    /// Create a plane wave with specified direction and amplitude
    pub fn plane_wave(direction: [f64; 3], amplitude: f64) -> Self {
        // Normalize direction
        let len = (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2)).sqrt();
        let dir = if len > 1e-10 {
            [direction[0] / len, direction[1] / len, direction[2] / len]
        } else {
            [0.0, 0.0, -1.0]
        };

        IncidentField::PlaneWave {
            direction: dir,
            amplitude: Complex64::new(amplitude, 0.0),
        }
    }

    /// Create a point source at given position
    pub fn point_source(position: [f64; 3], strength: f64) -> Self {
        IncidentField::PointSource {
            position,
            strength: Complex64::new(strength, 0.0),
        }
    }

    /// Evaluate incident pressure at given points
    ///
    /// # Arguments
    /// * `points` - Evaluation points (N × 3 array)
    /// * `physics` - Physical parameters (contains wave number k)
    ///
    /// # Returns
    /// Complex pressure values at each point
    pub fn evaluate_pressure(
        &self,
        points: &Array2<f64>,
        physics: &PhysicsParams,
    ) -> Array1<Complex64> {
        let n = points.nrows();
        let k = physics.wave_number;
        let mut pressure = Array1::zeros(n);

        match self {
            IncidentField::PlaneWave {
                direction,
                amplitude,
            } => {
                for i in 0..n {
                    // k·x = k * (d·x)
                    let kdotx = k
                        * (direction[0] * points[[i, 0]]
                            + direction[1] * points[[i, 1]]
                            + direction[2] * points[[i, 2]]);

                    // p = A * exp(i k·x)
                    pressure[i] = *amplitude * Complex64::new(kdotx.cos(), kdotx.sin());
                }
            }

            IncidentField::PointSource { position, strength } => {
                for i in 0..n {
                    let dx = points[[i, 0]] - position[0];
                    let dy = points[[i, 1]] - position[1];
                    let dz = points[[i, 2]] - position[2];
                    let r = (dx * dx + dy * dy + dz * dz).sqrt();

                    if r > 1e-10 {
                        let kr = k * r;
                        // p = S * exp(ikr) / (4πr)
                        let green = Complex64::new(kr.cos(), kr.sin()) / (4.0 * PI * r);
                        pressure[i] = *strength * green;
                    }
                }
            }

            IncidentField::MultiplePlaneWaves(waves) => {
                for (direction, amplitude) in waves {
                    for i in 0..n {
                        let kdotx = k
                            * (direction[0] * points[[i, 0]]
                                + direction[1] * points[[i, 1]]
                                + direction[2] * points[[i, 2]]);
                        pressure[i] += *amplitude * Complex64::new(kdotx.cos(), kdotx.sin());
                    }
                }
            }

            IncidentField::MultiplePointSources(sources) => {
                for (position, strength) in sources {
                    for i in 0..n {
                        let dx = points[[i, 0]] - position[0];
                        let dy = points[[i, 1]] - position[1];
                        let dz = points[[i, 2]] - position[2];
                        let r = (dx * dx + dy * dy + dz * dz).sqrt();

                        if r > 1e-10 {
                            let kr = k * r;
                            let green = Complex64::new(kr.cos(), kr.sin()) / (4.0 * PI * r);
                            pressure[i] += *strength * green;
                        }
                    }
                }
            }
        }

        pressure
    }

    /// Evaluate incident velocity (∂p/∂n) at given points with normals
    ///
    /// # Arguments
    /// * `points` - Evaluation points (N × 3 array)
    /// * `normals` - Unit normal vectors at each point (N × 3 array)
    /// * `physics` - Physical parameters
    ///
    /// # Returns
    /// Complex normal velocity values (actually ∂p/∂n, not v_n)
    pub fn evaluate_normal_derivative(
        &self,
        points: &Array2<f64>,
        normals: &Array2<f64>,
        physics: &PhysicsParams,
    ) -> Array1<Complex64> {
        let n = points.nrows();
        let k = physics.wave_number;
        let mut dpdn = Array1::zeros(n);

        match self {
            IncidentField::PlaneWave {
                direction,
                amplitude,
            } => {
                for i in 0..n {
                    let kdotx = k
                        * (direction[0] * points[[i, 0]]
                            + direction[1] * points[[i, 1]]
                            + direction[2] * points[[i, 2]]);

                    // ∂p/∂n = ik (d·n) p
                    let kdotn = k
                        * (direction[0] * normals[[i, 0]]
                            + direction[1] * normals[[i, 1]]
                            + direction[2] * normals[[i, 2]]);

                    let p = *amplitude * Complex64::new(kdotx.cos(), kdotx.sin());
                    dpdn[i] = Complex64::new(0.0, kdotn) * p;
                }
            }

            IncidentField::PointSource { position, strength } => {
                for i in 0..n {
                    let dx = points[[i, 0]] - position[0];
                    let dy = points[[i, 1]] - position[1];
                    let dz = points[[i, 2]] - position[2];
                    let r = (dx * dx + dy * dy + dz * dz).sqrt();

                    if r > 1e-10 {
                        let kr = k * r;
                        let _r2 = r * r;

                        // ∂G/∂r = (ik - 1/r) * G
                        let exp_ikr = Complex64::new(kr.cos(), kr.sin());
                        let g = exp_ikr / (4.0 * PI * r);
                        let dgdr = (Complex64::new(0.0, k) - Complex64::new(1.0 / r, 0.0)) * g;

                        // ∂r/∂n = (x-x0)·n / r
                        let drdn =
                            (dx * normals[[i, 0]] + dy * normals[[i, 1]] + dz * normals[[i, 2]])
                                / r;

                        dpdn[i] = *strength * dgdr * drdn;
                    }
                }
            }

            IncidentField::MultiplePlaneWaves(waves) => {
                for (direction, amplitude) in waves {
                    for i in 0..n {
                        let kdotx = k
                            * (direction[0] * points[[i, 0]]
                                + direction[1] * points[[i, 1]]
                                + direction[2] * points[[i, 2]]);

                        let kdotn = k
                            * (direction[0] * normals[[i, 0]]
                                + direction[1] * normals[[i, 1]]
                                + direction[2] * normals[[i, 2]]);

                        let p = *amplitude * Complex64::new(kdotx.cos(), kdotx.sin());
                        dpdn[i] += Complex64::new(0.0, kdotn) * p;
                    }
                }
            }

            IncidentField::MultiplePointSources(sources) => {
                for (position, strength) in sources {
                    for i in 0..n {
                        let dx = points[[i, 0]] - position[0];
                        let dy = points[[i, 1]] - position[1];
                        let dz = points[[i, 2]] - position[2];
                        let r = (dx * dx + dy * dy + dz * dz).sqrt();

                        if r > 1e-10 {
                            let kr = k * r;
                            let exp_ikr = Complex64::new(kr.cos(), kr.sin());
                            let g = exp_ikr / (4.0 * PI * r);
                            let dgdr = (Complex64::new(0.0, k) - Complex64::new(1.0 / r, 0.0)) * g;
                            let drdn = (dx * normals[[i, 0]]
                                + dy * normals[[i, 1]]
                                + dz * normals[[i, 2]])
                                / r;

                            dpdn[i] += *strength * dgdr * drdn;
                        }
                    }
                }
            }
        }

        dpdn
    }

    /// Compute the right-hand side vector for BEM
    ///
    /// For exterior Neumann problem (rigid scatterer) using DIRECT formulation
    /// where the unknown is the total surface pressure p:
    ///
    /// The RHS comes from the incident field contribution to the integral equation.
    /// From NumCalc NC_IncidentWaveRHS:
    ///   RHS = -(γ*p_inc + β*τ*∂p_inc/∂n)
    ///
    /// For exterior problems: τ = +1, γ = 1
    /// For interior problems: τ = -1
    pub fn compute_rhs(
        &self,
        element_centers: &Array2<f64>,
        element_normals: &Array2<f64>,
        physics: &PhysicsParams,
        use_burton_miller: bool,
    ) -> Array1<Complex64> {
        if use_burton_miller {
            let beta = physics.burton_miller_beta();
            self.compute_rhs_with_beta(element_centers, element_normals, physics, beta)
        } else {
            // CBIE only: RHS = -γ*p_inc
            let gamma = Complex64::new(physics.gamma(), 0.0);
            let p_inc = self.evaluate_pressure(element_centers, physics);
            -gamma * p_inc
        }
    }

    /// Compute RHS with custom Burton-Miller coupling parameter
    ///
    /// From NumCalc NC_IncidentWaveRHS:
    ///   RHS = -(γ*p_inc + β*τ*∂p_inc/∂n)
    ///
    /// This formula matches the C++ implementation for proper BEM assembly.
    pub fn compute_rhs_with_beta(
        &self,
        element_centers: &Array2<f64>,
        element_normals: &Array2<f64>,
        physics: &PhysicsParams,
        beta: Complex64,
    ) -> Array1<Complex64> {
        // Get incident pressure at collocation points
        let p_inc = self.evaluate_pressure(element_centers, physics);

        // Get incident velocity (normal derivative) at collocation points
        let dpdn = self.evaluate_normal_derivative(element_centers, element_normals, physics);

        // Parameters from physics
        let gamma = Complex64::new(physics.gamma(), 0.0);
        let tau = Complex64::new(physics.tau, 0.0);

        // Burton-Miller RHS: -(γ*p_inc + β*τ*∂p_inc/∂n)
        // This matches NC_IncidentWaveRHS: zrc -= zg*Gama3 + zv*(zBta3*Tao_)
        let mut rhs = Array1::zeros(element_centers.nrows());
        for i in 0..rhs.len() {
            rhs[i] = -(gamma * p_inc[i] + beta * tau * dpdn[i]);
        }

        rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_physics(k: f64) -> PhysicsParams {
        let c = 343.0;
        let freq = k * c / (2.0 * PI);
        PhysicsParams::new(freq, c, 1.21, false)
    }

    #[test]
    fn test_plane_wave_on_axis() {
        let incident = IncidentField::plane_wave([0.0, 0.0, 1.0], 1.0);
        let physics = make_physics(1.0);

        // Points on z-axis
        let points =
            Array2::from_shape_vec((3, 3), vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0])
                .unwrap();

        let p = incident.evaluate_pressure(&points, &physics);

        // At z=0: exp(0) = 1
        assert!((p[0].re - 1.0).abs() < 1e-10);
        assert!(p[0].im.abs() < 1e-10);

        // At z=1: exp(ik) = exp(i) = cos(1) + i*sin(1)
        // Since direction is +z, kdotx = k*z = 1 at z=1
        assert!((p[1].re - (1.0_f64).cos()).abs() < 1e-10);
        assert!((p[1].im - (1.0_f64).sin()).abs() < 1e-10);
    }

    #[test]
    fn test_point_source_decay() {
        let incident = IncidentField::point_source([0.0, 0.0, 0.0], 1.0);
        let physics = make_physics(1.0);

        // Points at different distances
        let points =
            Array2::from_shape_vec((3, 3), vec![1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 4.0, 0.0, 0.0])
                .unwrap();

        let p = incident.evaluate_pressure(&points, &physics);

        // Pressure should decay as 1/r
        let ratio_1_2 = p[0].norm() / p[1].norm();
        let ratio_2_4 = p[1].norm() / p[2].norm();

        assert!((ratio_1_2 - 2.0).abs() < 0.1);
        assert!((ratio_2_4 - 2.0).abs() < 0.1);
    }

    #[test]
    fn test_plane_wave_normal_derivative() {
        let incident = IncidentField::plane_wave([0.0, 0.0, 1.0], 1.0);
        let physics = make_physics(1.0);

        // Point with normal pointing in +z direction (along wave direction)
        let points = Array2::from_shape_vec((1, 3), vec![0.0, 0.0, 0.0]).unwrap();
        let normals = Array2::from_shape_vec((1, 3), vec![0.0, 0.0, 1.0]).unwrap();

        let dpdn = incident.evaluate_normal_derivative(&points, &normals, &physics);

        // ∂p/∂n = ik (d·n) p = ik * (+1) * 1 = +ik
        assert!(dpdn[0].re.abs() < 1e-10);
        assert!((dpdn[0].im - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_rhs_computation() {
        let incident = IncidentField::plane_wave([0.0, 0.0, 1.0], 1.0);
        let physics = make_physics(1.0);

        let centers = Array2::from_shape_vec((1, 3), vec![0.0, 0.0, 1.0]).unwrap();
        let normals = Array2::from_shape_vec((1, 3), vec![0.0, 0.0, 1.0]).unwrap();

        let rhs = incident.compute_rhs(&centers, &normals, &physics, false);

        // RHS = -∂p_inc/∂n, should be non-zero
        assert!(rhs[0].norm() > 0.0);
    }
}
