//! Hostile widget for the window probe (specs/009-declarative-widget-ui/research.md, R14; HS-03,
//! HS-04): content wider and taller than any frame, with unbroken text, a wide table, a long list
//! and a dense chart, nested as deep as the format allows. Nothing of it may paint or be hit
//! outside the frame's body.

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;
use serde_json::{Value, json};

struct UiOverflow;

impl Guest for UiOverflow {
    fn render(_config: String) -> Result<String, String> {
        let unbroken = "W".repeat(2000);
        let columns: Vec<String> = (0..20)
            .map(|i| format!("coluna{i}{}", "x".repeat(80)))
            .collect();
        let rows: Vec<Vec<String>> = (0..10)
            .map(|r| {
                (0..20)
                    .map(|c| format!("{r}.{c}{}", "y".repeat(90)))
                    .collect()
            })
            .collect();
        let items: Vec<String> = (0..150)
            .map(|i| format!("item {i} {}", "z".repeat(20)))
            .collect();
        let labels: Vec<String> = (0..200).map(|i| format!("r{i}")).collect();
        let values: Vec<f64> = (0..200).map(|i| f64::from(i % 17) * 1e6).collect();
        // Six stacks deep, each with an unbroken line
        let mut nested: Value = json!({ "type": "text", "text": unbroken });
        for _ in 0..6 {
            nested = json!({ "type": "stack", "children": [
                { "type": "text", "text": "M".repeat(400), "variant": "heading" },
                nested,
            ]});
        }
        Ok(json!({ "format": 1, "body": [
            { "type": "text", "text": unbroken, "variant": "heading" },
            { "type": "table", "columns": columns, "rows": rows },
            { "type": "list", "items": items },
            { "type": "chart", "kind": "bar", "labels": labels,
              "series": [{ "name": "N".repeat(300), "values": values }] },
            nested,
        ]})
        .to_string())
    }
}

export!(UiOverflow);
