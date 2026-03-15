//! Color Schemes (d3-scale-chromatic)
//!
//! Sequential and diverging color schemes with perceptually uniform interpolation.

use super::D3Color;
use crate::color::hcl::Hcl;

pub struct SequentialScale {
    colors: Vec<Hcl>,
    name: &'static str,
}

impl SequentialScale {
    pub fn new(colors: Vec<Hcl>, name: &'static str) -> Self {
        Self { colors, name }
    }

    pub fn get(&self, t: f64) -> D3Color {
        let t = t.clamp(0.0, 1.0);

        if self.colors.is_empty() {
            return D3Color::rgb(0, 0, 0);
        }

        if self.colors.len() == 1 {
            return self.colors[0].to_rgb();
        }

        let scaled_t = t * (self.colors.len() - 1) as f64;
        let i = scaled_t.floor() as usize;
        let frac = scaled_t.fract();

        if i >= self.colors.len() - 1 {
            return self.colors.last().unwrap().to_rgb();
        }

        self.colors[i]
            .interpolate(&self.colors[i + 1], frac)
            .to_rgb()
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn sample(&self, n: usize) -> Vec<D3Color> {
        (0..n)
            .map(|i| self.get(i as f64 / (n - 1).max(1) as f64))
            .collect()
    }
}

pub struct DivergingScale {
    negative: Vec<Hcl>,
    neutral: Hcl,
    positive: Vec<Hcl>,
    name: &'static str,
}

impl DivergingScale {
    pub fn new(negative: Vec<Hcl>, neutral: Hcl, positive: Vec<Hcl>, name: &'static str) -> Self {
        Self {
            negative,
            neutral,
            positive,
            name,
        }
    }

    pub fn get(&self, t: f64) -> D3Color {
        let t = t.clamp(0.0, 1.0);

        if t <= 0.5 {
            // Map [0, 0.5] → interpolate through [negative[0], ..., negative[n-1], neutral]
            let stops: Vec<&Hcl> = self.negative.iter().chain(std::iter::once(&self.neutral)).collect();
            Self::interpolate_stops(&stops, t * 2.0)
        } else {
            // Map (0.5, 1.0] → interpolate through [neutral, positive[0], ..., positive[n-1]]
            let stops: Vec<&Hcl> = std::iter::once(&self.neutral).chain(self.positive.iter()).collect();
            Self::interpolate_stops(&stops, (t - 0.5) * 2.0)
        }
    }

