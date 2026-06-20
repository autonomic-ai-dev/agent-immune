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
    /// Run a script in the network-isolated sandbox (CLI)
    Sandbox {
        /// Path to script or shell command file
        script: std::path::PathBuf,
        /// Allow network egress (default: blocked on Linux via unshare -n)
        #[arg(long)]
        allow_network: bool,
    },
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
            let config = agent_immune::config::Config::load()?;
            agent_immune::serve::start(config).await?;
        }
        Commands::Sandbox { script, allow_network } => {
            let options = agent_immune::sandbox::SandboxOptions {
                network_blackhole: !allow_network,
            };
            let result = agent_immune::sandbox::run_script(&script, &options).await;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Status => {
            let config = agent_immune::config::Config::load()?;
            println!("agent-immune status");
            println!("  config: {}", agent_immune::config::Config::config_path().display());
            println!("  port: {}", config.server.port);
            println!("  nats_url: {}", config.nats.url);
            println!("  jetstream_consumer: {}", config.nats.jetstream_consumer);
            println!("  network_blackhole: {}", config.sandbox.network_blackhole);
        }
    }
    Ok(())
}
