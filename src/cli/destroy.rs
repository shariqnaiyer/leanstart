use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;

#[derive(Debug, Args)]
pub struct DestroyArgs {
    /// Kubernetes namespace.
    #[arg(long, default_value = "lean-devnet")]
    pub namespace: String,

    /// Helm release name.
    #[arg(long, default_value = "lean-devnet")]
    pub release: String,

    /// Kind cluster name.
    #[arg(long, default_value = "lean-devnet")]
    pub cluster: String,
}

pub fn run(args: DestroyArgs) -> Result<()> {
    // Step 1: Scale down all StatefulSets
    println!("==> Scaling down all pods...");
    let _ = Command::new("kubectl")
        .args([
            "scale", "statefulset", "--all",
            "-n", &args.namespace,
            "--replicas=0",
        ])
        .status();

    // Step 2: Uninstall Helm release
    println!("==> Uninstalling Helm release '{}'...", args.release);
    let _ = Command::new("helm")
        .args(["uninstall", &args.release, "-n", &args.namespace])
        .status()
        .context("Failed to run helm")?;

    // Step 3: Delete the kind cluster
    println!("==> Deleting kind cluster '{}'...", args.cluster);
    let status = Command::new("kind")
        .args(["delete", "cluster", "--name", &args.cluster])
        .status()
        .context("Failed to run kind")?;

    if status.success() {
        println!("\nDestroyed cluster '{}'.", args.cluster);
    } else {
        println!("\nNo kind cluster '{}' found (may already be deleted).", args.cluster);
    }

    Ok(())
}