    fn interpolate_stops(stops: &[&Hcl], t: f64) -> D3Color {
        if stops.is_empty() {
            return D3Color::rgb(0, 0, 0);
        }
        if stops.len() == 1 {
            return stops[0].to_rgb();
        }
        let scaled = t * (stops.len() - 1) as f64;
        let i = scaled.floor() as usize;
        let frac = scaled.fract();
        if i >= stops.len() - 1 {
            return stops.last().unwrap().to_rgb();
        }
        stops[i].interpolate(stops[i + 1], frac).to_rgb()
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn sample(&self, n: usize) -> Vec<D3Color> {
        (0..n)
            .map(|i| self.get(i as f64 / (n - 1).max(1) as f64))
            .collect()
    }
}

pub struct SequentialScheme;

impl SequentialScheme {
    pub fn blues() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(220.0, 8.0, 97.0),
                Hcl::new(217.0, 26.0, 91.0),
                Hcl::new(212.0, 42.0, 84.0),
                Hcl::new(207.0, 54.0, 76.0),
                Hcl::new(202.0, 62.0, 67.0),
                Hcl::new(197.0, 70.0, 56.0),
                Hcl::new(192.0, 78.0, 44.0),
                Hcl::new(187.0, 85.0, 32.0),
                Hcl::new(182.0, 92.0, 20.0),
                Hcl::new(177.0, 98.0, 10.0),
            ],
            "Blues",
        )
    }

    pub fn greens() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(150.0, 8.0, 97.0),
                Hcl::new(153.0, 22.0, 91.0),
                Hcl::new(156.0, 35.0, 84.0),
                Hcl::new(160.0, 47.0, 76.0),
                Hcl::new(165.0, 57.0, 67.0),
                Hcl::new(170.0, 65.0, 56.0),
                Hcl::new(175.0, 73.0, 44.0),
                Hcl::new(180.0, 80.0, 32.0),
                Hcl::new(185.0, 87.0, 20.0),
                Hcl::new(190.0, 94.0, 10.0),
            ],
            "Greens",
        )
    }

    pub fn reds() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(0.0, 8.0, 97.0),
                Hcl::new(3.0, 25.0, 91.0),
                Hcl::new(7.0, 40.0, 84.0),
                Hcl::new(12.0, 52.0, 76.0),
                Hcl::new(18.0, 62.0, 67.0),
                Hcl::new(25.0, 70.0, 56.0),
                Hcl::new(32.0, 77.0, 44.0),
                Hcl::new(40.0, 83.0, 32.0),
                Hcl::new(50.0, 88.0, 20.0),
                Hcl::new(60.0, 93.0, 10.0),
            ],
            "Reds",
        )
    }

    pub fn purples() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(270.0, 8.0, 97.0),
                Hcl::new(266.0, 22.0, 91.0),
                Hcl::new(262.0, 35.0, 84.0),
                Hcl::new(258.0, 47.0, 76.0),
                Hcl::new(254.0, 57.0, 67.0),
                Hcl::new(250.0, 65.0, 56.0),
                Hcl::new(246.0, 73.0, 44.0),
                Hcl::new(242.0, 80.0, 32.0),
                Hcl::new(238.0, 87.0, 20.0),
                Hcl::new(234.0, 94.0, 10.0),
            ],
            "Purples",
        )
    }

    pub fn oranges() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(35.0, 8.0, 97.0),
                Hcl::new(33.0, 22.0, 91.0),
                Hcl::new(31.0, 35.0, 84.0),
                Hcl::new(29.0, 47.0, 76.0),
                Hcl::new(28.0, 57.0, 67.0),
                Hcl::new(27.0, 65.0, 56.0),
                Hcl::new(26.0, 73.0, 44.0),
                Hcl::new(25.0, 80.0, 32.0),
                Hcl::new(24.0, 87.0, 20.0),
                Hcl::new(23.0, 94.0, 10.0),
            ],
            "Oranges",
        )
    }

    pub fn viridis() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(276.0, 68.0, 6.0),
                Hcl::new(264.0, 65.0, 17.0),
                Hcl::new(253.0, 62.0, 24.0),
                Hcl::new(242.0, 58.0, 30.0),
                Hcl::new(232.0, 53.0, 36.0),
                Hcl::new(222.0, 49.0, 42.0),
                Hcl::new(212.0, 47.0, 49.0),
                Hcl::new(202.0, 48.0, 55.0),
                Hcl::new(192.0, 52.0, 62.0),
                Hcl::new(181.0, 59.0, 69.0),
                Hcl::new(170.0, 68.0, 77.0),
                Hcl::new(158.0, 78.0, 85.0),
                Hcl::new(145.0, 89.0, 92.0),
            ],
            "Viridis",
        )
    }

    pub fn magma() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(320.0, 72.0, 1.0),
                Hcl::new(310.0, 73.0, 8.0),
                Hcl::new(300.0, 72.0, 14.0),
                Hcl::new(290.0, 70.0, 20.0),
                Hcl::new(280.0, 68.0, 26.0),
                Hcl::new(270.0, 65.0, 33.0),
                Hcl::new(260.0, 62.0, 41.0),
                Hcl::new(250.0, 59.0, 50.0),
                Hcl::new(40.0, 65.0, 61.0),
                Hcl::new(45.0, 70.0, 72.0),
                Hcl::new(50.0, 75.0, 82.0),
                Hcl::new(55.0, 80.0, 91.0),
                Hcl::new(60.0, 85.0, 98.0),
            ],
            "Magma",
        )
    }

    pub fn inferno() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(260.0, 72.0, 1.0),
                Hcl::new(250.0, 72.0, 6.0),
                Hcl::new(40.0, 70.0, 12.0),
                Hcl::new(35.0, 68.0, 18.0),
                Hcl::new(30.0, 66.0, 25.0),
                Hcl::new(25.0, 64.0, 33.0),
                Hcl::new(20.0, 62.0, 41.0),
                Hcl::new(15.0, 60.0, 50.0),
                Hcl::new(10.0, 58.0, 59.0),
                Hcl::new(5.0, 55.0, 68.0),
                Hcl::new(55.0, 50.0, 77.0),
                Hcl::new(60.0, 45.0, 86.0),
                Hcl::new(65.0, 40.0, 98.0),
            ],
            "Inferno",
        )
    }

    pub fn plasma() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(280.0, 72.0, 3.0),
                Hcl::new(270.0, 70.0, 12.0),
                Hcl::new(260.0, 68.0, 19.0),
                Hcl::new(250.0, 66.0, 26.0),
                Hcl::new(240.0, 64.0, 33.0),
                Hcl::new(230.0, 62.0, 40.0),
                Hcl::new(220.0, 60.0, 48.0),
                Hcl::new(210.0, 60.0, 56.0),
                Hcl::new(200.0, 62.0, 64.0),
                Hcl::new(85.0, 68.0, 72.0),
                Hcl::new(80.0, 75.0, 80.0),
                Hcl::new(75.0, 82.0, 89.0),
                Hcl::new(70.0, 88.0, 98.0),
            ],
            "Plasma",
        )
    }

    pub fn turbo() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(260.0, 82.0, 10.0),
                Hcl::new(240.0, 75.0, 20.0),
                Hcl::new(210.0, 70.0, 30.0),
                Hcl::new(180.0, 65.0, 40.0),
                Hcl::new(150.0, 60.0, 50.0),
                Hcl::new(120.0, 55.0, 60.0),
                Hcl::new(90.0, 50.0, 70.0),
                Hcl::new(60.0, 55.0, 80.0),
                Hcl::new(40.0, 70.0, 90.0),
                Hcl::new(20.0, 85.0, 97.0),
            ],
            "Turbo",
        )
    }

    pub fn bu_pu() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(250.0, 5.0, 95.0),
                Hcl::new(260.0, 25.0, 80.0),
                Hcl::new(270.0, 50.0, 55.0),
                Hcl::new(280.0, 60.0, 35.0),
                Hcl::new(290.0, 50.0, 15.0),
            ],
            "BuPu",
        )
    }

    pub fn cubehelix() -> SequentialScale {
        SequentialScale::new(
            vec![
                Hcl::new(260.0, 80.0, 5.0),
                Hcl::new(250.0, 75.0, 14.0),
                Hcl::new(240.0, 70.0, 23.0),
                Hcl::new(230.0, 65.0, 32.0),
                Hcl::new(220.0, 60.0, 41.0),
                Hcl::new(210.0, 55.0, 50.0),
                Hcl::new(200.0, 50.0, 59.0),
                Hcl::new(190.0, 45.0, 68.0),
                Hcl::new(180.0, 40.0, 77.0),
                Hcl::new(170.0, 35.0, 86.0),
                Hcl::new(160.0, 30.0, 95.0),
            ],
            "Cubehelix",
        )
    }

    pub fn get(name: &str) -> Option<SequentialScale> {
        match name {
            "Blues" => Some(Self::blues()),
            "Greens" => Some(Self::greens()),
            "Reds" => Some(Self::reds()),
            "Purples" => Some(Self::purples()),
            "Oranges" => Some(Self::oranges()),
            "Viridis" => Some(Self::viridis()),
            "Magma" => Some(Self::magma()),
            "Inferno" => Some(Self::inferno()),
            "Plasma" => Some(Self::plasma()),
            "Turbo" => Some(Self::turbo()),
            "BuPu" => Some(Self::bu_pu()),
            "Cubehelix" => Some(Self::cubehelix()),
            _ => None,
        }
    }
}

