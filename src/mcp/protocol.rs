// MCP protocol-level types: initialization, tool descriptors, and tool call results.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Initialize ────────────────────────────────────────────────────────────────

/// Parameters sent by the client in an `initialize` request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(default)]
    pub capabilities: Value,
    pub client_info: Option<ClientInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ClientInfo {
    pub name: String,
    pub version: Option<String>,
}

/// Result returned to the client after a successful `initialize`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
pub struct ToolsCapability {
    // Server supports listing and calling tools.
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

// ── Tools ─────────────────────────────────────────────────────────────────────

/// Descriptor for a single tool exposed by this server.
#[derive(Debug, Serialize, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Result of `tools/list`.
#[derive(Debug, Serialize)]
pub struct ListToolsResult {
    pub tools: Vec<ToolDefinition>,
}

/// Parameters of a `tools/call` request.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// A single content item returned by a tool call.
#[derive(Debug, Serialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

/// Result of `tools/call`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<ToolContent>,
    pub is_error: bool,
}

impl CallToolResult {
    pub fn text(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent {
                kind: "text".into(),
                text: message.into(),
            }],
            is_error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent {
                kind: "text".into(),
                text: message.into(),
            }],
            is_error: true,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_initialize_params() {
        let raw = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0" }
        });
        let params: InitializeParams = serde_json::from_value(raw).unwrap();
        assert_eq!(params.protocol_version, "2024-11-05");
        assert_eq!(params.client_info.unwrap().name, "test-client");
    }

    #[test]
    fn serializes_initialize_result() {
        let result = InitializeResult {
            protocol_version: "2024-11-05".into(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability { list_changed: false },
            },
            server_info: ServerInfo {
                name: "mcp-code-explorer".into(),
                version: "0.1.0".into(),
            },
        };
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["protocolVersion"], "2024-11-05");
        assert_eq!(v["serverInfo"]["name"], "mcp-code-explorer");
    }

    #[test]
    fn deserializes_call_tool_params() {
        let raw = json!({ "name": "ping", "arguments": {} });
        let params: CallToolParams = serde_json::from_value(raw).unwrap();
        assert_eq!(params.name, "ping");
    }

    #[test]
    fn call_tool_result_text_marks_is_error_false() {
        let r = CallToolResult::text("pong");
        assert!(!r.is_error);
        assert_eq!(r.content[0].text, "pong");
    }

    #[test]
    fn call_tool_result_error_marks_is_error_true() {
        let r = CallToolResult::error("something went wrong");
        assert!(r.is_error);
    }
}
