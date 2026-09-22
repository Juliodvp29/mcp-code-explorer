# mcp-code-explorer

An MCP server (stdio transport) that indexes a repository with tree-sitter and exposes code-exploration tools to an LLM agent.

## Status

Work in progress — see [roadmap](roadmap-mcp-rust.md) for the phased plan.

## Build

```bash
cargo build --release
```

## Run

```bash
cargo run
```

Logs are written to **stderr**. stdout is reserved for the MCP protocol.
