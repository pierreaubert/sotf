use super::handle::content_directory_event_body;
use super::handle::handle_server_request;
use super::media_server_adapter::MediaServerAdapter;
use crate::device::DlnaDevice;
use crate::gena::GenaRegistry;
use crate::ssdp;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::Duration;

/// DLNA MediaServer — serves library content to DLNA controllers.
pub struct DlnaMediaServer {
    pub(super) device: DlnaDevice,
    pub(super) adapter: Arc<dyn MediaServerAdapter>,
    pub(super) events: GenaRegistry,
}

impl DlnaMediaServer {
    pub fn new(device: DlnaDevice, adapter: Arc<dyn MediaServerAdapter>) -> Self {
        Self {
            device,
            adapter,
            events: GenaRegistry::new(),
        }
    }

    pub async fn run(
        &self,
        bind_address: &str,
        local_ip: Ipv4Addr,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), String> {
        let bind_label = format!("{bind_address}:{}", self.device.http_port);
        let listener = TcpListener::bind((bind_address, self.device.http_port))
            .await
            .map_err(|e| format!("Failed to bind server HTTP on {bind_label}: {e}"))?;

        ssdp::send_alive(&self.device, local_ip).await?;

        let ssdp_device = self.device.clone();
        let ssdp_cancel = cancel.clone();
        tokio::spawn(async move {
            if let Err(e) = ssdp::listen_and_respond(ssdp_device, local_ip, ssdp_cancel).await {
                log::warn!("[DLNA Server] SSDP error: {}", e);
            }
        });

        let event_adapter = Arc::clone(&self.adapter);
        let event_registry = self.events.clone();
        let event_cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut last_update = event_adapter.content_directory_update_id();
            let mut cancel_rx = event_cancel;
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if !event_registry.has_subscribers("/ContentDirectory/event") {
                            continue;
                        }
                        let update_id = event_adapter.content_directory_update_id();
                        if update_id != last_update {
                            last_update = update_id;
                            event_registry.notify(
                                "/ContentDirectory/event",
                                content_directory_event_body(update_id),
                            );
                        }
                    }
                    changed = cancel_rx.changed() => {
                        if changed.is_err() || *cancel_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        log::info!(
            "[DLNA Server] '{}' running on {}",
            self.device.friendly_name,
            bind_label,
        );

        loop {
            let mut cancel_rx = cancel.clone();
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let adapter = Arc::clone(&self.adapter);
                            let device = self.device.clone();
                            let events = self.events.clone();
                            let base = format!("http://{}:{}", local_ip, device.http_port);
                            tokio::spawn(async move {
                                if let Err(e) = handle_server_request(
                                    stream,
                                    &device,
                                    &base,
                                    &adapter,
                                    &events,
                                )
                                .await
                                {
                                    log::debug!("[DLNA Server] HTTP error: {}", e);
                                }
                            });
                        }
                        Err(e) => log::warn!("[DLNA Server] Accept error: {}", e),
                    }
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        ssdp::send_byebye(&self.device).await.ok();
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}
