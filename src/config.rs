// Configuration — parsed from CLI arguments and environment variables.
// Command-line flags take precedence over environment variables.

use clap::Parser;

/// MCP server that indexes a TypeScript repository and exposes code-exploration tools.
#[derive(Debug, Parser)]
#[command(name = "mcp-code-explorer", version, about)]
pub struct Config {
    /// Default repository root to index when a tool call omits `repo_path`.
    /// Can also be set via the MCP_REPO_PATH environment variable.
    #[arg(long, env = "MCP_REPO_PATH", value_name = "PATH")]
    pub repo_path: Option<String>,

    /// Log level filter (e.g. info, debug, warn).
    /// Can also be set via RUST_LOG.
    #[arg(long, env = "RUST_LOG", default_value = "info", value_name = "LEVEL")]
    pub log_level: String,
}

impl Config {
    /// Parses configuration from the process arguments and environment.
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn defaults_when_no_args() {
        let cfg = Config::parse_from(["mcp-code-explorer"]);
        assert!(cfg.repo_path.is_none());
        assert_eq!(cfg.log_level, "info");
    }

    #[test]
    fn parses_repo_path_flag() {
        let cfg = Config::parse_from(["mcp-code-explorer", "--repo-path", "/some/repo"]);
        assert_eq!(cfg.repo_path.as_deref(), Some("/some/repo"));
    }

    #[test]
    fn parses_log_level_flag() {
        let cfg = Config::parse_from(["mcp-code-explorer", "--log-level", "debug"]);
        assert_eq!(cfg.log_level, "debug");
    }
}
