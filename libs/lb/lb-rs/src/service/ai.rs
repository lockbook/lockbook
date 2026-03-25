use serde::{Deserialize, Serialize};

use crate::Lb;
use crate::model::chat::Message;
use crate::model::errors::{LbErrKind, LbResult};

pub const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const MODEL: &str = "claude-sonnet-4-20250514";
pub const MAX_TOKENS: u32 = 4096;

// --- Claude API types ---

#[derive(Serialize, Clone, Debug)]
pub struct Request {
    pub model: String,
    pub max_tokens: u32,
    pub system: Vec<SystemContent>,
    pub tools: Vec<Tool>,
    pub messages: Vec<ApiMessage>,
}

#[derive(Serialize, Clone, Debug)]
pub struct SystemContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub control_type: String,
}

impl CacheControl {
    pub fn ephemeral() -> Self {
        Self { control_type: "ephemeral".into() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiMessage {
    pub role: String,
    pub content: Vec<Content>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String, input: serde_json::Value },
    #[serde(rename = "tool_result")]
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Deserialize, Debug)]
pub struct ApiResponse {
    pub content: Vec<Content>,
    pub stop_reason: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize, Debug)]
pub struct Usage {
    pub input_tokens: Option<u32>,
    pub cache_creation_input_tokens: Option<u32>,
    pub cache_read_input_tokens: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

// --- Public interface ---

/// A tool call extracted from an API response, with a human-readable description.
#[derive(Clone, Debug)]
pub struct ToolRequest {
    pub tool_use_id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub description: String,
}

impl ToolRequest {
    pub fn basic_description(name: &str, input: &serde_json::Value) -> String {
        let path = |key: &str| input[key].as_str().unwrap_or("?").to_string();
        match name {
            "list_files" => "List all files".into(),
            "read_document" => format!("Read {}", path("path")),
            "search" => format!("Search for \"{}\"", path("query")),
            "write_document" => format!("Write to {}", path("path")),
            "create_document" => format!("Create {}", path("path")),
            "create_folder" => format!("Create folder {}", path("path")),
            "move_file" => format!("Move {} to {}", path("path"), path("destination")),
            "rename_file" => format!("Rename {} to {}", path("path"), path("new_name")),
            "delete_file" => format!("Delete {}", path("path")),
            _ => format!("Unknown tool: {name}"),
        }
    }
}

pub fn tools() -> Vec<Tool> {
    let mut tools = vec![
        Tool {
            name: "list_files".into(),
            description: "List all files and folders in the user's lockbook as a path tree."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            cache_control: None,
        },
        Tool {
            name: "read_document".into(),
            description: "Read the contents of a document by its path.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path, e.g. 'username/notes/todo.md'"
                    }
                },
                "required": ["path"]
            }),
            cache_control: None,
        },
        Tool {
            name: "search".into(),
            description: "Search for files by name or content. Returns matching file paths."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                },
                "required": ["query"]
            }),
            cache_control: None,
        },
        Tool {
            name: "write_document".into(),
            description: "Write or update the contents of a document by its path.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path, e.g. 'username/notes/todo.md'"
                    },
                    "content": {
                        "type": "string",
                        "description": "The new content for the document"
                    }
                },
                "required": ["path", "content"]
            }),
            cache_control: None,
        },
        Tool {
            name: "create_document".into(),
            description: "Create a new document at the given path. Parent folders are created \
                          automatically."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The full path for the new document, e.g. 'username/notes/todo.md'"
                    },
                    "content": {
                        "type": "string",
                        "description": "Initial content for the document (optional)"
                    }
                },
                "required": ["path"]
            }),
            cache_control: None,
        },
        Tool {
            name: "create_folder".into(),
            description: "Create a new folder at the given path. Parent folders are created \
                          automatically. Path must end with a trailing slash."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The full path for the new folder, e.g. 'username/projects/'"
                    }
                },
                "required": ["path"]
            }),
            cache_control: None,
        },
        Tool {
            name: "move_file".into(),
            description: "Move a file or folder to a new parent folder.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path of the file to move, e.g. 'username/old-folder/doc.md'"
                    },
                    "destination": {
                        "type": "string",
                        "description": "The path of the destination folder, e.g. 'username/new-folder/'"
                    }
                },
                "required": ["path", "destination"]
            }),
            cache_control: None,
        },
        Tool {
            name: "rename_file".into(),
            description: "Rename a file or folder.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The current path of the file, e.g. 'username/notes/old-name.md'"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "The new name (not a full path), e.g. 'new-name.md'"
                    }
                },
                "required": ["path", "new_name"]
            }),
            cache_control: None,
        },
        Tool {
            name: "delete_file".into(),
            description: "Delete a file or folder.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path of the file to delete, e.g. 'username/notes/old.md'"
                    }
                },
                "required": ["path"]
            }),
            cache_control: None,
        },
    ];
    // Cache breakpoint on the last tool — everything above (system + all tools)
    // gets cached across requests.
    if let Some(last) = tools.last_mut() {
        last.cache_control = Some(CacheControl::ephemeral());
    }
    tools
}

