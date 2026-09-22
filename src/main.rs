mod config;
mod indexer;
mod mcp;

use tracing::info;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    let cfg = config::Config::parse();

    // Logging goes to stderr — stdout is reserved for the MCP protocol.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_new(&cfg.log_level)
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("mcp-code-explorer server starting");

    if let Some(ref path) = cfg.repo_path {
        info!("default repo_path: {path}");
    }

    mcp::server::run(cfg.repo_path.as_deref())?;

    Ok(())
}
