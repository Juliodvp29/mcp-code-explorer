// Tool registry — maps tool names to their definitions and handlers.

use crate::mcp::protocol::{CallToolParams, CallToolResult, ToolDefinition};
use serde_json::json;

/// Returns the list of tools this server exposes.
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: "ping".into(),
        description: "Returns a pong response. Useful for verifying the server is reachable."
            .into(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }]
}

/// Dispatches a tool call by name and returns its result.
pub fn dispatch(params: &CallToolParams) -> CallToolResult {
    match params.name.as_str() {
        "ping" => CallToolResult::text("pong"),
        other => CallToolResult::error(format!("unknown tool: {other}")),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_list_contains_ping() {
        let tools = tool_definitions();
        assert!(tools.iter().any(|t| t.name == "ping"));
    }

    #[test]
    fn ping_returns_pong() {
        let params = CallToolParams {
            name: "ping".into(),
            arguments: json!({}),
        };
        let result = dispatch(&params);
        assert!(!result.is_error);
        assert_eq!(result.content[0].text, "pong");
    }

    #[test]
    fn unknown_tool_returns_error() {
        let params = CallToolParams {
            name: "does_not_exist".into(),
            arguments: json!({}),
        };
        let result = dispatch(&params);
        assert!(result.is_error);
        assert!(result.content[0].text.contains("unknown tool"));
    }
}
