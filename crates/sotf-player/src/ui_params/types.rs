use super::misc::apply_eq_band_field;
use super::tui_editable_plugin::TuiEditablePlugin;
use super::tui_param_descriptor::spec_to_descriptor;
use super::tui_param_descriptor::specs_to_descriptors;
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
            PluginSettings::LinearPhaseEq { num_filters, .. } => {
                let mut descs = specs_to_descriptors(param_specs::linear_phase_eq::PARAMS);
                let n = (*num_filters as usize).clamp(1, 10);
                for i in 0..n {
                    let g = format!("Filter {}", i + 1);
                    for spec in param_specs::linear_phase_eq::BAND_TEMPLATE {
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
            PluginSettings::Matrix { .. } => {
                vec![
                    TuiParamDescriptor {
                        name: "Input Channels".to_string(),
                        param_type: TuiParamType::Int {
                            min: 1,
                            max: 32,
                            step: 1,
                        },
                        unit: "".to_string(),
                        group: "Matrix".to_string(),
                        doc: "Number of input channels".to_string(),
                    },
                    TuiParamDescriptor {
                        name: "Output Channels".to_string(),
                        param_type: TuiParamType::Int {
                            min: 1,
                            max: 32,
                            step: 1,
                        },
                        unit: "".to_string(),
                        group: "Matrix".to_string(),
                        doc: "Number of output channels".to_string(),
                    },
                ]
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
            PluginSettings::LinearPhaseEq {
                num_filters,
                fir_length,
                phase_mode,
                auto_gain,
                mix,
                filters,
            } => {
                let global_specs = sotf_plugins::param_specs::linear_phase_eq::PARAMS;
                let band_template = sotf_plugins::param_specs::linear_phase_eq::BAND_TEMPLATE;
                let global_count = global_specs.len();
                let band_params = band_template.len();
                if index < global_count {
                    let val = match index {
                        0 => *num_filters,
                        1 => *fir_length,
                        2 => {
                            return global_specs[2].format_value(*phase_mode);
                        }
                        3 => {
                            return global_specs[3].format_value(if *auto_gain {
                                1.0
                            } else {
                                0.0
                            });
                        }
                        4 => *mix,
                        _ => return String::new(),
                    };
                    global_specs[index].format_value(val)
                } else {
                    let band_offset = index - global_count;
                    let band_idx = band_offset / band_params;
                    let param_in_band = band_offset % band_params;
                    if let Some(filter) = filters.get(band_idx) {
                        match param_in_band {
                            0 => format!("{:?}", filter.filter_type),
                            1 => format!("{:.0}", filter.frequency),
                            2 => format!("{:.2}", filter.q),
                            3 => format!("{:.1}", filter.gain_db),
                            4 => {
                                band_template[4].format_value(if filter.muted { 0.0 } else { 1.0 })
                            }
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    }
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
            PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } => match index {
                0 => format!("{}", input_channels),
                1 => format!("{}", output_channels),
                _ => String::new(),
            },
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
                // TUI index space: idx 0 is `max_filters` (handled above), idx
                // 1+ maps to (band, field). Delegate to the shared helper so
                // this matches `controllers::plugin::adjust_plugin_param` (which
                // uses the same per-field math but a different outer indexing).
                let filter_offset = index - 1;
                let filter_idx = filter_offset / 4;
                let field_idx = filter_offset % 4;
                if let Some(filter) = filters.get_mut(filter_idx) {
                    return apply_eq_band_field(filter, field_idx, delta);
                }
            }
            PluginSettings::LinearPhaseEq {
                num_filters,
                fir_length,
                phase_mode,
                auto_gain,
                mix,
                filters,
            } => {
                let global_specs = sotf_plugins::param_specs::linear_phase_eq::PARAMS;
                let global_count = global_specs.len();
                if index < global_count {
                    match index {
                        0 => {
                            let old = *num_filters as usize;
                            *num_filters =
                                ((*num_filters as i64) + delta as i64).clamp(1, 10) as f64;
                            let new = *num_filters as usize;
                            if new > old {
                                while filters.len() < new {
                                    filters.push(crate::EQFilter::new(
                                        crate::BiquadFilterType::Peak,
                                        1000.0,
                                        1.0,
                                        0.0,
                                    ));
                                }
                            } else if new < old {
                                filters.truncate(new);
                            }
                        }
                        1 => {
                            *fir_length = ((*fir_length as i64) + delta as i64).clamp(0, 3) as f64;
                        }
                        2 => *phase_mode = if *phase_mode >= 0.5 { 0.0 } else { 1.0 },
                        3 => *auto_gain = !*auto_gain,
                        4 => *mix = (*mix + delta * 0.01).clamp(0.0, 1.0),
                        _ => return false,
                    }
                    return true;
                }
                let band_offset = index - global_count;
                let band_params = sotf_plugins::param_specs::linear_phase_eq::BAND_TEMPLATE.len();
                let band_idx = band_offset / band_params;
                let param_in_band = band_offset % band_params;
                if let Some(filter) = filters.get_mut(band_idx) {
                    use crate::BiquadFilterType;
                    match param_in_band {
                        0 => {
                            // Linear-phase only supports Peak/Lowshelf/Highshelf/Lowpass/Highpass.
                            let types = [
                                BiquadFilterType::Peak,
                                BiquadFilterType::Lowshelf,
                                BiquadFilterType::Highshelf,
                                BiquadFilterType::Lowpass,
                                BiquadFilterType::Highpass,
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
                        1 => {
                            filter.frequency =
                                (filter.frequency + delta * 10.0).clamp(20.0, 20000.0)
                        }
                        2 => filter.q = (filter.q + delta * 0.05).clamp(0.1, 10.0),
                        3 => filter.gain_db = (filter.gain_db + delta * 0.5).clamp(-24.0, 24.0),
                        4 => filter.muted = !filter.muted,
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
                // HAL Matrix: input/output channel counts (1..32)
                match index {
                    0 => {
                        *input_channels =
                            ((*input_channels as i64) + delta as i64).clamp(1, 32) as usize
                    }
                    1 => {
                        *output_channels =
                            ((*output_channels as i64) + delta as i64).clamp(1, 32) as usize
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
            PluginSettings::LinearPhaseEq { filters, .. } => {
                let global_specs = sotf_plugins::param_specs::linear_phase_eq::PARAMS;
                let global_count = global_specs.len();
                if index < global_count {
                    if let Some(spec) = global_specs.get(index)
                        && matches!(
                            spec.param_type,
                            sotf_plugins::param_specs::ParamType::Choice { .. }
                        )
                    {
                        return spec.choice_labels().iter().map(|s| s.to_string()).collect();
                    }
                    return Vec::new();
                }
                let band_offset = index - global_count;
                let band_params = sotf_plugins::param_specs::linear_phase_eq::BAND_TEMPLATE.len();
                let param_in_band = band_offset % band_params;
                let band_idx = band_offset / band_params;
                if param_in_band == 0 && band_idx < filters.len() {
                    let spec = &sotf_plugins::param_specs::linear_phase_eq::BAND_TEMPLATE[0];
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