pub struct DivergingScheme;

impl DivergingScheme {
    pub fn rd_bu() -> DivergingScale {
        DivergingScale::new(
            vec![
                Hcl::new(0.0, 75.0, 35.0),
                Hcl::new(2.0, 68.0, 42.0),
                Hcl::new(5.0, 60.0, 50.0),
                Hcl::new(10.0, 50.0, 60.0),
                Hcl::new(15.0, 38.0, 72.0),
                Hcl::new(20.0, 25.0, 85.0),
            ],
            Hcl::new(0.0, 0.0, 97.0),
            vec![
                Hcl::new(220.0, 25.0, 85.0),
                Hcl::new(225.0, 38.0, 72.0),
                Hcl::new(230.0, 50.0, 60.0),
                Hcl::new(235.0, 60.0, 50.0),
                Hcl::new(240.0, 68.0, 42.0),
                Hcl::new(245.0, 75.0, 35.0),
            ],
            "RdBu",
        )
    }

    pub fn rd_yl_bu() -> DivergingScale {
        DivergingScale::new(
            vec![
                Hcl::new(0.0, 75.0, 35.0),
                Hcl::new(5.0, 65.0, 45.0),
                Hcl::new(15.0, 55.0, 55.0),
                Hcl::new(30.0, 45.0, 65.0),
                Hcl::new(45.0, 35.0, 75.0),
                Hcl::new(55.0, 20.0, 88.0),
            ],
            Hcl::new(50.0, 0.0, 97.0),
            vec![
                Hcl::new(210.0, 20.0, 88.0),
                Hcl::new(220.0, 35.0, 75.0),
                Hcl::new(235.0, 45.0, 65.0),
                Hcl::new(250.0, 55.0, 55.0),
                Hcl::new(265.0, 65.0, 45.0),
                Hcl::new(280.0, 75.0, 35.0),
            ],
            "RdYlBu",
        )
    }

