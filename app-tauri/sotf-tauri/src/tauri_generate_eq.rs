#[tauri::command]
pub async fn generate_apo_format(
    filter_params: Vec<f64>,
    sample_rate: f64,
    peq_model: String,
) -> Result<String, String> {
    println!(
        "[TAURI] Generating APO format: {} params, {}Hz, model: {}",
        filter_params.len(),
        sample_rate,
        peq_model
    );

    // Convert string to PeqModel enum
    let peq_model_enum = match peq_model.as_str() {
        "hp-pk" => autoeq::cli::PeqModel::HpPk,
        "hp-pk-lp" => autoeq::cli::PeqModel::HpPkLp,
        "ls-pk" => autoeq::cli::PeqModel::LsPk,
        "ls-pk-hs" => autoeq::cli::PeqModel::LsPkHs,
        "free-pk-free" => autoeq::cli::PeqModel::FreePkFree,
        "free" => autoeq::cli::PeqModel::Free,
        "pk" | _ => autoeq::cli::PeqModel::Pk,
    };

    // Convert parameter vector to PEQ structure
    let peq = autoeq::x2peq::x2peq(&filter_params, sample_rate, peq_model_enum);

    // Generate APO format string
    let apo_string = autoeq::iir::peq_format_apo("AutoEQ Optimization Result", &peq);

    println!("[TAURI] Generated {} bytes of APO data", apo_string.len());

    Ok(apo_string)
}

#[tauri::command]
pub async fn generate_aupreset_format(
    filter_params: Vec<f64>,
    sample_rate: f64,
    peq_model: String,
    preset_name: String,
) -> Result<String, String> {
    println!(
        "[TAURI] Generating AUpreset format: {} params, {}Hz, model: {}, name: {}",
        filter_params.len(),
        sample_rate,
        peq_model,
        preset_name
    );

    // Convert string to PeqModel enum
    let peq_model_enum = match peq_model.as_str() {
        "hp-pk" => autoeq::cli::PeqModel::HpPk,
        "hp-pk-lp" => autoeq::cli::PeqModel::HpPkLp,
        "ls-pk" => autoeq::cli::PeqModel::LsPk,
        "ls-pk-hs" => autoeq::cli::PeqModel::LsPkHs,
        "free-pk-free" => autoeq::cli::PeqModel::FreePkFree,
        "free" => autoeq::cli::PeqModel::Free,
        "pk" | _ => autoeq::cli::PeqModel::Pk,
    };

    // Convert parameter vector to PEQ structure
    let peq = autoeq::x2peq::x2peq(&filter_params, sample_rate, peq_model_enum);

    // Generate AUpreset format string
    let aupreset_string = autoeq::iir::peq_format_aupreset(&peq, &preset_name);

    println!(
        "[TAURI] Generated {} bytes of AUpreset data",
        aupreset_string.len()
    );

    Ok(aupreset_string)
}

#[tauri::command]
pub async fn generate_rme_format(
    filter_params: Vec<f64>,
    sample_rate: f64,
    peq_model: String,
) -> Result<String, String> {
    println!(
        "[TAURI] Generating RME format: {} params, {}Hz, model: {}",
        filter_params.len(),
        sample_rate,
        peq_model
    );

    // Convert string to PeqModel enum
    let peq_model_enum = match peq_model.as_str() {
        "hp-pk" => autoeq::cli::PeqModel::HpPk,
        "hp-pk-lp" => autoeq::cli::PeqModel::HpPkLp,
        "ls-pk" => autoeq::cli::PeqModel::LsPk,
        "ls-pk-hs" => autoeq::cli::PeqModel::LsPkHs,
        "free-pk-free" => autoeq::cli::PeqModel::FreePkFree,
        "free" => autoeq::cli::PeqModel::Free,
        "pk" | _ => autoeq::cli::PeqModel::Pk,
    };

    // Convert parameter vector to PEQ structure
    let peq = autoeq::x2peq::x2peq(&filter_params, sample_rate, peq_model_enum);

    // Generate RME format string
    let rme_string = autoeq::iir::peq_format_rme_channel(&peq);

    println!("[TAURI] Generated {} bytes of RME data", rme_string.len());

    Ok(rme_string)
}

#[tauri::command]
pub async fn generate_rme_room_format(
    filter_params: Vec<f64>,
    sample_rate: f64,
    peq_model: String,
) -> Result<String, String> {
    println!(
        "[TAURI] Generating RME Room format: {} params, {}Hz, model: {}",
        filter_params.len(),
        sample_rate,
        peq_model
    );

    // Convert string to PeqModel enum
    let peq_model_enum = match peq_model.as_str() {
        "hp-pk" => autoeq::cli::PeqModel::HpPk,
        "hp-pk-lp" => autoeq::cli::PeqModel::HpPkLp,
        "ls-pk" => autoeq::cli::PeqModel::LsPk,
        "ls-pk-hs" => autoeq::cli::PeqModel::LsPkHs,
        "free-pk-free" => autoeq::cli::PeqModel::FreePkFree,
        "free" => autoeq::cli::PeqModel::Free,
        "pk" | _ => autoeq::cli::PeqModel::Pk,
    };

    // Convert parameter vector to PEQ structure
    let peq = autoeq::x2peq::x2peq(&filter_params, sample_rate, peq_model_enum);

    // Generate RME Room format string (dual channel, using same PEQ for both)
    let empty_peq: Vec<(f64, autoeq::iir::Biquad)> = vec![];
    let rme_room_string = autoeq::iir::peq_format_rme_room(&peq, &empty_peq);

    println!(
        "[TAURI] Generated {} bytes of RME Room data",
        rme_room_string.len()
    );

    Ok(rme_room_string)
}
