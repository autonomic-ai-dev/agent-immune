use clap::{Parser, Subcommand, ValueEnum};

use agent_body_core::cli::apply_progress_env;
use agent_body_core::ui::ProgressMode;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProgressArg {
    Auto,
    Plain,
    Quiet,
}

impl From<ProgressArg> for ProgressMode {
    fn from(value: ProgressArg) -> Self {
        match value {
            ProgressArg::Auto => ProgressMode::Auto,
            ProgressArg::Plain => ProgressMode::Plain,
            ProgressArg::Quiet => ProgressMode::Quiet,
        }
    }
}

#[derive(Parser)]
#[command(version)]
#[command(name = "agent-immune", about = "Dependency fuzzing & security sandbox")]
struct Cli {
    /// Progress output style: auto, plain, or quiet
    #[arg(long, value_enum, global = true, default_value = "auto")]
    progress: ProgressArg,

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
    /// Start daemon (HTTP + MCP server + JetStream consumer)
    Serve,
    /// Start the MCP stdio server only (no HTTP daemon)
    ServeMcp,
    /// Run a script in the network-isolated sandbox (CLI)
    Sandbox {
        /// Path to script or shell command file
        script: std::path::PathBuf,
        /// Allow network egress (default: blocked on Linux via unshare -n)
        #[arg(long)]
        allow_network: bool,
    },
    /// Verify a script has no runaway memory growth (dataset gate)
    VerifyMemory {
        /// Script path or command file
        script: std::path::PathBuf,
        /// Max RSS growth in KiB before failing (default 512 MiB)
        #[arg(long, default_value_t = agent_immune::leak_check::DEFAULT_THRESHOLD_KB)]
        threshold_kb: u64,
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
    apply_progress_env(cli.progress.into());
    match cli.command {
        Commands::Scan { path } => agent_immune::run_scan(&path).await?,
        Commands::Serve => {
            let config = agent_immune::config::Config::load()?;
            agent_immune::serve::start(config).await?;
        }
        Commands::ServeMcp => {
            let config = agent_immune::config::Config::load()?;
            agent_immune::mcp_server::ImmuneMcp::run(config).await?;
        }
        Commands::Sandbox {
            script,
            allow_network,
        } => {
            let config = agent_immune::config::Config::load()?;
            let mut options = agent_immune::sandbox::SandboxOptions::from(&config.sandbox);
            options.network_blackhole = !allow_network;
            let result = agent_immune::sandbox::run_script(&script, &options).await;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::VerifyMemory {
            script,
            threshold_kb,
        } => {
            let report =
                agent_immune::leak_check::gate_trajectory_script(&script, threshold_kb).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.passed {
                std::process::exit(1);
            }
        }
        Commands::Status => {
            let config = agent_immune::config::Config::load()?;
            println!("agent-immune status");
            println!(
                "  config: {}",
                agent_immune::config::Config::config_path().display()
            );
            println!("  port: {}", config.server.port);
            println!("  nats_url: {}", config.nats.url);
            println!("  jetstream_consumer: {}", config.nats.jetstream_consumer);
            println!("  network_blackhole: {}", config.sandbox.network_blackhole);
            println!("  sandbox_backend: {}", config.sandbox.backend);
            println!("  seccomp: {}", config.sandbox.seccomp);
            println!(
                "  firecracker_ready: {}",
                agent_immune::firecracker::is_available()
            );
        }
    }
    Ok(())
}