    pub fn rd_yl_gn() -> DivergingScale {
        DivergingScale::new(
            vec![
                Hcl::new(0.0, 75.0, 35.0),
                Hcl::new(5.0, 65.0, 45.0),
                Hcl::new(25.0, 55.0, 55.0),
                Hcl::new(50.0, 45.0, 65.0),
                Hcl::new(90.0, 35.0, 75.0),
                Hcl::new(120.0, 20.0, 88.0),
            ],
            Hcl::new(80.0, 0.0, 97.0),
            vec![
                Hcl::new(140.0, 20.0, 88.0),
                Hcl::new(150.0, 35.0, 75.0),
                Hcl::new(160.0, 45.0, 65.0),
                Hcl::new(170.0, 55.0, 55.0),
                Hcl::new(180.0, 65.0, 45.0),
                Hcl::new(190.0, 75.0, 35.0),
            ],
            "RdYlGn",
        )
    }

    pub fn pi_yg() -> DivergingScale {
        DivergingScale::new(
            vec![
                Hcl::new(330.0, 70.0, 40.0),
                Hcl::new(335.0, 60.0, 50.0),
                Hcl::new(340.0, 50.0, 60.0),
                Hcl::new(345.0, 38.0, 72.0),
                Hcl::new(350.0, 25.0, 85.0),
            ],
            Hcl::new(0.0, 0.0, 97.0),
            vec![
                Hcl::new(150.0, 25.0, 85.0),
                Hcl::new(145.0, 38.0, 72.0),
                Hcl::new(140.0, 50.0, 60.0),
                Hcl::new(135.0, 60.0, 50.0),
                Hcl::new(130.0, 70.0, 40.0),
            ],
            "PiYG",
        )
    }

    pub fn br_bg() -> DivergingScale {
        DivergingScale::new(
            vec![
                Hcl::new(25.0, 70.0, 40.0),
                Hcl::new(35.0, 60.0, 50.0),
                Hcl::new(50.0, 48.0, 62.0),
                Hcl::new(75.0, 35.0, 75.0),
                Hcl::new(120.0, 20.0, 88.0),
            ],
            Hcl::new(90.0, 0.0, 97.0),
            vec![
                Hcl::new(200.0, 20.0, 88.0),
                Hcl::new(210.0, 35.0, 75.0),
                Hcl::new(220.0, 48.0, 62.0),
                Hcl::new(235.0, 60.0, 50.0),
                Hcl::new(250.0, 70.0, 40.0),
            ],
            "BrBG",
        )
    }

    pub fn pu_or() -> DivergingScale {
        DivergingScale::new(
            vec![
                Hcl::new(280.0, 70.0, 40.0),
                Hcl::new(285.0, 60.0, 50.0),
                Hcl::new(290.0, 50.0, 62.0),
                Hcl::new(295.0, 38.0, 75.0),
                Hcl::new(300.0, 25.0, 88.0),
            ],
            Hcl::new(45.0, 0.0, 97.0),
            vec![
                Hcl::new(30.0, 25.0, 88.0),
                Hcl::new(25.0, 38.0, 75.0),
                Hcl::new(20.0, 50.0, 62.0),
                Hcl::new(15.0, 60.0, 50.0),
                Hcl::new(10.0, 70.0, 40.0),
            ],
            "PuOr",
        )
    }

