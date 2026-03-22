//! Plugin parameter editing interface shared between TUI and GPUI

use crate::PluginSettings;
use std::path::PathBuf;

/// Specification for a plugin parameter in the TUI
pub struct TuiParamSpec {
    pub name: String,
    pub value: String,
    pub unit: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TuiParamType {
    Float { min: f64, max: f64, step: f64 },
    Int { min: i32, max: i32, step: i32 },
    Bool,
    Choice { count: usize },
}

pub struct TuiParamDescriptor {
    pub name: String,
    pub param_type: TuiParamType,
    pub unit: String,
    pub group: String,
    pub doc: String,
}

pub trait TuiEditablePlugin {
    fn get_descriptors(&self) -> Vec<TuiParamDescriptor>;
    fn get_params(&self) -> Vec<TuiParamSpec>;
    fn adjust_param(&mut self, index: usize, delta: f64) -> bool;
    fn get_value_as_string(&self, index: usize) -> String;
    /// Return the list of choice labels for a Choice parameter.
    /// Returns empty vec for non-Choice params.
    fn get_choice_labels(&self, _index: usize) -> Vec<String> {
        Vec::new()
    }
    /// Set a parameter to an absolute value.
    /// For Float/Int: computes the delta from current value and calls adjust_param.
    /// For Bool: value > 0.5 = true.
    /// For Choice: value is the option index.
    fn set_param(&mut self, index: usize, value: f64) -> bool {
        let current_str = self.get_value_as_string(index);
        let current = current_str.parse::<f64>().unwrap_or(0.0);
        let descriptors = self.get_descriptors();
        if let Some(desc) = descriptors.get(index) {
            match desc.param_type {
                TuiParamType::Float { step, .. } => {
                    // Compute delta in units of what adjust_param expects (delta=1.0 means +step)
                    let delta = (value - current) / step;
                    if delta.abs() > 0.001 {
                        return self.adjust_param(index, delta);
                    }
                }
                TuiParamType::Int { step, .. } => {
                    let delta = (value - current) / step as f64;
                    if delta.abs() > 0.001 {
                        return self.adjust_param(index, delta);
                    }
                }
                TuiParamType::Bool => {
                    let is_true = current_str == "On"
                        || current_str == "true"
                        || current_str == "Linked"
                        || current_str == "Soft"
                        || current_str == "1";
                    let want_true = value > 0.5;
                    if is_true != want_true {
                        return self.adjust_param(index, 1.0);
                    }
                }
                TuiParamType::Choice { count } => {
                    let target = (value as usize).min(count.saturating_sub(1));
                    // Cycle forward until we reach the target
                    for _ in 0..count {
                        let cur = self.get_value_as_string(index);
                        let labels = self.get_choice_labels(index);
                        if let Some(cur_idx) = labels.iter().position(|l| *l == cur) {
                            if cur_idx == target {
                                return true;
                            }
                        }
                        self.adjust_param(index, 1.0);
                    }
                }
            }
        }
        false
    }
}

fn spec_to_descriptor(spec: &sotf_plugins::param_specs::ParamSpec) -> TuiParamDescriptor {
    use sotf_plugins::param_specs::ParamType;
    TuiParamDescriptor {
        name: spec.name.to_string(),
        param_type: match spec.param_type {
            ParamType::Float { min, max, step, .. } => TuiParamType::Float { min, max, step },
            ParamType::Int { min, max, step, .. } => TuiParamType::Int {
                min: min as i32,
                max: max as i32,
                step: step as i32,
            },
            ParamType::Bool { .. } => TuiParamType::Bool,
            ParamType::Choice { labels, .. } => TuiParamType::Choice {
                count: labels.len(),
            },
            ParamType::FilePath => TuiParamType::Choice { count: 0 },
        },
        unit: spec.unit.to_string(),
        group: spec.group.to_string(),
        doc: spec.doc.to_string(),
    }
}

fn specs_to_descriptors(specs: &[sotf_plugins::param_specs::ParamSpec]) -> Vec<TuiParamDescriptor> {
    specs.iter().map(spec_to_descriptor).collect()
}

impl TuiEditablePlugin for PluginSettings {
    fn get_descriptors(&self) -> Vec<TuiParamDescriptor> {
        use sotf_plugins::param_specs;
        match self {
            // Dynamic-param plugins: global + per-band from BAND_TEMPLATE
            PluginSettings::EQ { max_filters, .. } => {
                let mut descs = specs_to_descriptors(param_specs::eq::GLOBAL_PARAMS);
                for i in 0..*max_filters {
                    let g = format!("Filter {}", i + 1);
                    for spec in param_specs::eq::BAND_TEMPLATE {
                        let mut d = spec_to_descriptor(spec);
                        d.group = g.clone();
                        descs.push(d);
                    }
                }
                descs
            }
            PluginSettings::MultibandCompressor { num_bands, .. } => {
                let mut descs =
                    specs_to_descriptors(param_specs::multiband_compressor::GLOBAL_PARAMS);
                for i in 0..*num_bands {
                    let g = format!("Band {}", i + 1);
                    for spec in param_specs::multiband_compressor::BAND_TEMPLATE {
                        let mut d = spec_to_descriptor(spec);
                        d.group = g.clone();
                        descs.push(d);
                    }
                }
                descs
            }
            PluginSettings::MultibandExpander { num_bands, .. } => {
                let mut descs =
                    specs_to_descriptors(param_specs::multiband_expander::GLOBAL_PARAMS);
                for i in 0..*num_bands {
                    let g = format!("Band {}", i + 1);
                    for spec in param_specs::multiband_expander::BAND_TEMPLATE {
                        let mut d = spec_to_descriptor(spec);
                        d.group = g.clone();
                        descs.push(d);
                    }
                }
                descs
            }
            // All other plugins: generic from ParamSpec
            _ => specs_to_descriptors(self.param_specs()),
        }
    }

