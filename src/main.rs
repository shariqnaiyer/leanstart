mod cli;
mod config;
mod genesis;
mod k8s;
mod keys;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lean-devnet", about = "Kubernetes devnet orchestrator for Lean validators")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate validator config, keys, genesis, and Helm values.
    Generate(cli::generate::GenerateArgs),
    /// Deploy the devnet to Kubernetes.
    Deploy(cli::deploy::DeployArgs),
    /// Show pod status in the devnet namespace.
    Status(cli::status::StatusArgs),
    /// Tear down the devnet deployment.
    Destroy(cli::destroy::DestroyArgs),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate(args) => cli::generate::run(args),
        Commands::Deploy(args) => cli::deploy::run(args),
        Commands::Status(args) => cli::status::run(args),
        Commands::Destroy(args) => cli::destroy::run(args),
    }
}
