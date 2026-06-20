use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agent-immune", about = "Dependency fuzzing & security sandbox")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a manifest file for vulnerable dependencies
    Scan {
        /// Path to Cargo.toml or package.json
        path: std::path::PathBuf,
    },
    /// Start daemon (placeholder)
    Serve,
    /// Show configuration and status
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Scan { path } => agent_immune::run_scan(&path).await?,
        Commands::Serve => {
            let _cfg = agent_immune::config::Config::load()?;
            println!("agent-immune serve (not yet implemented)");
            println!("  config: {}", agent_immune::config::Config::config_path().display());
        }
        Commands::Status => {
            let _cfg = agent_immune::config::Config::load()?;
            println!("agent-immune status");
            println!("  config: {}", agent_immune::config::Config::config_path().display());
        }
    }
    Ok(())
}
