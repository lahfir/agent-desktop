use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{AdapterError, AppError, Deadline, ErrorCode};

const MAX_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(500);
const READ_TIMEOUT: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A DevTools protocol endpoint verified on a launched process, not merely
/// requested of it. `port` and `http_endpoint` come from the launch itself;
/// `websocket_url` and `product` come from a live `/json/version` read and
/// are absent when the endpoint answers but its body cannot be parsed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpEndpoint {
    pub port: u16,
    pub http_endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub websocket_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
}

/// Binds an ephemeral port to learn one the OS considers free, then releases
/// it immediately. The gap between release and reuse is inherent to asking
/// the OS for a free port this way; the launched process claiming it first
/// is the expected outcome, not a race this function needs to close.
pub(crate) fn pick_free_port() -> Result<u16, AppError> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(bind_failed)?;
    let port = listener.local_addr().map_err(bind_failed)?.port();
    drop(listener);
    Ok(port)
}

pub(crate) fn port_is_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Polls `/json/version` until it answers or the deadline runs out. The
/// port only exists once the launched process opens it, so an endpoint that
/// is not yet listening is the expected state right after launch, not a
/// failure — the loop is what turns "port injected" into "port verified".
pub(crate) fn probe(port: u16, deadline: Deadline) -> Result<CdpEndpoint, AppError> {
    loop {
        let attempt_timeout = deadline.remaining().min(MAX_ATTEMPT_TIMEOUT);
        if let Some(endpoint) = attempt(port, attempt_timeout.max(Duration::from_millis(1))) {
            return Ok(endpoint);
        }
        if deadline.is_expired() {
            return Err(unavailable(port, deadline));
        }
        std::thread::sleep(POLL_INTERVAL.min(deadline.remaining()));
    }
}

fn attempt(port: u16, timeout: Duration) -> Option<CdpEndpoint> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, timeout).ok()?;
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(READ_TIMEOUT)).ok()?;
    stream
        .write_all(b"GET /json/version HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    Some(parse_response(port, &raw))
}

fn parse_response(port: u16, raw: &[u8]) -> CdpEndpoint {
    let text = String::from_utf8_lossy(raw);
    let body = text
        .split_once("\r\n\r\n")
        .map_or("", |(_, body)| body)
        .trim();
    let parsed: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let product = parsed
        .as_ref()
        .and_then(|value| value.get("Browser"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let websocket_url = parsed
        .as_ref()
        .and_then(|value| value.get("webSocketDebuggerUrl"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    CdpEndpoint {
        port,
        http_endpoint: format!("http://127.0.0.1:{port}"),
        websocket_url,
        product,
    }
}

fn bind_failed(error: std::io::Error) -> AppError {
    AppError::Adapter(
        AdapterError::new(
            ErrorCode::Internal,
            "Could not bind a local port to pick a free one",
        )
        .with_platform_detail(error.to_string()),
    )
}

fn unavailable(port: u16, deadline: Deadline) -> AppError {
    AppError::Adapter(
        AdapterError::new(
            ErrorCode::ActionFailed,
            "The DevTools endpoint never answered before the deadline",
        )
        .with_details(serde_json::json!({
            "kind": "cdp_endpoint_unavailable",
            "port": port,
            "elapsed_ms": deadline.elapsed().as_millis(),
        }))
        .with_disposition(crate::DeliverySemantics::delivered_unverified()),
    )
}

#[cfg(test)]
#[path = "cdp_endpoint_tests.rs"]
mod tests;
