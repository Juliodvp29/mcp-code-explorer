// JSON-RPC 2.0 request and response types used by the MCP protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Requests ─────────────────────────────────────────────────────────────────

/// A JSON-RPC 2.0 request or notification received from the client.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

// ── Responses ────────────────────────────────────────────────────────────────

/// A successful JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Value,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result,
        }
    }
}

/// A JSON-RPC 2.0 error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub error: RpcError,
}

impl ErrorResponse {
    pub fn new(id: Value, code: i64, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            error: RpcError {
                code,
                message: message.to_owned(),
                data: None,
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ── Standard error codes ──────────────────────────────────────────────────────

pub const PARSE_ERROR: i64 = -32700;
#[allow(dead_code)]
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_request_with_id() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(json!(1)));
    }

    #[test]
    fn deserializes_notification_without_id() {
        let raw = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert_eq!(req.method, "notifications/initialized");
        assert!(req.id.is_none());
    }

    #[test]
    fn serializes_response() {
        let resp = Response::ok(json!(1), json!({"status": "ok"}));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains(r#""jsonrpc":"2.0""#));
        assert!(s.contains(r#""status":"ok""#));
    }

    #[test]
    fn serializes_error_response() {
        let resp = ErrorResponse::new(json!(1), METHOD_NOT_FOUND, "method not found");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains(r#""code":-32601"#));
        assert!(s.contains("method not found"));
    }

    #[test]
    fn error_data_field_omitted_when_none() {
        let resp = ErrorResponse::new(json!(null), INTERNAL_ERROR, "oops");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(!s.contains("\"data\""));
    }
}