    pub fn spectral() -> DivergingScale {
        DivergingScale::new(
            vec![
                Hcl::new(0.0, 75.0, 35.0),
                Hcl::new(20.0, 65.0, 45.0),
                Hcl::new(45.0, 55.0, 55.0),
                Hcl::new(70.0, 45.0, 65.0),
                Hcl::new(100.0, 40.0, 75.0),
                Hcl::new(130.0, 30.0, 85.0),
            ],
            Hcl::new(80.0, 0.0, 97.0),
            vec![
                Hcl::new(180.0, 30.0, 85.0),
                Hcl::new(210.0, 40.0, 75.0),
                Hcl::new(240.0, 45.0, 65.0),
                Hcl::new(270.0, 55.0, 55.0),
                Hcl::new(300.0, 65.0, 45.0),
                Hcl::new(330.0, 75.0, 35.0),
            ],
            "Spectral",
        )
    }

    pub fn get(name: &str) -> Option<DivergingScale> {
        match name {
            "RdBu" => Some(Self::rd_bu()),
            "RdYlBu" => Some(Self::rd_yl_bu()),
            "RdYlGn" => Some(Self::rd_yl_gn()),
            "PiYG" => Some(Self::pi_yg()),
            "BrBG" => Some(Self::br_bg()),
            "PuOr" => Some(Self::pu_or()),
            "Spectral" => Some(Self::spectral()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequential_scale_get() {
        let scale = SequentialScheme::blues();
        let c = scale.get(0.0);
        assert!(c.r >= 0.0 && c.r <= 1.0);

        let c = scale.get(1.0);
        assert!(c.r >= 0.0 && c.r <= 1.0);
    }

    #[test]
    fn test_sequential_scale_midpoint() {
        let scale = SequentialScheme::viridis();
        let c = scale.get(0.5);
        assert!(c.r >= 0.0 && c.r <= 1.0);
        assert!(c.g >= 0.0 && c.g <= 1.0);
        assert!(c.b >= 0.0 && c.b <= 1.0);
    }

    #[test]
    fn test_sequential_sample() {
        let scale = SequentialScheme::greens();
        let samples = scale.sample(5);
        assert_eq!(samples.len(), 5);
    }

    #[test]
    fn test_diverging_scale_get() {
        let scale = DivergingScheme::rd_bu();
        let c = scale.get(0.0);
        assert!(c.r >= 0.0 && c.r <= 1.0);

        let c = scale.get(0.5);
        assert!(c.r >= 0.0 && c.r <= 1.0);
        assert!(c.g >= 0.0 && c.g <= 1.0);
        assert!(c.b >= 0.0 && c.b <= 1.0);

        let c = scale.get(1.0);
        assert!(c.r >= 0.0 && c.r <= 1.0);
    }

    #[test]
    fn test_diverging_sample() {
        let scale = DivergingScheme::spectral();
        let samples = scale.sample(5);
        assert_eq!(samples.len(), 5);
    }

    #[test]
    fn test_sequential_get_by_name() {
        let scale = SequentialScheme::get("Viridis").unwrap();
        let c = scale.get(0.5);
        assert!(c.r >= 0.0 && c.r <= 1.0);
    }

    #[test]
    fn test_diverging_get_by_name() {
        let scale = DivergingScheme::get("RdBu").unwrap();
        let c = scale.get(0.5);
        assert!(c.r >= 0.0 && c.r <= 1.0);
    }

    #[test]
    fn test_sequential_scale_bounds() {
        let scale = SequentialScheme::magma();
        let c0 = scale.get(-0.5);
        let c1 = scale.get(1.5);
        let c_mid = scale.get(0.5);

        assert!(c0.r >= 0.0 && c0.r <= 1.0);
        assert!(c1.r >= 0.0 && c1.r <= 1.0);
        assert!(c_mid.r >= 0.0 && c_mid.r <= 1.0);
    }

    #[test]
    fn test_diverging_scale_bounds() {
        let scale = DivergingScheme::pi_yg();
        let c0 = scale.get(-0.5);
        let c1 = scale.get(1.5);
        let c_mid = scale.get(0.5);

        assert!(c0.r >= 0.0 && c0.r <= 1.0);
        assert!(c1.r >= 0.0 && c1.r <= 1.0);
        assert!(c_mid.r >= 0.0 && c_mid.r <= 1.0);
    }
}
