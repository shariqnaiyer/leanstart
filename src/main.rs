mod cli;
mod config;
mod genesis;
mod k8s;
mod keys;

use std::env;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "leanstart", about = "Devnet orchestrator for Lean validators")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a devnet: leanstart ream zeam:2
    Run(cli::run::RunArgs),
    /// Generate validator config, keys, genesis, and Helm values (advanced).
    Generate(cli::generate::GenerateArgs),
    /// Deploy a generated devnet to Kubernetes.
    Deploy(cli::deploy::DeployArgs),
    /// Show pod status in the devnet namespace.
    Status(cli::status::StatusArgs),
    /// Tear down the devnet deployment.
    Destroy(cli::destroy::DestroyArgs),
}

const SUBCOMMANDS: &[&str] = &["run", "generate", "deploy", "status", "destroy", "help"];

fn main() -> anyhow::Result<()> {
    // If the first arg isn't a known subcommand, treat it as `run <args...>`
    // This lets users type `leanstart ream zeam:2` instead of `leanstart run ream zeam:2`
    let args: Vec<String> = env::args().collect();
    let cli = if args.len() > 1
        && !SUBCOMMANDS.contains(&args[1].as_str())
        && !args[1].starts_with('-')
    {
        let mut patched = vec![args[0].clone(), "run".to_string()];
        patched.extend_from_slice(&args[1..]);
        Cli::parse_from(patched)
    } else {
        Cli::parse()
    };

    match cli.command {
        Commands::Run(args) => cli::run::run(args),
        Commands::Generate(args) => cli::generate::run(args),
        Commands::Deploy(args) => cli::deploy::run(args),
        Commands::Status(args) => cli::status::run(args),
        Commands::Destroy(args) => cli::destroy::run(args),
    }
}
