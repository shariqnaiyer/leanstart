use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Kubernetes namespace.
    #[arg(long, default_value = "lean-devnet")]
    pub namespace: String,
}

pub fn run(args: StatusArgs) -> Result<()> {
    // Check if a cluster is reachable
    let check = Command::new("kubectl")
        .args(["cluster-info"])
        .output()
        .context("kubectl not found")?;

    if !check.status.success() {
        println!("No cluster running. Start a devnet with: leanstart ream zeam:2");
        return Ok(());
    }

    let output = Command::new("kubectl")
        .args([
            "get", "pods", "-n", &args.namespace,
            "-o", "wide",
            "--sort-by=.metadata.name",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            println!("No devnet running in namespace '{}'.", args.namespace);
        } else {
            eprintln!("{stderr}");
        }
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() || stdout.contains("No resources found") {
        println!("No pods running in namespace '{}'.", args.namespace);
        return Ok(());
    }

    print!("{stdout}");

    // Summary
    let output = Command::new("kubectl")
        .args([
            "get", "pods", "-n", &args.namespace,
            "-o", "jsonpath={range .items[*]}{.metadata.labels.app}{\"\\t\"}{.status.phase}{\"\\n\"}{end}",
        ])
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut running = 0u32;
        let mut pending = 0u32;
        let mut other = 0u32;
        for line in stdout.lines() {
            if line.contains("Running") {
                running += 1;
            } else if line.contains("Pending") {
                pending += 1;
            } else if !line.is_empty() {
                other += 1;
            }
        }
        let total = running + pending + other;
        println!("\n{running}/{total} running");
    }

    Ok(())
}
