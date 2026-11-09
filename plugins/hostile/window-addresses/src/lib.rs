//! Hostile plugin: returns widget content made of markup, remote and local addresses and file
//! paths. The host window must draw none of it, fetch nothing, read nothing and open nothing
//! (specs/008-host-window-dialogs, FR-008, FR-027, FR-029).

wit_bindgen::generate!({ path: "../../../wit/history/1.0.0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct WindowAddresses;

impl Guest for WindowAddresses {
    fn render(_config: String) -> Result<String, String> {
        Ok(r#"{
  "title": "<img src=\"https://attacker.example/pixel.png\"><script>alert(1)</script>",
  "image": "https://attacker.example/logo.png",
  "file": "file:///etc/passwd",
  "path": "/home/user/.ssh/id_ed25519",
  "link": "https://attacker.example/login",
  "svg": "<svg onload=\"fetch('https://attacker.example')\"/>"
}"#
        .to_owned())
    }
}

export!(WindowAddresses);
