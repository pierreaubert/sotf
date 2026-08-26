use super::super::commands::DevCommand;
use super::dispatch::consume_commands;
use super::dispatch::dispatch_request;
use super::types::HttpRequest;
use super::with::mark_process_started;
use anyhow::{Result, anyhow};
use gpui::{AnyWindowHandle, App, AsyncApp};
use sotf_audio_player::room_eq_types::{
    SimpleCrossoverChoice, SimpleLossChoice, SimpleProcessingChoice, SpeakerTier,
};
use sotf_dev_api::server::{ServerConfig, start_server};
use sotf_dev_api::{
    Capabilities, FixtureCapability, HttpResponse, InputCapabilities, Method, NamedCapability,
    RunId,
};
use std::sync::mpsc::{self};
use std::time::Duration;

/// Spawn the dev-api server. Listens on `127.0.0.1:<port>`.
///
/// Must be called from the GPUI main thread, after the window has been
/// created. The returned task is detached and runs for the lifetime of
/// the app.
pub fn start(cx: &mut App, port: u16, window: AnyWindowHandle) {
    mark_process_started();
    let (tx, rx) = mpsc::sync_channel::<DevCommand>(64);
    let run_id = match std::env::var("SOTF_DEV_API_RUN_ID")
        .map_err(anyhow::Error::from)
        .and_then(|value| RunId::parse(value).map_err(anyhow::Error::from))
    {
        Ok(run_id) => run_id,
        Err(error) => {
            log::error!("refusing to start dev-api without a valid SOTF_DEV_API_RUN_ID: {error:#}");
            return;
        }
    };
    let capabilities = capabilities(cx);
    let mut config = ServerConfig::loopback(run_id, capabilities);
    config.bind = ([127, 0, 0, 1], port).into();
    let dispatcher_tx = tx.clone();
    match start_server(
        config,
        move |request: sotf_dev_api::HttpRequest, _context| {
            let request = HttpRequest {
                method: match request.method {
                    Method::Get => "GET".into(),
                    Method::Post => "POST".into(),
                },
                path: request.path,
                body: request.body,
            };
            legacy_response(&dispatch_request(&request, &dispatcher_tx))
        },
    ) {
        Ok(handle) => {
            let endpoint = handle.endpoint();
            std::thread::Builder::new()
                .name("sotf-dev-api-owner".into())
                .spawn(move || {
                    loop {
                        std::hint::black_box(&handle);
                        std::thread::park_timeout(Duration::from_secs(3600));
                    }
                })
                .expect("failed to retain dev-api server handle");
            log::info!("dev-api protocol v2 listening on http://{endpoint}");
        }
        Err(error) => {
            log::error!("dev-api listener failed: {error}");
            return;
        }
    }

    cx.spawn(async move |cx: &mut AsyncApp| {
        consume_commands(rx, window, cx).await;
    })
    .detach();
}

