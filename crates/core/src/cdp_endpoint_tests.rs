use super::*;

#[test]
fn pick_free_port_returns_a_port_that_is_immediately_bindable_again() {
    let port = pick_free_port().unwrap();

    assert!(port_is_free(port));
}

#[test]
fn port_is_free_reports_false_while_a_listener_holds_the_port_and_true_once_released() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    assert!(!port_is_free(port));
    drop(listener);
    assert!(port_is_free(port));
}

/// Serves a canned `/json/version` response over a real socket so the parse
/// path is exercised end to end, not just against an in-memory byte slice.
#[test]
fn probe_parses_websocket_url_and_product_from_a_live_http_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 1024];
        let received = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..received]).into_owned();
        let body = r#"{"Browser":"Chrome/120.0.0.0","webSocketDebuggerUrl":"ws://127.0.0.1/devtools/browser/abc"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
        request
    });

    let endpoint = probe(port, Deadline::after(2_000).unwrap()).unwrap();
    let request = server.join().unwrap();

    assert!(
        request.contains(&format!("Host: 127.0.0.1:{port}")),
        "Chromium echoes the Host header into webSocketDebuggerUrl, so the port must be in it; sent: {request}"
    );

    assert_eq!(endpoint.port, port);
    assert_eq!(endpoint.http_endpoint, format!("http://127.0.0.1:{port}"));
    assert_eq!(endpoint.product.as_deref(), Some("Chrome/120.0.0.0"));
    assert_eq!(
        endpoint.websocket_url.as_deref(),
        Some("ws://127.0.0.1/devtools/browser/abc")
    );
}

/// Chromium's real DevTools server never closes the connection, even when
/// asked to — the launch-blocking bug this pins was a probe that read to EOF
/// and timed out on every live endpoint. The server here answers and then
/// holds the socket open, so this test fails if the probe ever needs EOF.
#[test]
fn probe_completes_against_a_server_that_never_closes_the_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 1024];
        let _ = stream.read(&mut buf);
        let body = r#"{"Browser":"Chrome/142.0.0.0","webSocketDebuggerUrl":"ws://127.0.0.1/devtools/browser/def"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
        let _ = done_rx.recv_timeout(Duration::from_secs(10));
        drop(stream);
    });

    let started = std::time::Instant::now();
    let endpoint = probe(port, Deadline::after(5_000).unwrap()).unwrap();
    let elapsed = started.elapsed();
    done_tx.send(()).unwrap();
    server.join().unwrap();

    assert!(elapsed < Duration::from_secs(2), "probe took {elapsed:?}");
    assert_eq!(endpoint.product.as_deref(), Some("Chrome/142.0.0.0"));
    assert_eq!(
        endpoint.websocket_url.as_deref(),
        Some("ws://127.0.0.1/devtools/browser/def")
    );
}

#[test]
fn probe_returns_the_unavailable_error_quickly_when_nothing_is_listening() {
    let port = pick_free_port().unwrap();
    let started = std::time::Instant::now();

    let error = probe(port, Deadline::after(300).unwrap()).unwrap_err();

    assert!(started.elapsed() < Duration::from_secs(2));
    match error {
        AppError::Adapter(inner) => {
            assert_eq!(inner.code, ErrorCode::ActionFailed);
            assert_eq!(
                inner.details.as_ref().and_then(|d| d.get("kind")),
                Some(&serde_json::json!("cdp_endpoint_unavailable"))
            );
        }
        other => panic!("expected an adapter error, got {other:?}"),
    }
}
