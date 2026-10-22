//! Hostile plugin: reads a byte far beyond its own linear memory. WebAssembly bounds checks
//! must trap, so the plugin can never see the host's memory (FR-008).

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct OutOfBounds;

impl Guest for OutOfBounds {
    fn render(_config: String) -> Result<String, String> {
        // Near the top of the 4 GiB address space; the plugin's memory is a few MiB at most.
        let address = std::ptr::without_provenance::<u8>(0xFFFF_FFF0);
        // SAFETY: none; reading invalid memory is what this fixture is for.
        let byte = unsafe { std::ptr::read_volatile(address) };
        Ok(byte.to_string())
    }
}

export!(OutOfBounds);
