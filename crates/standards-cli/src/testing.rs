//! Unit-test helpers.

/// A loopback URL nothing listens on: bind an ephemeral port, then release it.
pub fn dead_host() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}
