// Stdio server — reads newline-delimited JSON-RPC messages from stdin and writes
// responses to stdout. Logging goes to stderr to keep the protocol channel clean.

use std::io::{self, BufRead, Write};

use anyhow::Result;
use serde_json::{json, Value};
use tracing::{debug, error, warn};

use crate::mcp::protocol::{
    CallToolParams, InitializeParams, InitializeResult, ListToolsResult, ServerCapabilities,
    ServerInfo, ToolsCapability,
};
use crate::mcp::tools;
use crate::mcp::types::{
    ErrorResponse, Request, Response, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND,
    PARSE_ERROR,
};

/// Runs the JSON-RPC stdio loop until stdin is closed.
pub fn run() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => l,
            Err(e) => {
                error!("failed to read stdin: {e}");
                break;
            }
        };

        debug!("recv: {line}");

        let response_bytes = match serde_json::from_str::<Request>(&line) {
            Err(e) => {
                warn!("parse error: {e}");
                let resp = ErrorResponse::new(json!(null), PARSE_ERROR, "parse error");
                serde_json::to_vec(&resp)?
            }
            Ok(req) => {
                // Notifications (no id) are processed but not answered.
                if req.id.is_none() {
                    handle_notification(&req.method);
                    continue;
                }
                let id = req.id.clone().unwrap_or(Value::Null);
                handle_request(id, &req.method, req.params)?
            }
        };

        out.write_all(&response_bytes)?;
        out.write_all(b"\n")?;
        out.flush()?;
    }

    Ok(())
}

fn handle_notification(method: &str) {
    debug!("notification: {method}");
}

fn handle_request(id: Value, method: &str, params: Value) -> Result<Vec<u8>> {
    let bytes = match method {
        "initialize" => {
            match serde_json::from_value::<InitializeParams>(params) {
                Ok(_p) => {
                    let result = InitializeResult {
                        protocol_version: "2024-11-05".into(),
                        capabilities: ServerCapabilities {
                            tools: ToolsCapability { list_changed: false },
                        },
                        server_info: ServerInfo {
                            name: env!("CARGO_PKG_NAME").into(),
                            version: env!("CARGO_PKG_VERSION").into(),
                        },
                    };
                    serde_json::to_vec(&Response::ok(id, serde_json::to_value(result)?))?
                }
                Err(e) => {
                    serde_json::to_vec(&ErrorResponse::new(id, INVALID_PARAMS, &e.to_string()))?
                }
            }
        }

        "tools/list" => {
            let result = ListToolsResult {
                tools: tools::tool_definitions(),
            };
            serde_json::to_vec(&Response::ok(id, serde_json::to_value(result)?))?
        }

        "tools/call" => {
            match serde_json::from_value::<CallToolParams>(params) {
                Ok(p) => {
                    let result = tools::dispatch(&p);
                    serde_json::to_vec(&Response::ok(id, serde_json::to_value(result)?))?
                }
                Err(e) => {
                    serde_json::to_vec(&ErrorResponse::new(id, INVALID_PARAMS, &e.to_string()))?
                }
            }
        }

        other => {
            warn!("unknown method: {other}");
            serde_json::to_vec(&ErrorResponse::new(
                id,
                METHOD_NOT_FOUND,
                &format!("method not found: {other}"),
            ))?
        }
    };

    // Guard: the protocol channel must never receive non-JSON noise.
    if let Err(e) = serde_json::from_slice::<Value>(&bytes) {
        error!("BUG: serialized an invalid JSON response: {e}");
        let fallback = ErrorResponse::new(Value::Null, INTERNAL_ERROR, "internal error");
        return Ok(serde_json::to_vec(&fallback)?);
    }

    Ok(bytes)
}
