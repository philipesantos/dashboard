//! Hostile plugin: allocates 1 GiB. The memory cap must refuse the growth.

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct HugeAlloc;

impl Guest for HugeAlloc {
    fn render(_config: String) -> Result<String, String> {
        let block = std::hint::black_box(vec![1u8; 1 << 30]);
        Ok(block.len().to_string())
    }
}

export!(HugeAlloc);
