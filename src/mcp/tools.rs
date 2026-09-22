// Tool registry — maps tool names to their definitions and handlers.

use crate::mcp::handlers;
use crate::mcp::protocol::{CallToolParams, CallToolResult, ToolDefinition};
use serde_json::json;

/// Returns the list of tools this server exposes.
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "search_symbols".into(),
            description: "Search the symbol index by name (partial or exact, case-insensitive). \
                Returns matching symbols with their file path, line number, and kind."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repo_path": {
                        "type": "string",
                        "description": "Absolute path to the root of the repository to index."
                    },
                    "query": {
                        "type": "string",
                        "description": "Name fragment to search for."
                    }
                },
                "required": ["repo_path", "query"]
            }),
        },
        ToolDefinition {
            name: "get_file_structure".into(),
            description: "Returns all symbols defined in a TypeScript file, sorted by line number."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repo_path": {
                        "type": "string",
                        "description": "Absolute path to the root of the repository."
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to the TypeScript file to inspect."
                    }
                },
                "required": ["repo_path", "file_path"]
            }),
        },
        ToolDefinition {
            name: "find_references".into(),
            description: "Finds all occurrences of an identifier across the repository's \
                TypeScript files. Returns file paths and line numbers."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repo_path": {
                        "type": "string",
                        "description": "Absolute path to the root of the repository."
                    },
                    "name": {
                        "type": "string",
                        "description": "Exact identifier name to search for."
                    }
                },
                "required": ["repo_path", "name"]
            }),
        },
        ToolDefinition {
            name: "list_dependencies".into(),
            description: "Returns all import declarations found in a TypeScript file.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repo_path": {
                        "type": "string",
                        "description": "Absolute path to the root of the repository."
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Absolute or repo-relative path to the TypeScript file."
                    }
                },
                "required": ["repo_path", "file_path"]
            }),
        },
    ]
}

/// Dispatches a tool call by name and returns its result.
pub fn dispatch(params: &CallToolParams) -> CallToolResult {
    let args = &params.arguments;

    match params.name.as_str() {
        "search_symbols" => {
            let repo = str_arg(args, "repo_path");
            let query = str_arg(args, "query");
            match (repo, query) {
                (Some(r), Some(q)) => run(handlers::search_symbols(r, q)),
                _ => CallToolResult::error("missing required arguments: repo_path, query"),
            }
        }

        "get_file_structure" => {
            let repo = str_arg(args, "repo_path");
            let file = str_arg(args, "file_path");
            match (repo, file) {
                (Some(r), Some(f)) => run(handlers::get_file_structure(r, f)),
                _ => CallToolResult::error("missing required arguments: repo_path, file_path"),
            }
        }

        "find_references" => {
            let repo = str_arg(args, "repo_path");
            let name = str_arg(args, "name");
            match (repo, name) {
                (Some(r), Some(n)) => run(handlers::find_references(r, n)),
                _ => CallToolResult::error("missing required arguments: repo_path, name"),
            }
        }

        "list_dependencies" => {
            let repo = str_arg(args, "repo_path");
            let file = str_arg(args, "file_path");
            match (repo, file) {
                (Some(r), Some(f)) => run(handlers::list_dependencies(r, f)),
                _ => CallToolResult::error("missing required arguments: repo_path, file_path"),
            }
        }

        other => CallToolResult::error(format!("unknown tool: {other}")),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn str_arg<'a>(args: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn run(result: anyhow::Result<String>) -> CallToolResult {
    match result {
        Ok(text) => CallToolResult::text(text),
        Err(e) => CallToolResult::error(e.to_string()),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_list_contains_all_four_tools() {
        let tools = tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"search_symbols"));
        assert!(names.contains(&"get_file_structure"));
        assert!(names.contains(&"find_references"));
        assert!(names.contains(&"list_dependencies"));
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

    #[test]
    fn missing_args_returns_error() {
        let params = CallToolParams {
            name: "search_symbols".into(),
            arguments: json!({}),
        };
        let result = dispatch(&params);
        assert!(result.is_error);
        assert!(result.content[0].text.contains("missing required arguments"));
    }
}
