use crate::app::App;
use sotf_audio_player::room_eq_types::RoomEqStep;

pub(super) fn room_eq_step_prev_wrap(s: RoomEqStep) -> RoomEqStep {
    match s {
        RoomEqStep::LoadData => RoomEqStep::Export,
        RoomEqStep::Delay => RoomEqStep::LoadData,
        RoomEqStep::Process => RoomEqStep::Delay,
        RoomEqStep::Configure => RoomEqStep::Process,
        RoomEqStep::Optimize => RoomEqStep::Configure,
        RoomEqStep::Review => RoomEqStep::Optimize,
        RoomEqStep::Export => RoomEqStep::Review,
    }
}

pub(super) fn room_eq_step_next_wrap(s: RoomEqStep) -> RoomEqStep {
    match s {
        RoomEqStep::LoadData => RoomEqStep::Delay,
        RoomEqStep::Delay => RoomEqStep::Process,
        RoomEqStep::Process => RoomEqStep::Configure,
        RoomEqStep::Configure => RoomEqStep::Optimize,
        RoomEqStep::Optimize => RoomEqStep::Review,
        RoomEqStep::Review => RoomEqStep::Export,
        RoomEqStep::Export => RoomEqStep::LoadData,
    }
}

pub(super) fn room_eq_field_value_string(app: &App, field: usize) -> String {
    let c = &app.room_eq.model.optimizer_config;
    match field {
        0 => c.num_filters.to_string(),
        1 => format!("{:.0}", c.min_freq),
        2 => format!("{:.0}", c.max_freq),
        3 => format!("{:.1}", c.min_db),
        4 => format!("{:.1}", c.max_db),
        5 => format!("{:.1}", c.min_q),
        6 => format!("{:.1}", c.max_q),
        9 => c.max_iter.to_string(),
        10 => c.population.to_string(),
        11 => c.bo_initial_samples.to_string(),
        12 => c.bo_batch_size.to_string(),
        13 => format!("{:.3}", c.bo_posterior_std_threshold),
        23 => format!("{:.1}", c.target_response.slope_db_per_octave),
        25 => format!("{:.0}", c.excursion_protection.manual_f3_hz),
        27 => format!("{:.0}", c.schroeder_split.schroeder_freq),
        _ => String::new(),
    }
}
