//! Cloud [`ChatBackend`]s over rig. Two wire formats: [`openai_compat`] for
//! every OpenAI-compatible endpoint (Cerebras, Groq, Together, OpenRouter,
//! Ollama, …) and [`anthropic`] for the Messages API. Both share the completion
//! path ([`complete_via`]) and differ only in client construction, caching, and
//! model listing. A future on-device backend bypasses rig entirely.
//!
//! Concrete rig client types are mostly not named — constructors return a boxed
//! [`ChatBackend`].

use async_trait::async_trait;
use lb_rs::model::chat::Usage;
use rig::client::{CompletionClient, ModelListingClient, ProviderClient};
use rig::completion::{CompletionModel, ToolDefinition};
use rig::message::{AssistantContent, Message as RigMessage};
use rig::providers::{anthropic, openai};

use super::backend::{ChatBackend, ChatMsg, CompletionReq, CompletionResp, ModelChoice, ToolCall};

// -- OpenAI-compatible --

/// Holds the client (not just the completion model) so the model is cheap to
/// derive per request; `base_url`/`api_key` back the model listing, which goes
/// straight to `GET /models` (rig's OpenAI lister is bound to its responses
/// client, not the chat-completions one we use here).
struct OpenAiBackend<C> {
    client: C,
    model_id: String,
    base_url: String,
    api_key: String,
}

/// Build a backend against an OpenAI-compatible endpoint: the chat-completions
/// client with a swapped base URL.
pub fn openai_compat(
    api_key: &str, base_url: &str, model: &str,
) -> Result<Box<dyn ChatBackend>, String> {
    let client = openai::CompletionsClient::builder()
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(|e| format!("failed to build client: {e}"))?;
    Ok(Box::new(OpenAiBackend {
        client,
        model_id: model.to_string(),
        base_url: base_url.to_string(),
        api_key: api_key.to_string(),
    }))
}

#[async_trait]
impl<C> ChatBackend for OpenAiBackend<C>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
    C::CompletionModel: Send + Sync,
{
    async fn complete(&self, req: CompletionReq) -> Result<CompletionResp, String> {
        complete_via(self.client.completion_model(&self.model_id), req).await
    }

    async fn list_models(&self) -> Result<Vec<ModelChoice>, String> {
        // OpenAI-standard `GET /models` → `{ "data": [ { "id": … }, … ] }`.
        #[derive(serde::Deserialize)]
        struct Resp {
            data: Vec<Entry>,
        }
        #[derive(serde::Deserialize)]
        struct Entry {
            id: String,
        }
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let resp = reqwest::Client::new()
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<Resp>()
            .await
            .map_err(|e| e.to_string())?;
        // `/models` carries no display names; the id (e.g. gemma-4-31b) is the
        // label.
        Ok(resp
            .data
            .into_iter()
            .map(|e| ModelChoice { label: e.id.clone(), id: e.id })
            .collect())
    }
}

// -- Anthropic (Messages API) --

struct AnthropicBackend {
    client: anthropic::Client,
    model_id: String,
}

/// Build an Anthropic Messages-API backend. The base URL is rig's default;
/// Anthropic isn't an OpenAI-compatible endpoint, so a custom `base_url` in
/// the provider config doesn't apply here.
pub fn anthropic(api_key: &str, model: &str) -> Result<Box<dyn ChatBackend>, String> {
    let client = anthropic::Client::from_val(api_key.to_string())
        .map_err(|e| format!("failed to build anthropic client: {e}"))?;
    Ok(Box::new(AnthropicBackend { client, model_id: model.to_string() }))
}

#[async_trait]
impl ChatBackend for AnthropicBackend {
    async fn complete(&self, req: CompletionReq) -> Result<CompletionResp, String> {
        // Automatic prompt caching with a 1h TTL: a human-paced chat is one
        // request per turn with minutes between sends, so the default 5m
        // entries would expire before the next request reads them.
        let model = self
            .client
            .completion_model(&self.model_id)
            .with_automatic_caching_1h();
        complete_via(model, req).await
    }

    async fn list_models(&self) -> Result<Vec<ModelChoice>, String> {
        let list = self.client.list_models().await.map_err(|e| e.to_string())?;
        Ok(list
            .iter()
            .map(|m| ModelChoice { id: m.id.clone(), label: m.display_name().to_string() })
            .collect())
    }
}

// -- Shared --

/// Run one completion request against any rig completion model and map the
/// response into our neutral types.
async fn complete_via<M: CompletionModel>(
    model: M, req: CompletionReq,
) -> Result<CompletionResp, String> {
    let tools: Vec<ToolDefinition> = req
        .tools
        .into_iter()
        .map(|t| ToolDefinition {
            name: t.name,
            description: t.description,
            parameters: t.parameters,
        })
        .collect();
    let mut messages = to_rig_messages(req.messages);
    // The builder takes the latest turn as the prompt and the rest as chat
    // history. Our loop always ends the conversation on the turn the model
    // must answer (a user message or a tool result).
    let prompt = messages.pop().ok_or("empty conversation")?;

    let resp = model
        .completion_request(prompt)
        .preamble(req.system)
        .messages(messages)
        .tools(tools)
        .max_tokens(req.max_tokens as u64)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for content in resp.choice.iter() {
        match content {
            AssistantContent::Text(t) => text.push_str(&t.text),
            AssistantContent::ToolCall(tc) => tool_calls.push(ToolCall {
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                args: tc.function.arguments.clone(),
            }),
            _ => {}
        }
    }

    let u = resp.usage;
    Ok(CompletionResp {
        text,
        tool_calls,
        usage: Usage {
            input: u.input_tokens,
            output: u.output_tokens,
            cache_read: u.cached_input_tokens,
            cache_write: u.cache_creation_input_tokens,
        },
    })
}

fn to_rig_messages(msgs: Vec<ChatMsg>) -> Vec<RigMessage> {
    msgs.into_iter()
        .map(|m| match m {
            ChatMsg::User(text) => RigMessage::user(text),
            ChatMsg::Assistant { text, tool_calls } if tool_calls.is_empty() => {
                RigMessage::assistant(text)
            }
            ChatMsg::Assistant { text, tool_calls } => {
                // Text (if any) followed by the tool-call blocks, in one
                // assistant message — the shape every provider expects.
                let mut content = Vec::new();
                if !text.is_empty() {
                    content.push(AssistantContent::text(text));
                }
                for c in tool_calls {
                    content.push(AssistantContent::tool_call(c.id, c.name, c.args));
                }
                RigMessage::Assistant {
                    id: None,
                    content: rig::OneOrMany::many(content)
                        .expect("tool_calls non-empty ⇒ content non-empty"),
                }
            }
            ChatMsg::ToolResult { id, content } => RigMessage::tool_result(id, content),
        })
        .collect()
}
