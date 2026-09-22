mod indexer;
mod mcp;

use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

fn main() -> anyhow::Result<()> {
    // Logging goes to stderr — stdout is reserved for the MCP protocol.
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_writer(std::io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("mcp-code-explorer server starting");

    mcp::server::run()?;

    Ok(())
}
