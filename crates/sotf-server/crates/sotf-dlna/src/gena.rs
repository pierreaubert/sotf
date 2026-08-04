use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use uuid::Uuid;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1_800);
const MAX_TIMEOUT: Duration = Duration::from_secs(86_400);
const CALLBACK_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_SUBSCRIPTIONS: usize = 64;

#[derive(Clone)]
pub(crate) struct GenaRegistry {
    subscriptions: Arc<Mutex<HashMap<String, Subscription>>>,
}

#[derive(Clone)]
struct Subscription {
    service: String,
    callback: Callback,
    expires_at: Instant,
    next_sequence: u32,
    send_lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Clone)]
struct Callback {
    connect_host: String,
    host_header: String,
    port: u16,
    path: String,
}

pub(crate) struct GenaResponse {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
}

impl GenaRegistry {
    pub(crate) fn new() -> Self {
        Self {
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn subscribe(
        &self,
        service: &str,
        headers: &[(String, String)],
        initial_event: String,
    ) -> GenaResponse {
        let sid_header = header(headers, "sid");
        let callback_header = header(headers, "callback");
        let nt_header = header(headers, "nt");
        let timeout = parse_timeout(header(headers, "timeout"));

        if let Some(sid) = sid_header {
            if callback_header.is_some() || nt_header.is_some() {
                return GenaResponse::error(412);
            }

            let mut subscriptions = self.lock();
            let Some(subscription) = subscriptions.get_mut(sid.trim()) else {
                return GenaResponse::error(412);
            };
            if subscription.service != service {
                return GenaResponse::error(412);
            }
            if subscription.expires_at <= Instant::now() {
                subscriptions.remove(sid.trim());
                return GenaResponse::error(412);
            }
            subscription.expires_at = Instant::now() + timeout;
            return GenaResponse::ok(sid.trim().to_string(), timeout);
        }

        if !nt_header.is_some_and(|value| value.eq_ignore_ascii_case("upnp:event")) {
            return GenaResponse::error(412);
        }
        let Some(callback_header) = callback_header else {
            return GenaResponse::error(412);
        };
        let Some(callback) = parse_callback(callback_header) else {
            return GenaResponse::error(412);
        };

        let sid = format!("uuid:{}", Uuid::new_v4());
        let send_lock = Arc::new(tokio::sync::Mutex::new(()));
        {
            let mut subscriptions = self.lock();
            subscriptions.retain(|_, subscription| subscription.expires_at > Instant::now());
            if subscriptions.len() >= MAX_SUBSCRIPTIONS {
                return GenaResponse::error(503);
            }
            subscriptions.insert(
                sid.clone(),
                Subscription {
                    service: service.to_string(),
                    callback: callback.clone(),
                    expires_at: Instant::now() + timeout,
                    next_sequence: 1,
                    send_lock: Arc::clone(&send_lock),
                },
            );
        }

        self.spawn_notify(callback, sid.clone(), 0, initial_event, send_lock);
        GenaResponse::ok(sid, timeout)
    }

    pub(crate) fn unsubscribe(&self, service: &str, headers: &[(String, String)]) -> GenaResponse {
        let Some(sid) = header(headers, "sid") else {
            return GenaResponse::error(412);
        };

        let mut subscriptions = self.lock();
        let removed = subscriptions
            .get(sid.trim())
            .is_some_and(|subscription| subscription.service == service);
        if removed {
            subscriptions.remove(sid.trim());
        }
        if removed {
            GenaResponse::empty_ok()
        } else {
            GenaResponse::error(412)
        }
    }

    /// Send a property-change event to every live subscriber. Sequence
    /// numbers are assigned while holding the registry lock, so concurrent
    /// updates cannot deliver duplicate or reordered sequence numbers for a
    /// single subscription.
    pub(crate) fn notify(&self, service: &str, event: String) {
        let targets = {
            let mut subscriptions = self.lock();
            let now = Instant::now();
            subscriptions.retain(|_, subscription| subscription.expires_at > now);
            subscriptions
                .iter_mut()
                .filter(|(_, subscription)| subscription.service == service)
                .map(|(sid, subscription)| {
                    let sequence = subscription.next_sequence;
                    subscription.next_sequence = subscription.next_sequence.wrapping_add(1);
                    (
                        subscription.callback.clone(),
                        sid.clone(),
                        sequence,
                        Arc::clone(&subscription.send_lock),
                    )
                })
                .collect::<Vec<_>>()
        };

        for (callback, sid, sequence, send_lock) in targets {
            self.spawn_notify(callback, sid, sequence, event.clone(), send_lock);
        }
    }

    pub(crate) fn has_subscribers(&self, service: &str) -> bool {
        let mut subscriptions = self.lock();
        subscriptions.retain(|_, subscription| subscription.expires_at > Instant::now());
        subscriptions
            .values()
            .any(|subscription| subscription.service == service)
    }

    fn spawn_notify(
        &self,
        callback: Callback,
        sid: String,
        sequence: u32,
        event: String,
        send_lock: Arc<tokio::sync::Mutex<()>>,
    ) {
        tokio::spawn(async move {
            let _send_guard = send_lock.lock().await;
            if let Err(error) = send_notify(&callback, &sid, sequence, &event).await {
                log::debug!("[DLNA] GENA NOTIFY failed for {sid}: {error}");
            }
        });
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Subscription>> {
        self.subscriptions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl GenaResponse {
    fn ok(sid: String, timeout: Duration) -> Self {
        Self {
            status: 200,
            headers: vec![
                ("SID".to_string(), sid),
                ("TIMEOUT".to_string(), timeout_header(timeout)),
            ],
        }
    }

    fn empty_ok() -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
        }
    }

    fn error(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
        }
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn parse_timeout(value: Option<&str>) -> Duration {
    let Some(value) = value else {
        return DEFAULT_TIMEOUT;
    };
    let lower = value.trim().to_ascii_lowercase();
    let Some(seconds) = lower.strip_prefix("second-") else {
        return DEFAULT_TIMEOUT;
    };
    let Ok(seconds) = seconds.parse::<u64>() else {
        return DEFAULT_TIMEOUT;
    };
    Duration::from_secs(seconds).clamp(Duration::from_secs(1), MAX_TIMEOUT)
}

fn timeout_header(timeout: Duration) -> String {
    format!("Second-{}", timeout.as_secs())
}

fn parse_callback(value: &str) -> Option<Callback> {
    let callback = value
        .split('<')
        .nth(1)
        .and_then(|part| part.split('>').next())
        .unwrap_or(value)
        .trim();
    let rest = callback.strip_prefix("http://")?;
    if rest
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return None;
    }

    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() || authority.contains('@') {
        return None;
    }

    let (connect_host, port, host_header) = if let Some(host) = authority.strip_prefix('[') {
        let close = host.find(']')?;
        let hostname = &host[..close];
        let port = host[close + 1..]
            .strip_prefix(':')
            .map(str::parse)
            .transpose()
            .ok()?
            .unwrap_or(80);
        if hostname.is_empty() {
            return None;
        }
        (hostname.to_string(), port, format!("[{hostname}]:{port}"))
    } else {
        let (hostname, port) = authority
            .rsplit_once(':')
            .map(|(hostname, port)| (hostname, port.parse().ok()))
            .unwrap_or((authority, Some(80)));
        let port = port?;
        if hostname.is_empty() || hostname.contains(':') {
            return None;
        }
        (hostname.to_string(), port, format!("{hostname}:{port}"))
    };

    if port == 0 || path.len() > 2048 {
        return None;
    }
    Some(Callback {
        connect_host,
        host_header,
        port,
        path: format!("/{}", path.trim_start_matches('/')),
    })
}

async fn send_notify(
    callback: &Callback,
    sid: &str,
    sequence: u32,
    event: &str,
) -> Result<(), String> {
    let mut stream = tokio::time::timeout(
        CALLBACK_CONNECT_TIMEOUT,
        TcpStream::connect((callback.connect_host.as_str(), callback.port)),
    )
    .await
    .map_err(|_| "callback connect timeout".to_string())?
    .map_err(|error| error.to_string())?;
    let request = format!(
        "NOTIFY {} HTTP/1.1\r\n\
         HOST: {}\r\n\
         CONTENT-TYPE: text/xml; charset=\"utf-8\"\r\n\
         CONTENT-LENGTH: {}\r\n\
         NT: upnp:event\r\n\
         NTS: upnp:propchange\r\n\
         SID: {}\r\n\
         SEQ: {}\r\n\
         CONNECTION: close\r\n\
         \r\n\
         {}",
        callback.path,
        callback.host_header,
        event.len(),
        sid,
        sequence,
        event,
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stream.shutdown().await.map_err(|error| error.to_string())
}

pub(crate) fn event_property_set(properties: &[(&str, String)]) -> String {
    let mut body = String::from(
        r#"<?xml version="1.0" encoding="utf-8"?>
<e:propertyset xmlns:e="urn:schemas-upnp-org:event">"#,
    );
    for (name, value) in properties {
        body.push_str("<e:property><");
        body.push_str(name);
        body.push('>');
        body.push_str(&crate::xml::xml_escape(value));
        body.push_str("</");
        body.push_str(name);
        body.push_str("></e:property>");
    }
    body.push_str("</e:propertyset>");
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_callback_with_ipv4_and_default_port() {
        let callback = parse_callback("<http://127.0.0.1/events>").unwrap();
        assert_eq!(callback.connect_host, "127.0.0.1");
        assert_eq!(callback.port, 80);
        assert_eq!(callback.path, "/events");
    }

    #[test]
    fn rejects_unsafe_callback_urls() {
        assert!(parse_callback("<https://127.0.0.1/events>").is_none());
        assert!(parse_callback("<http://user@127.0.0.1/events>").is_none());
        assert!(parse_callback("<http://127.0.0.1:0/events>").is_none());
    }

    #[test]
    fn event_property_set_escapes_values() {
        let event = event_property_set(&[("Title", "A&B".to_string())]);
        assert!(event.contains("<Title>A&amp;B</Title>"));
        assert!(event.contains("urn:schemas-upnp-org:event"));
    }
}
