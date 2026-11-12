//! Hostile widget for the window probe (specs/009-declarative-widget-ui/research.md, R14; HS-05,
//! HS-16): content shaped like a host dialog asking for a password, in every tone and button
//! variant, whose buttons name the host's commands and other widgets' actions. It must neither
//! cover nor take the clicks or focus of anything outside its frame, and the host must refuse
//! every action it does not declare.

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "interactive-widget" });

use exports::dashboard::widget::actor::Guest as Actor;
use exports::dashboard::widget::renderer::Guest as Renderer;
use serde_json::json;

struct UiDialog;

impl Renderer for UiDialog {
    fn render(_config: String) -> Result<String, String> {
        // (label, action, variant): the host's commands, the showcase's and ui-invalid's actions,
        // and the one this widget declares
        let buttons = [
            ("Permitir", "widget_act", "primary"),
            ("Entrar", "widgets_list", "secondary"),
            ("Abrir configurações", "plugin:opener|open_url", "outline"),
            ("Enviar relatório", "shell_probe_report", "ghost"),
            ("Contar", "increment", "destructive"),
            ("Adicionar", "add", "primary"),
            ("Girar", "spin", "secondary"),
            ("Fechar", "dismiss", "ghost"),
        ];
        let buttons: Vec<_> = buttons
            .iter()
            .map(|(label, action, variant)| {
                json!({ "type": "button", "label": label, "action": action, "variant": variant })
            })
            .collect();
        Ok(json!({ "format": 1, "body": [
            { "type": "text", "text": "Confirme o acesso", "variant": "heading", "tone": "accent" },
            { "type": "text", "text": "O Dashboard precisa da sua senha para continuar.", "tone": "warning" },
            { "type": "text", "text": "Conta conectada", "tone": "ok" },
            { "type": "text", "text": "A sessão expirou", "tone": "error" },
            { "type": "text", "text": "Dashboard — sistema", "variant": "muted", "tone": "muted" },
            { "type": "stack", "children": buttons },
            { "type": "form",
              "fields": [{ "id": "password", "label": "Senha", "kind": "text" }],
              "submit": { "label": "Entrar", "action": "widget_render" } },
        ]})
        .to_string())
    }
}

impl Actor for UiDialog {
    fn act(_action: String, _input: String) -> Result<String, String> {
        Ok("{}".to_owned())
    }
}

export!(UiDialog);
