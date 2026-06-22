use crate::app::App;

pub(super) fn is_headphone_eq_field_numerical(field: usize) -> bool {
    matches!(field, 0..=6 | 9 | 10 | 12 | 13 | 17)
}

pub(super) fn set_headphone_eq_field_from_string(app: &mut App) {
    let c = &mut app.headphone_eq.model.optimizer_config;
    let buf = &app.headphone_eq.edit_buffer;
    match app.headphone_eq.config_selected_field {
        0 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.num_filters = v.clamp(1, 30);
            }
        }
        1 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_freq = v.clamp(20.0, 500.0);
            }
        }
        2 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_freq = v.clamp(1000.0, 20000.0);
            }
        }
        3 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_db = v.clamp(-24.0, 0.0);
            }
        }
        4 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_db = v.clamp(0.0, 12.0);
            }
        }
        5 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_q = v.clamp(0.1, 2.0);
            }
        }
        6 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_q = v.clamp(1.0, 20.0);
            }
        }
        9 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.max_iter = v.clamp(1000, 100000);
            }
        }
        10 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.population = v.clamp(10, 200);
            }
        }
        12 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.de_f = v.clamp(0.1, 2.0);
            }
        }
        13 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.de_cr = v.clamp(0.1, 1.0);
            }
        }
        17 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.smooth_n = v.clamp(1, 24);
            }
        }
        _ => {}
    }
}

pub(super) fn adjust_headphone_eq_field(app: &mut App, delta: i32) {
    // Field 100: preset cycling
    if app.headphone_eq.config_selected_field == 100 {
        use sotf_audio_player::autoeq::{self, EqWorkflow};
        let presets = autoeq::presets_for(EqWorkflow::Headphone);
        let ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
        let new_id = super::super::cycle_string(&app.headphone_eq.selected_preset, &ids, delta);
        app.headphone_eq.selected_preset = new_id.clone();
        // Apply preset parameters (skip for "custom")
        if let Some(preset) = autoeq::find_preset(EqWorkflow::Headphone, &new_id)
            && let Some(params) = preset.apply()
        {
            let c = &mut app.headphone_eq.model.optimizer_config;
            c.num_filters = params.num_filters;
            c.min_freq = params.min_freq;
            c.max_freq = params.max_freq;
            c.min_db = params.min_db;
            c.max_db = params.max_db;
            c.min_q = params.min_q;
            c.max_q = params.max_q;
            c.peq_model = params.peq_model;
            c.population = params.population;
            c.max_iter = params.maxeval;
            c.refine = params.refine;
            c.smooth = params.smooth;
            c.smooth_n = params.smooth_n;
            c.loss = params.loss;
        }
        return;
    }
    // Field 18: loss function cycling
    if app.headphone_eq.config_selected_field == 18 {
        let losses: Vec<&str> = sotf_audio_player::autoeq::HEADPHONE_LOSS_OPTIONS
            .iter()
            .map(|(id, _)| *id)
            .collect();
        app.headphone_eq.model.optimizer_config.loss = super::super::cycle_string(
            &app.headphone_eq.model.optimizer_config.loss,
            &losses,
            delta,
        );
        return;
    }
    let c = &mut app.headphone_eq.model.optimizer_config;
    match app.headphone_eq.config_selected_field {
        0 => {
            let n = c.num_filters as i32 + delta;
            c.num_filters = n.clamp(1, 30) as usize;
        }
        1 => c.min_freq = (c.min_freq + delta as f64 * 10.0).clamp(20.0, 500.0),
        2 => c.max_freq = (c.max_freq + delta as f64 * 500.0).clamp(1000.0, 20000.0),
        3 => c.min_db = (c.min_db + delta as f64).clamp(-24.0, 0.0),
        4 => c.max_db = (c.max_db + delta as f64).clamp(0.0, 12.0),
        5 => c.min_q = (c.min_q + delta as f64 * 0.1).clamp(0.1, 2.0),
        6 => c.max_q = (c.max_q + delta as f64 * 0.5).clamp(1.0, 20.0),
        7 => {
            c.peq_model = super::super::cycle_string(
                &c.peq_model,
                &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"],
                delta,
            );
        }
        8 => {
            use sotf_audio_player::room_eq_types::RoomEqAlgorithm;
            let algos = RoomEqAlgorithm::all();
            let idx = algos.iter().position(|a| *a == c.algorithm).unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % algos.len()
            } else {
                (idx + algos.len() - 1) % algos.len()
            };
            c.algorithm = algos[new_idx];
        }
        9 => {
            let n = c.max_iter as i32 + delta * 1000;
            c.max_iter = n.clamp(1000, 100000) as usize;
        }
        10 => {
            let n = c.population as i32 + delta * 10;
            c.population = n.clamp(10, 200) as usize;
        }
        11 => {
            c.strategy = super::super::cycle_string(
                &c.strategy,
                &["currenttobest1bin", "best1bin", "rand1bin", "best2bin"],
                delta,
            );
        }
        12 => c.de_f = (c.de_f + delta as f64 * 0.1).clamp(0.1, 2.0),
        13 => c.de_cr = (c.de_cr + delta as f64 * 0.1).clamp(0.1, 1.0),
        14 => c.refine = !c.refine,
        15 => {
            c.local_algo = super::super::cycle_string(&c.local_algo, &["cobyla"], delta);
        }
        16 => c.smooth = !c.smooth,
        17 => {
            let n = c.smooth_n as i32 + delta;
            c.smooth_n = n.clamp(1, 24) as usize;
        }
        _ => {}
    }
}
