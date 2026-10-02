//! First run: pick a provider, paste a key. Provider files are ordinary
//! encrypted documents under `/.agent/providers/`; the default lives in
//! `/.agent/default.json`.

use lb_rs::blocking::Lb;
use serde_json::json;

pub struct Template {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: &'static str,
    pub base_url: &'static str,
    pub model: &'static str,
    pub needs_key: bool,
}

pub const TEMPLATES: &[Template] = &[
    Template {
        name: "anthropic",
        label: "Anthropic",
        kind: "anthropic",
        base_url: "https://api.anthropic.com/v1",
        model: "claude-opus-5-5",
        needs_key: true,
    },
    Template {
        name: "openai",
        label: "OpenAI",
        kind: "openai",
        base_url: "https://api.openai.com/v1",
        model: "gpt-5.5",
        needs_key: true,
    },
    Template {
        name: "google",
        label: "Google",
        kind: "openai",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        model: "gemini-3.5-flash",
        needs_key: true,
    },
    Template {
        name: "xai",
        label: "xAI",
        kind: "openai",
        base_url: "https://api.x.ai/v1",
        model: "grok-4.3",
        needs_key: true,
    },
    Template {
        name: "openrouter",
        label: "OpenRouter",
        kind: "openai",
        base_url: "https://openrouter.ai/api/v1",
        model: "openrouter/auto",
        needs_key: true,
    },
    Template {
        name: "groq",
        label: "Groq",
        kind: "openai",
        base_url: "https://api.groq.com/openai/v1",
        model: "openai/gpt-oss-120b",
        needs_key: true,
    },
    Template {
        name: "cerebras",
        label: "Cerebras",
        kind: "openai",
        base_url: "https://api.cerebras.ai/v1",
        model: "gemma-4-31b",
        needs_key: true,
    },
    Template {
        name: "ollama",
        label: "Ollama (this machine)",
        kind: "openai",
        base_url: "http://localhost:11434/v1",
        model: "",
        needs_key: false,
    },
    Template {
        name: "custom",
        label: "Custom endpoint",
        kind: "openai",
        base_url: "http://100.64.0.1:11434/v1",
        model: "",
        needs_key: false,
    },
];

#[derive(Default)]
pub struct Setup {
    pub picked: Option<&'static Template>,
    pub key: String,
    pub model: String,
    pub base_url: String,
    pub error: Option<String>,
}

impl Setup {
    pub fn pick(&mut self, template: &'static Template) {
        self.picked = Some(template);
        self.key.clear();
        self.model = template.model.to_string();
        self.base_url = template.base_url.to_string();
        self.error = None;
    }

    /// Writes the provider file and makes it the default.
    pub fn connect(&mut self, lb: &Lb) -> Result<(), String> {
        let Some(t) = self.picked else { return Err("pick a provider".into()) };
        let key = self.key.trim();
        if t.needs_key && key.is_empty() {
            return Err("paste an API key".into());
        }
        if self.model.trim().is_empty() {
            return Err("name a model".into());
        }
        let mut file = json!({
            "display_name": t.label,
            "kind": t.kind,
            "base_url": self.base_url.trim(),
            "model": self.model.trim(),
        });
        if !key.is_empty() {
            file["api_key"] = json!(key);
        }
        write(
            lb,
            &format!("/.agent/providers/{}.json", t.name),
            &serde_json::to_vec_pretty(&file).unwrap(),
        )?;
        write(
            lb,
            "/.agent/default.json",
            &serde_json::to_vec_pretty(&json!({ "provider": t.name })).unwrap(),
        )?;
        self.key.clear();
        Ok(())
    }
}

/// Names of the provider files that exist.
pub fn providers(lb: &Lb) -> Vec<String> {
    let Ok(folder) = lb.get_by_path("/.agent/providers/") else { return Vec::new() };
    let mut names: Vec<String> = lb
        .get_children(&folder.id)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| f.name.strip_suffix(".json").map(str::to_string))
        .collect();
    names.sort();
    names
}

fn write(lb: &Lb, path: &str, bytes: &[u8]) -> Result<(), String> {
    let file = match lb.get_by_path(path) {
        Ok(f) => f,
        Err(_) => lb.create_at_path(path).map_err(|e| e.to_string())?,
    };
    lb.write_document(file.id, bytes).map_err(|e| e.to_string())
}
