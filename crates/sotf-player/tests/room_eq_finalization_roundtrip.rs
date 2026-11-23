//! Imported protection survives the shared RoomEQ configuration conversion.
use sotf_audio_player::room_eq_types::RoomEqOptimizerConfig;

#[test]
fn imported_finalization_is_preserved_and_replaced() {
    let mut backend = roomeq_model::OptimizerConfig::default();
    backend.finalization.subwoofer_limiter = true;
    backend.finalization.output_ceiling_dbfs = -3.0;
    let mut ui = RoomEqOptimizerConfig::default();
    ui.import_from_backend(&backend);
    let restored = ui.to_optimizer_config();
    assert_eq!(restored.finalization, backend.finalization);
    ui.import_from_backend(&roomeq_model::OptimizerConfig::default());
    assert!(!ui.to_optimizer_config().finalization.subwoofer_limiter);
}
