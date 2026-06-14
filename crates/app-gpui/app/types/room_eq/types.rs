pub type CrossoverType = sotf_audio_player::room_eq_types::RoomEqCrossoverType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomEqReviewGraphId {
    OverviewOriginal,
    OverviewEq,
    OverviewCorrected,
    ChannelFull,
    ChannelZoom,
    ChannelEq,
}
