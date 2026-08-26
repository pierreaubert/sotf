use super::super::commands::{DevCommand, DevQueryReply, DevReply};
use super::misc::REPLY_TIMEOUT;
use super::parse::parse_json_payload;
use anyhow::{Result, anyhow};
use std::sync::mpsc::{self, SyncSender};

pub(super) fn post_action(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        name: String,
        #[serde(default)]
        payload: Option<serde_json::Value>,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx): (SyncSender<DevReply>, _) = mpsc::sync_channel(1);
    tx.send(DevCommand::Action {
        name: payload.name,
        payload: payload.payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;

    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_key(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        keystroke: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Key {
        keystroke: payload.keystroke,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_text(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        text: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    if payload.text.chars().count() > 4096 {
        return Err(anyhow!("text input exceeds 4096 characters"));
    }

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Text {
        text: payload.text,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))
}

pub(super) fn post_input(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    let input: sotf_dev_api::CoordinateInput =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid coordinate input JSON: {e}"))?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Input {
        input,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))
}

pub(super) fn post_click(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        selector: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Click {
        selector: payload.selector,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_hover(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        selector: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Hover {
        selector: payload.selector,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_drag(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        source: String,
        target: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Drag {
        source: payload.source,
        target: payload.target,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_scroll(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        selector: String,
        delta_y: f32,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    if !payload.delta_y.is_finite() {
        return Err(anyhow!("scroll delta_y must be finite"));
    }

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Scroll {
        selector: payload.selector,
        delta_y: payload.delta_y,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_resize(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        width: f32,
        height: f32,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    if !payload.width.is_finite()
        || !payload.height.is_finite()
        || payload.width <= 0.0
        || payload.height <= 0.0
    {
        return Err(anyhow!(
            "resize width and height must be finite positive numbers"
        ));
    }

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Resize {
        width: payload.width,
        height: payload.height,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_screenshot(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        name: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    if payload.name.is_empty()
        || !payload.name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(anyhow!(
            "screenshot name must contain only ASCII letters, digits, '-' or '_'"
        ));
    }

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Screenshot {
        name: payload.name,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_quit(tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Quit { reply: reply_tx })
        .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_qa_seed(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    let payload = parse_json_payload(body)?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaSeed {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_qa_recording_fake_capture(
    body: &[u8],
    tx: &mpsc::SyncSender<DevCommand>,
) -> Result<DevReply> {
    let payload = parse_json_payload(body)?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaRecordingFakeCapture {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_qa_headphone_discovery_fixture(
    body: &[u8],
    tx: &mpsc::SyncSender<DevCommand>,
) -> Result<DevReply> {
    let payload = parse_json_payload(body)?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaHeadphoneDiscoveryFixture {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))
}

pub(super) fn post_qa_spinorama_discovery_fixture(
    body: &[u8],
    tx: &mpsc::SyncSender<DevCommand>,
) -> Result<DevReply> {
    let payload = parse_json_payload(body)?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaSpinoramaDiscoveryFixture {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))
}

pub(super) fn post_qa_room_eq(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    let payload = parse_json_payload(body)?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaRoomEq {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_qa_room_eq_ui_fixture(
    body: &[u8],
    tx: &mpsc::SyncSender<DevCommand>,
) -> Result<DevReply> {
    let payload = parse_json_payload(body)?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaRoomEqUiFixture {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))
}

pub(super) fn post_qa_room_eq_export_json(
    body: &[u8],
    tx: &mpsc::SyncSender<DevCommand>,
) -> Result<DevQueryReply> {
    let payload = if body.is_empty() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        parse_json_payload(body)?
    };
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaRoomEqExportJson {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}
