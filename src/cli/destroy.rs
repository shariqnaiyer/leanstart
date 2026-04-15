use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;

#[derive(Debug, Args)]
pub struct DestroyArgs {
    /// Kubernetes namespace.
    #[arg(long, default_value = "lean-devnet")]
    pub namespace: String,

    /// Helm release name.
    #[arg(long, default_value = "lean-devnet")]
    pub release: String,

    /// Keep PVCs (only delete the Helm release).
    #[arg(long)]
    pub keep_pvcs: bool,
}

pub fn run(args: DestroyArgs) -> Result<()> {
    println!("==> Uninstalling Helm release '{}'...", args.release);

    let status = Command::new("helm")
        .args(["uninstall", &args.release, "-n", &args.namespace])
        .status()
        .context("Failed to run helm")?;

    if !status.success() {
        bail!("helm uninstall failed");
    }

    // Clean up pod secrets
    println!("==> Cleaning up pod secrets...");
    let _ = Command::new("kubectl")
        .args([
            "delete",
            "secrets",
            "-n",
            &args.namespace,
            "-l",
            "app.kubernetes.io/part-of=lean-devnet",
        ])
        .status();

    if !args.keep_pvcs {
        println!("==> Deleting PVCs...");
        let _ = Command::new("kubectl")
            .args([
                "delete",
                "pvc",
                "--all",
                "-n",
                &args.namespace,
            ])
            .status();
    } else {
        println!("  PVCs preserved (--keep-pvcs).");
    }

    println!("\n✓ Destroyed release '{}' in namespace '{}'.", args.release, args.namespace);
    Ok(())
}
