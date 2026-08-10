use super::super::commands::{DevCommand, DevQueryReply, DevReply};
use super::misc::REPLY_TIMEOUT;
use super::parse::parse_json_payload;
use anyhow::{Result, anyhow};
use std::sync::mpsc::{self, SyncSender};

pub(super) fn post_action(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
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

pub(super) fn post_key(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
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

pub(super) fn post_click(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
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

pub(super) fn post_quit(tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Quit { reply: reply_tx })
        .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

pub(super) fn post_qa_seed(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
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
    tx: &mpsc::Sender<DevCommand>,
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

pub(super) fn post_qa_room_eq(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
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

pub(super) fn post_qa_room_eq_export_json(
    body: &[u8],
    tx: &mpsc::Sender<DevCommand>,
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