pub fn system_content() -> Vec<SystemContent> {
    vec![SystemContent {
        content_type: "text".into(),
        text: "You are an AI assistant embedded in Lockbook, a secure note-taking app. You have \
               full access to the user's files: you can list, read, write, create, rename, move, \
               and delete documents and folders. All file operations use paths (e.g. \
               'username/folder/file.md'). Use list_files first to discover the path structure. \
               Be concise and helpful."
            .into(),
        cache_control: None,
    }]
}

/// Convert chat display messages into API messages for the initial request.
/// Used when there's no existing API message history (e.g. tab was reopened).
pub fn chat_to_api_messages(messages: &[Message]) -> Vec<ApiMessage> {
    messages
        .iter()
        .map(|m| {
            let role = if m.from == "agent" { "assistant".into() } else { "user".into() };
            ApiMessage { role, content: vec![Content::Text { text: m.content.clone() }] }
        })
        .collect()
}

/// Extract tool requests from an API response with basic descriptions.
pub fn extract_tool_requests(response: &ApiResponse) -> Vec<ToolRequest> {
    response
        .content
        .iter()
        .filter_map(|c| match c {
            Content::ToolUse { id, name, input } => {
                let description = ToolRequest::basic_description(name, input);
                Some(ToolRequest {
                    tool_use_id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    description,
                })
            }
            _ => None,
        })
        .collect()
}

