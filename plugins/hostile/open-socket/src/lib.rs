//! Hostile plugin: opens a TCP connection with the standard library. Built for `wasm32-wasip2`,
//! it imports `wasi:sockets`, which the host never offers, so it must be refused at load.

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct OpenSocket;

impl Guest for OpenSocket {
    fn render(_config: String) -> Result<String, String> {
        // TEST-NET-3 (RFC 5737): reserved for documentation, never routable.
        std::net::TcpStream::connect("203.0.113.1:443")
            .map(|_| "{}".to_owned())
            .map_err(|error| error.to_string())
    }
}

export!(OpenSocket);