fn capabilities(cx: &App) -> Capabilities {
    let mut capabilities = Capabilities::new("desktop-gpui", "sotf-desktop");
    capabilities.build_version = env!("CARGO_PKG_VERSION").into();
    capabilities.build_id = option_env!("SOTF_BUILD_ID").unwrap_or("development").into();
    capabilities.debug_features = vec!["dev-api".into(), "qa".into(), "visual-qa".into()];
    capabilities.actions = advertised_action_names(cx)
        .into_iter()
        .map(|name| NamedCapability {
            name,
            family: "gpui_action".into(),
            payload_schema: None,
        })
        .collect();
    for name in [
        "PluginAdd",
        "PluginClear",
        "PluginRemove",
        "PluginToggle",
        "PluginMoveUp",
        "PluginMoveDown",
        "PluginSetParam",
        "PluginSetParamString",
        "PluginChainSave",
        "PluginChainLoad",
        "SettingsSetTab",
        "MetadataSeedAlbum",
        "HomeSeedShelves",
        "ListeningResetGuide",
        "MetadataOpenAlbumEditor",
        "MetadataSetField",
    ] {
        capabilities.actions.push(NamedCapability {
            name: name.into(),
            family: "qa_action".into(),
            payload_schema: None,
        });
    }
    capabilities.queries = vec![
        "screen.focused".into(),
        "input_mode".into(),
        "playback.volume".into(),
        "playback.is_playing".into(),
        "playback.muted".into(),
        "spectrum.hold".into(),
        "spectrum.smoothing".into(),
        "spectrum.has_data".into(),
        "listening.guide_open".into(),
        "listening.guide_completed".into(),
        "listening.surface".into(),
        "queue.length".into(),
        "queue.current_index".into(),
        "queue.first_title".into(),
        "queue.second_title".into(),
        "queue.can_undo_clear".into(),
        "queue.can_undo_remove".into(),
        "library.album_count".into(),
        "library.track_count".into(),
        "library.filtered_album_count".into(),
        "library.search_query".into(),
        "library.sort_order".into(),
        "library.channel_filter".into(),
        "metadata.editor_open".into(),
    ];
    capabilities.inputs = InputCapabilities {
        key: true,
        text: true,
        selector: true,
        pointer: true,
        touch: false,
        scroll: true,
        resize: true,
        remote: vec![],
    };
    capabilities.fixtures = [
        "seed",
        "recording/fake-capture",
        "headphone/discovery-fixture",
        "spinorama/discovery-fixture",
        "room-eq",
        "room-eq/ui-fixture",
    ]
    .into_iter()
    .map(|name| FixtureCapability {
        name: name.into(),
        max_body_bytes: capabilities.limits.fixture_body_bytes,
    })
    .collect();
    capabilities
}

fn advertised_action_names(cx: &App) -> Vec<String> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for name in cx.all_action_names() {
        let bare = name.rsplit("::").next().unwrap_or(name).to_owned();
        *counts.entry(bare).or_default() += 1;
    }
    let mut names = cx
        .all_action_names()
        .iter()
        .map(|name| {
            let bare = name.rsplit("::").next().unwrap_or(name);
            if counts.get(bare) == Some(&1) {
                bare.to_owned()
            } else {
                (*name).to_owned()
            }
        })
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn legacy_response(response: &str) -> HttpResponse {
    let (head, body) = response.split_once("\r\n\r\n").unwrap_or((response, ""));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_ascii_whitespace().nth(1))
        .and_then(|status| status.parse().ok())
        .unwrap_or(500);
    HttpResponse::json(status, body.as_bytes().to_vec())
}

pub(super) fn parse_speaker_tier(value: &str) -> Result<SpeakerTier> {
    match value {
        "NearField" | "near-field" | "nearfield" => Ok(SpeakerTier::NearField),
        "MidField" | "mid-field" | "midfield" => Ok(SpeakerTier::MidField),
        "FarField" | "far-field" | "farfield" => Ok(SpeakerTier::FarField),
        other => Err(anyhow!("unknown RoomEQ target `{other}`")),
    }
}

pub(super) fn parse_simple_loss(value: &str) -> Result<SimpleLossChoice> {
    match value {
        "Flat" | "flat" => Ok(SimpleLossChoice::Flat),
        "Epa" | "EPA" | "epa" => Ok(SimpleLossChoice::Epa),
        other => Err(anyhow!("unknown RoomEQ loss `{other}`")),
    }
}

pub(super) fn parse_simple_processing(value: &str) -> Result<SimpleProcessingChoice> {
    match value {
        "Iir" | "IIR" | "iir" => Ok(SimpleProcessingChoice::Iir),
        "MixedPhase" | "mixed-phase" | "mixed_phase" => Ok(SimpleProcessingChoice::MixedPhase),
        other => Err(anyhow!("unknown RoomEQ processing `{other}`")),
    }
}

pub(super) fn parse_simple_crossover(value: &str) -> Result<SimpleCrossoverChoice> {
    match value {
        "Lr24" | "LR24" | "lr24" => Ok(SimpleCrossoverChoice::Lr24),
        "Lr48" | "LR48" | "lr48" => Ok(SimpleCrossoverChoice::Lr48),
        other => Err(anyhow!("unknown RoomEQ crossover `{other}`")),
    }
}

pub(super) fn parse_json_payload(body: &[u8]) -> Result<serde_json::Value> {
    serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))
}
