//! Hostile plugin: recurses until its stack runs out. The call must fail as a crash and leave
//! the host running.

wit_bindgen::generate!({ path: "../../../wit/v0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct DeepRecursion;

impl Guest for DeepRecursion {
    fn render(_config: String) -> Result<String, String> {
        Ok(depth(0).to_string())
    }
}

// Recursing without end is the point of this fixture.
#[allow(unconditional_recursion)]
fn depth(n: u64) -> u64 {
    // A frame the compiler cannot drop, so the recursion cannot become a loop.
    let frame = std::hint::black_box([n; 64]);
    depth(frame[0] + 1) + frame[63]
}

export!(DeepRecursion);
