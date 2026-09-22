# mcp-code-explorer

An MCP server (stdio transport) that indexes a TypeScript repository with tree-sitter and exposes code-exploration tools to any MCP-compatible LLM client.

## Features

- `search_symbols` — find functions, classes, interfaces, enums, and type aliases by name (partial, case-insensitive)
- `get_file_structure` — list all symbols defined in a single file, sorted by line
- `find_references` — locate every occurrence of an identifier across the repo
- `list_dependencies` — extract import declarations from a TypeScript file

Results are cached in `.mcp-index.bin` at the repo root. Only modified files are re-parsed on subsequent runs.

## Build

Requires Rust 1.75+ and a C compiler (for the tree-sitter grammars).

```bash
cargo build --release
```

The binary is written to `target/release/mcp-code-explorer`.

## Configuration

| Flag | Env var | Default | Description |
|------|---------|---------|-------------|
| `--repo-path PATH` | `MCP_REPO_PATH` | *(none)* | Default repo to index. When set, tool calls can omit `repo_path`. |
| `--log-level LEVEL` | `RUST_LOG` | `info` | Tracing filter (e.g. `debug`, `warn`). Logs go to **stderr** only. |

## Register in Claude Desktop

Add the server to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "code-explorer": {
      "command": "/absolute/path/to/mcp-code-explorer",
      "args": ["--repo-path", "/absolute/path/to/your/ts-project"],
      "env": {
        "RUST_LOG": "warn"
      }
    }
  }
}
```

## Register in Claude Code (VS Code extension)

Add to your workspace `.vscode/mcp.json` or the global MCP config:

```json
{
  "servers": {
    "code-explorer": {
      "type": "stdio",
      "command": "/absolute/path/to/mcp-code-explorer",
      "args": ["--repo-path", "${workspaceFolder}"]
    }
  }
}
```

## Register in Codex CLI (OpenAI)

Add to `~/.codex/config.toml`:

```toml
[[mcp_servers]]
name   = "code-explorer"
type   = "stdio"
cmd    = "/absolute/path/to/mcp-code-explorer"
args   = ["--repo-path", "/absolute/path/to/your/ts-project"]
```

Or pass it inline when starting a session:

```bash
codex --mcp-server '/absolute/path/to/mcp-code-explorer --repo-path /path/to/ts-project'
```

## Register in Cursor

Add to `.cursor/mcp.json` in your project root, or to the global `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "code-explorer": {
      "command": "/absolute/path/to/mcp-code-explorer",
      "args": ["--repo-path", "/absolute/path/to/your/ts-project"]
    }
  }
}
```

## Register in Windsurf

Add to `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "code-explorer": {
      "command": "/absolute/path/to/mcp-code-explorer",
      "args": ["--repo-path", "/absolute/path/to/your/ts-project"]
    }
  }
}
```

## Any other MCP-compatible client

The server speaks JSON-RPC 2.0 over stdin/stdout with no client-specific
extensions. Any client that supports stdio MCP servers can connect to it with:

```
command: /absolute/path/to/mcp-code-explorer
args:    [--repo-path, /absolute/path/to/your/ts-project]
```

Check your client's documentation for the exact config file location and format.
The `--repo-path` flag is optional: if your client passes `repo_path` in every
tool call argument, you can omit it.

## Tool reference

All tools accept `repo_path` to override the server's default at call time.

### `search_symbols`

```json
{
  "name": "search_symbols",
  "arguments": {
    "repo_path": "/path/to/repo",
    "query": "UserService"
  }
}
```

Returns one match per line: `SymbolName [kind] /path/to/file.ts:LINE`.

### `get_file_structure`

```json
{
  "name": "get_file_structure",
  "arguments": {
    "repo_path": "/path/to/repo",
    "file_path": "/path/to/repo/src/user.service.ts"
  }
}
```

Returns a sorted outline of every symbol in the file.

### `find_references`

```json
{
  "name": "find_references",
  "arguments": {
    "repo_path": "/path/to/repo",
    "name": "UserService"
  }
}
```

Returns `file.ts:LINE` for every identifier occurrence in the repo.

### `list_dependencies`

```json
{
  "name": "list_dependencies",
  "arguments": {
    "repo_path": "/path/to/repo",
    "file_path": "/path/to/repo/src/app.module.ts"
  }
}
```

Returns the raw import statements found in the file.

## Development

```bash
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- --repo-path /path/to/ts-project
```

Logs are written to **stderr**. stdout carries only the MCP JSON-RPC protocol.