/// Extract final text from an API response.
pub fn extract_text(response: &ApiResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

impl Lb {
    /// Send messages to the Claude API and return the raw response.
    #[instrument(level = "debug", skip(self, api_messages), err(Debug))]
    pub async fn ai_send(&self, api_messages: Vec<ApiMessage>) -> LbResult<ApiResponse> {
        let api_key = self.get_ai_api_key()?;

        let request = Request {
            model: MODEL.into(),
            max_tokens: MAX_TOKENS,
            system: system_content(),
            tools: tools(),
            messages: api_messages,
        };

        let resp = self
            .client
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| LbErrKind::Unexpected(format!("AI request failed: {e}")))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| LbErrKind::Unexpected(format!("AI response read failed: {e}")))?;

        if !status.is_success() {
            return Err(LbErrKind::Unexpected(format!("AI API error ({status}): {body}")).into());
        }

        serde_json::from_str(&body)
            .map_err(|e| LbErrKind::Unexpected(format!("AI response parse failed: {e}\nbody: {body}")).into())
    }

    /// Execute a single tool call and return the result string.
    pub async fn ai_execute_tool(&self, name: &str, input: &serde_json::Value) -> String {
        let path = |key: &str| input[key].as_str().unwrap_or_default();
        match name {
            "list_files" => self.tool_list_files().await,
            "read_document" => self.tool_read_document(path("path")).await,
            "search" => self.tool_search(path("query")).await,
            "write_document" => {
                self.tool_write_document(path("path"), path("content")).await
            }
            "create_document" => {
                self.tool_create_document(path("path"), input["content"].as_str()).await
            }
            "create_folder" => self.tool_create_folder(path("path")).await,
            "move_file" => self.tool_move_file(path("path"), path("destination")).await,
            "rename_file" => self.tool_rename_file(path("path"), path("new_name")).await,
            "delete_file" => self.tool_delete_file(path("path")).await,
            _ => format!("Unknown tool: {name}"),
        }
    }

    async fn tool_list_files(&self) -> String {
        match self.list_paths(None).await {
            Ok(paths) => {
                if paths.is_empty() {
                    "No files found.".into()
                } else {
                    paths.join("\n")
                }
            }
            Err(e) => format!("Error listing files: {e}"),
        }
    }

    async fn tool_read_document(&self, path: &str) -> String {
        let file = match self.get_by_path(path).await {
            Ok(f) => f,
            Err(e) => return format!("Error: {e}"),
        };
        match self.read_document(file.id, false).await {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => format!("Error reading document: {e}"),
        }
    }

    async fn tool_search(&self, query: &str) -> String {
        #[cfg(not(target_family = "wasm"))]
        {
            use crate::subscribers::search::SearchConfig;
            match self.search(query, SearchConfig::PathsAndDocuments).await {
                Ok(results) => {
                    if results.is_empty() {
                        "No results found.".into()
                    } else {
                        results.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>().join("\n")
                    }
                }
                Err(e) => format!("Error searching: {e}"),
            }
        }
        #[cfg(target_family = "wasm")]
        {
            let _ = query;
            "Search is not available on this platform.".into()
        }
    }

    async fn tool_write_document(&self, path: &str, content: &str) -> String {
        let file = match self.get_by_path(path).await {
            Ok(f) => f,
            Err(e) => return format!("Error: {e}"),
        };
        match self.write_document(file.id, content.as_bytes()).await {
            Ok(()) => "Document written successfully.".into(),
            Err(e) => format!("Error writing document: {e}"),
        }
    }

    async fn tool_create_document(&self, path: &str, content: Option<&str>) -> String {
        match self.create_at_path(path).await {
            Ok(file) => {
                if let Some(content) = content {
                    if let Err(e) = self.write_document(file.id, content.as_bytes()).await {
                        return format!("Created document but failed to write content: {e}");
                    }
                }
                format!("Created {path}")
            }
            Err(e) => format!("Error creating document: {e}"),
        }
    }

    async fn tool_create_folder(&self, path: &str) -> String {
        let path = if path.ends_with('/') { path.to_string() } else { format!("{path}/") };
        match self.create_at_path(&path).await {
            Ok(_) => format!("Created folder {path}"),
            Err(e) => format!("Error creating folder: {e}"),
        }
    }

    async fn tool_move_file(&self, path: &str, destination: &str) -> String {
        let file = match self.get_by_path(path).await {
            Ok(f) => f,
            Err(e) => return format!("Error finding file: {e}"),
        };
        let dest = match self.get_by_path(destination).await {
            Ok(f) => f,
            Err(e) => return format!("Error finding destination: {e}"),
        };
        match self.move_file(&file.id, &dest.id).await {
            Ok(()) => format!("Moved {path} to {destination}"),
            Err(e) => format!("Error moving file: {e}"),
        }
    }

    async fn tool_rename_file(&self, path: &str, new_name: &str) -> String {
        let file = match self.get_by_path(path).await {
            Ok(f) => f,
            Err(e) => return format!("Error finding file: {e}"),
        };
        match self.rename_file(&file.id, new_name).await {
            Ok(()) => format!("Renamed to {new_name}"),
            Err(e) => format!("Error renaming file: {e}"),
        }
    }

    async fn tool_delete_file(&self, path: &str) -> String {
        let file = match self.get_by_path(path).await {
            Ok(f) => f,
            Err(e) => return format!("Error finding file: {e}"),
        };
        match self.delete(&file.id).await {
            Ok(()) => format!("Deleted {path}"),
            Err(e) => format!("Error deleting file: {e}"),
        }
    }

    pub fn get_ai_api_key(&self) -> LbResult<String> {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.is_empty() {
                return Ok(key);
            }
        }

        let config_path =
            std::path::Path::new(&self.config.writeable_path).join("anthropic_api_key");
        if let Ok(key) = std::fs::read_to_string(&config_path) {
            let key = key.trim().to_string();
            if !key.is_empty() {
                return Ok(key);
            }
        }

        Err(LbErrKind::Unexpected(format!(
            "No Anthropic API key found. Set ANTHROPIC_API_KEY environment variable or place your key at {}",
            config_path.display()
        ))
        .into())
    }
}