    fn get_params(&self) -> Vec<TuiParamSpec> {
        let descriptors = self.get_descriptors();
        let mut params = Vec::with_capacity(descriptors.len());
        for (i, desc) in descriptors.into_iter().enumerate() {
            params.push(TuiParamSpec {
                name: desc.name,
                value: self.get_value_as_string(i),
                unit: desc.unit,
            });
        }
        params
    }

    fn get_value_as_string(&self, index: usize) -> String {
        use sotf_plugins::param_specs::ParamType;
        match self {
            PluginSettings::EQ {
                filters,
                max_filters,
                ..
            } => {
                if index == 0 {
                    return format!("{}", max_filters);
                }
                let filter_offset = index - 1;
                let filter_idx = filter_offset / 4;
                let param_idx = filter_offset % 4;
                if let Some(filter) = filters.get(filter_idx) {
                    match param_idx {
                        0 => format!("{:.0}", filter.frequency),
                        1 => format!("{:.2}", filter.q),
                        2 => format!("{:.1}", filter.gain_db),
                        3 => format!("{:?}", filter.filter_type),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                }
            }
            PluginSettings::MultibandCompressor {
                num_bands,
                crossover_preset,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                mix,
                link_channels,
                bands,
                ..
            } => {
                let global_specs = sotf_plugins::param_specs::multiband_compressor::GLOBAL_PARAMS;
                let band_template = sotf_plugins::param_specs::multiband_compressor::BAND_TEMPLATE;
                let global_count = global_specs.len();
                let band_params = band_template.len();
                if index < global_count {
                    let val = match index {
                        0 => return format!("{}", num_bands),
                        1 => return format!("{}", crossover_preset),
                        2 => *crossover_freq_1,
                        3 => *crossover_freq_2,
                        4 => *crossover_freq_3,
                        5 => *crossover_freq_4,
                        6 => *threshold_db,
                        7 => *ratio,
                        8 => *attack_ms,
                        9 => *release_ms,
                        10 => *knee_db,
                        11 => *mix,
                        12 => {
                            return global_specs[12].format_value(if *link_channels {
                                1.0
                            } else {
                                0.0
                            });
                        }
                        _ => return String::new(),
                    };
                    global_specs[index].format_value(val)
                } else {
                    let band_offset = index - global_count;
                    let band_idx = band_offset / band_params;
                    let param_in_band = band_offset % band_params;
                    if let Some(band) = bands.get(band_idx) {
                        match param_in_band {
                            0 => band_template[0].format_value(if band.solo { 1.0 } else { 0.0 }),
                            1 => band_template[1].format_value(if band.bypass { 1.0 } else { 0.0 }),
                            2 => band
                                .threshold_db
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            3 => band
                                .ratio
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            4 => band
                                .attack_ms
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            5 => band
                                .release_ms
                                .map(|v| format!("{:.0}", v))
                                .unwrap_or("Global".into()),
                            6 => band
                                .knee_db
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            7 => format!("{:.1}", band.makeup_gain_db),
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    }
                }
            }
            PluginSettings::MultibandExpander {
                num_bands,
                crossover_preset,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                bands,
                ..
            } => {
                let global_specs = sotf_plugins::param_specs::multiband_expander::GLOBAL_PARAMS;
                let band_template = sotf_plugins::param_specs::multiband_expander::BAND_TEMPLATE;
                let global_count = global_specs.len();
                let band_params = band_template.len();
                if index < global_count {
                    let val = match index {
                        0 => return format!("{}", num_bands),
                        1 => return format!("{}", crossover_preset),
                        2 => *crossover_freq_1,
                        3 => *crossover_freq_2,
                        4 => *crossover_freq_3,
                        5 => *crossover_freq_4,
                        6 => *threshold_db,
                        7 => *ratio,
                        8 => *attack_ms,
                        9 => *release_ms,
                        10 => *range_db,
                        11 => *knee_db,
                        12 => *hysteresis_db,
                        13 => *hold_ms,
                        14 => *mix,
                        15 => {
                            return global_specs[15].format_value(if *link_channels {
                                1.0
                            } else {
                                0.0
                            });
                        }
                        _ => return String::new(),
                    };
                    global_specs[index].format_value(val)
                } else {
                    let band_offset = index - global_count;
                    let band_idx = band_offset / band_params;
                    let param_in_band = band_offset % band_params;
                    if let Some(band) = bands.get(band_idx) {
                        match param_in_band {
                            0 => band_template[0].format_value(if band.solo { 1.0 } else { 0.0 }),
                            1 => band_template[1].format_value(if band.bypass { 1.0 } else { 0.0 }),
                            2 => band
                                .threshold_db
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            3 => band
                                .ratio
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            4 => band
                                .attack_ms
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            5 => band
                                .release_ms
                                .map(|v| format!("{:.0}", v))
                                .unwrap_or("Global".into()),
                            6 => band
                                .range_db
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            7 => band
                                .knee_db
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            8 => band
                                .hysteresis_db
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or("Global".into()),
                            9 => band
                                .hold_ms
                                .map(|v| format!("{:.0}", v))
                                .unwrap_or("Global".into()),
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    }
                }
            }
            // Generic: all other plugins use ParamSpec
            _ => {
                let specs = self.param_specs();
                if let Some(spec) = specs.get(index) {
                    match spec.param_type {
                        ParamType::FilePath => match self.param_value_string(index) {
                            Some(path) if !path.is_empty() => PathBuf::from(&path)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            _ => "None".to_string(),
                        },
                        _ => {
                            if let Some(value) = self.param_value(index) {
                                spec.format_value(value)
                            } else {
                                String::new()
                            }
                        }
                    }
                } else {
                    String::new()
                }
            }
        }
    }

    fn adjust_param(&mut self, index: usize, delta: f64) -> bool {
        use sotf_plugins::param_specs::ParamType;
        match self {
            PluginSettings::EQ {
                filters,
                max_filters,
                ..
            } => {
                if index == 0 {
                    let old_max = *max_filters;
                    *max_filters = ((*max_filters as i64) + delta as i64).clamp(1, 20) as usize;
                    if *max_filters > old_max {
                        while filters.len() < *max_filters {
                            filters.push(crate::EQFilter::new(
                                crate::BiquadFilterType::Peak,
                                1000.0,
                                1.0,
                                0.0,
                            ));
                        }
                    } else if *max_filters < old_max {
                        filters.truncate(*max_filters);
                    }
                    return true;
                }
                let filter_offset = index - 1;
                let filter_idx = filter_offset / 4;
                let param_idx = filter_offset % 4;
                if let Some(filter) = filters.get_mut(filter_idx) {
                    match param_idx {
                        0 => {
                            filter.frequency =
                                (filter.frequency + delta * 10.0).clamp(20.0, 20000.0)
                        }
                        1 => filter.q = (filter.q + delta * 0.1).clamp(0.1, 10.0),
                        2 => filter.gain_db = (filter.gain_db + delta * 0.5).clamp(-24.0, 24.0),
                        3 => {
                            use crate::BiquadFilterType;
                            let types = [
                                BiquadFilterType::Peak,
                                BiquadFilterType::Lowshelf,
                                BiquadFilterType::Highshelf,
                                BiquadFilterType::Lowpass,
                                BiquadFilterType::Highpass,
                                BiquadFilterType::Bandpass,
                                BiquadFilterType::Notch,
                            ];
                            let current_idx = types
                                .iter()
                                .position(|t| *t == filter.filter_type)
                                .unwrap_or(0);
                            let new_idx = if delta > 0.0 {
                                (current_idx + 1) % types.len()
                            } else {
                                (current_idx + types.len() - 1) % types.len()
                            };
                            filter.filter_type = types[new_idx];
                        }
                        _ => return false,
                    }
                    return true;
                }
            }
            PluginSettings::MultibandCompressor {
                num_bands,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                bands,
                ..
            } => {
                use sotf_plugins::param_specs::{
                    find_by_key as pk,
                    multiband_compressor::{BAND_TEMPLATE, GLOBAL_PARAMS},
                };
                let global_count = GLOBAL_PARAMS.len();
                let band_params = BAND_TEMPLATE.len();
                match index {
                    0 => {
                        let new_bands = ((*num_bands as i64) + delta as i64).clamp(
                            pk(GLOBAL_PARAMS, "num_bands").min_f64() as i64,
                            pk(GLOBAL_PARAMS, "num_bands").max_f64() as i64,
                        ) as usize;
                        *num_bands = new_bands;
                        bands.resize_with(new_bands, Default::default);
                    }
                    i if i < global_count => {
                        if let Some(current) = self.param_value(i) {
                            let new_val = GLOBAL_PARAMS[i].adjust_f64(current, delta);
                            self.set_param_value(i, new_val);
                        }
                    }
                    _ => {
                        let band_offset = index - global_count;
                        let band_idx = band_offset / band_params;
                        let param_in_band = band_offset % band_params;
                        if band_idx >= bands.len() {
                            bands.resize_with(band_idx + 1, Default::default);
                        }
                        let band = &mut bands[band_idx];
                        match param_in_band {
                            0 => band.solo = !band.solo,
                            1 => band.bypass = !band.bypass,
                            2 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "threshold").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "threshold").max_f64() as f32,
                                );
                                band.threshold_db = match band.threshold_db {
                                    None => Some(*threshold_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            3 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "ratio").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "ratio").max_f64() as f32,
                                );
                                band.ratio = match band.ratio {
                                    None => Some(*ratio as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            4 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "attack").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "attack").max_f64() as f32,
                                );
                                band.attack_ms = match band.attack_ms {
                                    None => Some(*attack_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            5 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "release").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "release").max_f64() as f32,
                                );
                                band.release_ms = match band.release_ms {
                                    None => Some(*release_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            6 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "knee").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "knee").max_f64() as f32,
                                );
                                band.knee_db = match band.knee_db {
                                    None => Some(*knee_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            7 => {
                                band.makeup_gain_db =
                                    (band.makeup_gain_db + delta as f32 * 0.5).clamp(-24.0, 24.0);
                            }
                            _ => return false,
                        }
                    }
                }
                return true;
            }
            PluginSettings::MultibandExpander {
                num_bands,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                bands,
                ..
            } => {
                use sotf_plugins::param_specs::{
                    find_by_key as pk,
                    multiband_expander::{BAND_TEMPLATE, GLOBAL_PARAMS},
                };
                let global_count = GLOBAL_PARAMS.len();
                let band_params = BAND_TEMPLATE.len();
                match index {
                    0 => {
                        let new_bands = ((*num_bands as i64) + delta as i64).clamp(
                            pk(GLOBAL_PARAMS, "num_bands").min_f64() as i64,
                            pk(GLOBAL_PARAMS, "num_bands").max_f64() as i64,
                        ) as usize;
                        *num_bands = new_bands;
                        bands.resize_with(new_bands, Default::default);
                    }
                    i if i < global_count => {
                        if let Some(current) = self.param_value(i) {
                            let new_val = GLOBAL_PARAMS[i].adjust_f64(current, delta);
                            self.set_param_value(i, new_val);
                        }
                    }
                    _ => {
                        let band_offset = index - global_count;
                        let band_idx = band_offset / band_params;
                        let param_in_band = band_offset % band_params;
                        if band_idx >= bands.len() {
                            bands.resize_with(band_idx + 1, Default::default);
                        }
                        let band = &mut bands[band_idx];
                        match param_in_band {
                            0 => band.solo = !band.solo,
                            1 => band.bypass = !band.bypass,
                            2 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "threshold").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "threshold").max_f64() as f32,
                                );
                                band.threshold_db = match band.threshold_db {
                                    None => Some(*threshold_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            3 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "ratio").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "ratio").max_f64() as f32,
                                );
                                band.ratio = match band.ratio {
                                    None => Some(*ratio as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            4 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "attack").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "attack").max_f64() as f32,
                                );
                                band.attack_ms = match band.attack_ms {
                                    None => Some(*attack_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            5 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "release").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "release").max_f64() as f32,
                                );
                                band.release_ms = match band.release_ms {
                                    None => Some(*release_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            6 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "range").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "range").max_f64() as f32,
                                );
                                band.range_db = match band.range_db {
                                    None => Some(*range_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            7 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "knee").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "knee").max_f64() as f32,
                                );
                                band.knee_db = match band.knee_db {
                                    None => Some(*knee_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            8 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "hysteresis").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "hysteresis").max_f64() as f32,
                                );
                                band.hysteresis_db = match band.hysteresis_db {
                                    None => Some(*hysteresis_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            9 => {
                                let (lo, hi) = (
                                    pk(BAND_TEMPLATE, "hold").min_f64() as f32,
                                    pk(BAND_TEMPLATE, "hold").max_f64() as f32,
                                );
                                band.hold_ms = match band.hold_ms {
                                    None => Some(*hold_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < lo {
                                            None
                                        } else {
                                            Some(new_v.clamp(lo, hi))
                                        }
                                    }
                                };
                            }
                            _ => return false,
                        }
                    }
                }
                return true;
            }
            PluginSettings::SpectrumAnalyzer {
                num_bins,
                min_freq,
                max_freq,
                smoothing,
                tilt_correction,
                tilt_reference,
            } => {
                use sotf_plugins::param_specs::{find_by_key as pk, spectrum::PARAMS as SP};
                use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};
                match index {
                    0 => {
                        *num_bins = ((*num_bins as i64) + delta as i64).clamp(
                            pk(SP, "num_bins").min_f64() as i64,
                            pk(SP, "num_bins").max_f64() as i64,
                        ) as usize
                    }
                    1 => {
                        *min_freq = (*min_freq + delta as f32).clamp(
                            pk(SP, "min_freq").min_f64() as f32,
                            pk(SP, "min_freq").max_f64() as f32,
                        )
                    }
                    2 => {
                        *max_freq = (*max_freq + delta as f32 * 100.0).clamp(
                            pk(SP, "max_freq").min_f64() as f32,
                            pk(SP, "max_freq").max_f64() as f32,
                        )
                    }
                    3 => {
                        *smoothing = (*smoothing + delta as f32 * 0.01).clamp(
                            pk(SP, "smoothing").min_f64() as f32,
                            pk(SP, "smoothing").max_f64() as f32,
                        )
                    }
                    4 => {
                        let modes = [
                            SpectralTiltCorrection::None,
                            SpectralTiltCorrection::ThreeDbPerOctave,
                            SpectralTiltCorrection::SixDbPerOctave,
                            SpectralTiltCorrection::Pink,
                        ];
                        let current = modes.iter().position(|m| m == tilt_correction).unwrap_or(0);
                        let next = if delta > 0.0 {
                            (current + 1) % modes.len()
                        } else {
                            (current + modes.len() - 1) % modes.len()
                        };
                        *tilt_correction = modes[next];
                    }
                    5 => {
                        let modes = [
                            TiltReferenceFreq::Standard,
                            TiltReferenceFreq::OneKilohertz,
                            TiltReferenceFreq::TwoKilohertz,
                            TiltReferenceFreq::MinFreq,
                        ];
                        let current = modes.iter().position(|m| m == tilt_reference).unwrap_or(0);
                        let next = if delta > 0.0 {
                            (current + 1) % modes.len()
                        } else {
                            (current + modes.len() - 1) % modes.len()
                        };
                        *tilt_reference = modes[next];
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::ChannelMuteSolo { enabled, .. } => {
                match index {
                    0 => *enabled = !*enabled,
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } => {
                use sotf_plugins::param_specs::{find_by_key as pk, hal::PARAMS as HL};
                match index {
                    0 => {
                        *input_channels = ((*input_channels as i64) + delta as i64).clamp(
                            pk(HL, "input_channels").min_f64() as i64,
                            pk(HL, "input_channels").max_f64() as i64,
                        ) as usize
                    }
                    1 => {
                        *output_channels = ((*output_channels as i64) + delta as i64).clamp(
                            pk(HL, "output_channels").min_f64() as i64,
                            pk(HL, "output_channels").max_f64() as i64,
                        ) as usize
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Crossfeed {
                preset,
                mode,
                bauer_fcut_hz,
                bauer_feed_db,
                meier_level,
                mb_low_freq_hz,
                mb_mid_high_freq_hz,
                mb_low_feed_db,
                mb_mid_feed_db,
                mb_high_feed_db,
                ..
            } if index == 1 => {
                // Preset cycling applies preset logic that changes multiple fields
                use sotf_plugins::{CrossfeedPluginParams, CrossfeedPreset};
                let presets = [
                    CrossfeedPreset::Default,
                    CrossfeedPreset::Cmoy,
                    CrossfeedPreset::Meier,
                    CrossfeedPreset::Mb,
                    CrossfeedPreset::Off,
                ];
                let current = presets.iter().position(|p| p == preset).unwrap_or(0);
                let next = if delta > 0.0 {
                    (current + 1) % presets.len()
                } else {
                    (current + presets.len() - 1) % presets.len()
                };
                *preset = presets[next];
                let p = CrossfeedPluginParams::from_preset(*preset);
                *mode = p.mode;
                *bauer_fcut_hz = p.bauer_fcut_hz as f64;
                *bauer_feed_db = p.bauer_feed_db as f64;
                *meier_level = p.meier_level as f64;
                *mb_low_freq_hz = p.mb_low_freq_hz as f64;
                *mb_mid_high_freq_hz = p.mb_mid_high_freq_hz as f64;
                *mb_low_feed_db = p.mb_low_feed_db as f64;
                *mb_mid_feed_db = p.mb_mid_feed_db as f64;
                *mb_high_feed_db = p.mb_high_feed_db as f64;
                return true;
            }
            // Generic: all other plugins use ParamSpec
            _ => {
                let specs = self.param_specs();
                if let Some(spec) = specs.get(index) {
                    if matches!(spec.param_type, ParamType::FilePath) {
                        return false;
                    }
                    if let Some(current) = self.param_value(index) {
                        let new_val = spec.adjust_f64(current, delta);
                        self.set_param_value(index, new_val);
                        return true;
                    }
                }
            }
        }
        false
    }

    fn get_choice_labels(&self, index: usize) -> Vec<String> {
        match self {
            PluginSettings::EQ { filters, .. } => {
                if index == 0 {
                    return Vec::new();
                }
                let filter_offset = index - 1;
                let param_idx = filter_offset % 4;
                if param_idx == 3 && (filter_offset / 4) < filters.len() {
                    let spec = &sotf_plugins::param_specs::eq::BAND_TEMPLATE[3];
                    return spec.choice_labels().iter().map(|s| s.to_string()).collect();
                }
                Vec::new()
            }
            PluginSettings::SpectrumAnalyzer { .. } => match index {
                4 => vec![
                    "None".into(),
                    "ThreeDbPerOctave".into(),
                    "SixDbPerOctave".into(),
                    "Pink".into(),
                ],
                5 => vec![
                    "Standard".into(),
                    "OneKilohertz".into(),
                    "TwoKilohertz".into(),
                    "MinFreq".into(),
                ],
                _ => Vec::new(),
            },
            _ => {
                let specs = self.param_specs();
                if let Some(spec) = specs.get(index) {
                    spec.choice_labels().iter().map(|s| s.to_string()).collect()
                } else {
                    Vec::new()
                }
            }
        }
    }
}
