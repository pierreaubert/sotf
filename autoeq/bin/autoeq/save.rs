use autoeq::iir;
use chrono;
use std::{error::Error, path::Path};
use tokio::fs;

/// Save PEQ settings to APO format file
///
/// # Arguments
/// * `args` - Command line arguments
/// * `x` - Optimized filter parameters
/// * `output_path` - Base output path for files
/// * `loss_type` - Type of optimization performed
///
/// # Returns
/// * Result indicating success or error
pub(super) async fn save_peq_to_file(
    args: &autoeq::cli::Args,
    x: &[f64],
    output_path: &Path,
    loss_type: &autoeq::LossType,
) -> Result<(), Box<dyn Error>> {
    // Build the PEQ from the optimized parameters
    let peq_model = args.effective_peq_model();
    let peq = autoeq::x2peq::x2peq(x, args.sample_rate, peq_model);

    // Determine filename based on loss type
    let filename = match loss_type {
        autoeq::LossType::SpeakerFlat | autoeq::LossType::HeadphoneFlat => "iir-autoeq-flat.txt",
        autoeq::LossType::SpeakerScore | autoeq::LossType::HeadphoneScore => "iir-autoeq-score.txt",
        autoeq::LossType::DriversFlat => {
            // Unreachable: DriversFlat mode uses a separate code path
            unreachable!("DriversFlat mode should not reach this point");
        }
    };

    // Create the full path (same directory as the plots)
    let parent_dir = output_path.parent().unwrap_or(output_path);
    let file_path = parent_dir.join(filename);

    // Generate comment string with optimization details
    let comment = format!(
        "# AutoEQ Parametric Equalizer Settings\n# Speaker: {}\n# Loss Type: {:?}\n# Filters: {}\n# Generated: {}",
        args.speaker.as_deref().unwrap_or("Unknown"),
        loss_type,
        args.num_filters,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    // Format the PEQ as APO string
    let apo_content = iir::peq_format_apo(&comment, &peq);

    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Write the APO file
    fs::write(&file_path, apo_content).await?;
    autoeq::qa_println!(args, "🕶 PEQ settings saved to: {}", file_path.display());

    // Save RME TotalMix format (.xml)
    let rme_filename = filename.replace(".txt", ".tmreq");
    let rme_path = parent_dir.join(&rme_filename);
    let rme_content = iir::peq_format_rme_room(&peq, &peq);
    fs::write(&rme_path, rme_content).await?;
    autoeq::qa_println!(
        args,
        "🎚  RME TotalMix RoomEQ preset saved to: {}",
        rme_path.display()
    );

    // Save Apple AUNBandEQ format (.aupreset)
    let aupreset_filename = filename.replace(".txt", ".aupreset");
    let aupreset_path = parent_dir.join(&aupreset_filename);
    let preset_name = format!("AutoEQ {}", args.speaker.as_deref().unwrap_or("Unknown"));
    let aupreset_content = iir::peq_format_aupreset(&peq, &preset_name);
    fs::write(&aupreset_path, aupreset_content).await?;
    autoeq::qa_println!(
        args,
        "🍎 Apple AUpreset saved to: {}",
        aupreset_path.display()
    );

    Ok(())
}
