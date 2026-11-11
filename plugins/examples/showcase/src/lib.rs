//! The showcase: the widget the app ships, built into it (constitution v2.6.3, Principle VII).
//! It shows every component, tone and variant of the declarative UI
//! (`specs/009-declarative-widget-ui/contracts/content-format.md`), and two actions that need no
//! permission: `increment` counts, and `add` puts the form's title in the list. Its state is the
//! data its actions return, which the host keeps in memory while the app runs, so it starts over
//! when the app restarts.

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "interactive-widget" });

use exports::dashboard::widget::actor::Guest as Actor;
use exports::dashboard::widget::renderer::Guest as Renderer;
use serde_json::{Value, json};

/// How many titles the list keeps.
const MAX_ADDED: usize = 20;

struct Showcase;

/// The state in the data the last action returned.
fn state(data: &Value) -> (u64, Vec<String>) {
    let count = data["count"].as_u64().unwrap_or(0);
    let added = data["added"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    (count, added)
}

impl Renderer for Showcase {
    fn render(config: String) -> Result<String, String> {
        let document: Value = serde_json::from_str(&config).map_err(|error| error.to_string())?;
        let (count, added) = state(&document["data"]);
        let mut items = vec![
            json!("Texto, lista e tabela"),
            json!({ "text": "Gráficos desenhados pelo host", "tone": "accent" }),
            json!({ "text": "Nenhum campo de senha", "tone": "ok" }),
        ];
        items.extend(
            added
                .iter()
                .map(|title| json!({ "text": title, "tone": "muted" })),
        );
        let content = json!({
            "format": 1,
            "body": [
                { "type": "text", "text": "Vitrine dos componentes", "variant": "heading" },
                { "type": "text", "text": "Tudo o que um widget pode desenhar, sempre pelo host.", "variant": "muted" },
                { "type": "stack", "children": [
                    { "type": "text", "text": "Sincronizado", "tone": "ok" },
                    { "type": "text", "text": "Cota quase cheia", "tone": "warning" },
                    { "type": "text", "text": "Falha ao atualizar", "tone": "error" },
                    { "type": "text", "text": "Em destaque", "tone": "accent" }
                ]},
                { "type": "list", "items": items },
                { "type": "table", "columns": ["Componente", "Para quê"], "rows": [
                    ["Texto", "Títulos, avisos e status"],
                    ["Tabela", "Dados em linhas e colunas"],
                    ["Formulário", "Ações com valores"]
                ]},
                { "type": "chart", "kind": "bar", "labels": ["seg", "ter", "qua", "qui", "sex"],
                  "series": [{ "name": "Commits", "values": [3, 5, 2, 8, 4] }] },
                { "type": "chart", "kind": "line", "labels": ["jan", "fev", "mar", "abr"],
                  "series": [
                    { "name": "Abertas", "values": [12, 9, 7, 4] },
                    { "name": "Fechadas", "values": [2, 6, 9, 13] }
                  ] },
                { "type": "text", "text": format!("Contagem: {count}") },
                { "type": "stack", "children": [
                    { "type": "button", "label": "Contar", "action": "increment", "variant": "primary" },
                    { "type": "button", "label": "Secundário", "action": "increment", "variant": "secondary" },
                    { "type": "button", "label": "Contorno", "action": "increment", "variant": "outline" },
                    { "type": "button", "label": "Fantasma", "action": "increment", "variant": "ghost" },
                    { "type": "button", "label": "Destrutivo", "action": "increment", "variant": "destructive" }
                ]},
                { "type": "form",
                  "fields": [
                    { "id": "title", "label": "Título", "kind": "text" },
                    { "id": "priority", "label": "Prioridade", "kind": "select", "options": ["baixa", "média", "alta"] },
                    { "id": "estimate", "label": "Estimativa (horas)", "kind": "number" },
                    { "id": "done", "label": "Feita", "kind": "checkbox" }
                  ],
                  "submit": { "label": "Adicionar", "action": "add" } }
            ]
        });
        Ok(content.to_string())
    }
}

impl Actor for Showcase {
    fn act(action: String, input: String) -> Result<String, String> {
        let input: Value = serde_json::from_str(&input).map_err(|error| error.to_string())?;
        let (mut count, mut added) = state(&input["data"]);
        match action.as_str() {
            "increment" => count += 1,
            "add" => {
                let title = input["values"]["title"].as_str().unwrap_or_default().trim();
                if title.is_empty() {
                    return Err("give the item a title".to_owned());
                }
                added.insert(0, title.chars().take(80).collect());
                added.truncate(MAX_ADDED);
            }
            other => return Err(format!("unknown action {other}")),
        }
        Ok(json!({ "count": count, "added": added }).to_string())
    }
}

export!(Showcase);
