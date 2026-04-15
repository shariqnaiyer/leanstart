use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;

#[derive(Debug, Args)]
pub struct DeployArgs {
    /// Output directory from the generate step.
    #[arg(long, default_value = "./output")]
    pub output_dir: PathBuf,

    /// Kubernetes namespace.
    #[arg(long, default_value = "lean-devnet")]
    pub namespace: String,

    /// Helm release name.
    #[arg(long, default_value = "lean-devnet")]
    pub release: String,

    /// Perform a dry run (no actual deployment).
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: DeployArgs) -> Result<()> {
    let values_path = args.output_dir.join("helm-values.yaml");
    if !values_path.exists() {
        bail!(
            "helm-values.yaml not found at {}. Run 'lean-devnet generate' first.",
            values_path.display()
        );
    }

    let chart_path = find_chart_path()?;

    // Apply per-pod secrets first
    let secrets_dir = args.output_dir.join("secrets");
    if secrets_dir.exists() {
        println!("==> Applying pod secrets...");
        let mut kubectl_args = vec![
            "apply".to_string(),
            "-f".to_string(),
            secrets_dir.display().to_string(),
            "-n".to_string(),
            args.namespace.clone(),
        ];
        if args.dry_run {
            kubectl_args.push("--dry-run=client".into());
        }

        // Ensure namespace exists
        if !args.dry_run {
            let _ = Command::new("kubectl")
                .args(["create", "namespace", &args.namespace])
                .status();
        }

        let status = Command::new("kubectl")
            .args(&kubectl_args)
            .status()
            .context("Failed to run kubectl")?;
        if !status.success() {
            bail!("kubectl apply secrets failed");
        }
    }

    // Run helm install/upgrade
    println!("==> Installing Helm chart...");
    let mut helm_args = vec![
        "upgrade".to_string(),
        "--install".to_string(),
        args.release.clone(),
        chart_path.display().to_string(),
        "-f".to_string(),
        values_path.display().to_string(),
        "-n".to_string(),
        args.namespace.clone(),
        "--create-namespace".to_string(),
    ];
    if args.dry_run {
        helm_args.push("--dry-run".into());
    }

    let status = Command::new("helm")
        .args(&helm_args)
        .status()
        .context("Failed to run helm")?;

    if !status.success() {
        bail!("helm install failed");
    }

    if args.dry_run {
        println!("\n✓ Dry run complete.");
    } else {
        println!("\n✓ Deployed to namespace '{}'.", args.namespace);
        println!("  Run 'lean-devnet status' to check pod health.");
    }
    Ok(())
}

fn find_chart_path() -> Result<PathBuf> {
    // Look for chart relative to the binary, then in common locations
    let candidates = [
        PathBuf::from("helm/lean-devnet"),
        PathBuf::from("../helm/lean-devnet"),
    ];
    for path in &candidates {
        if path.join("Chart.yaml").exists() {
            return Ok(path.clone());
        }
    }
    bail!(
        "Could not find Helm chart. Looked in: {:?}",
        candidates
    )
}
