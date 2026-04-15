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
    println!("==> Pod status in namespace '{}':\n", args.namespace);

    let output = Command::new("kubectl")
        .args([
            "get",
            "pods",
            "-n",
            &args.namespace,
            "-o",
            "wide",
            "--sort-by=.metadata.name",
        ])
        .output()
        .context("Failed to run kubectl")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("kubectl error: {stderr}");
    } else {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    // Summary by client type
    println!("\n==> Summary:");
    let output = Command::new("kubectl")
        .args([
            "get",
            "pods",
            "-n",
            &args.namespace,
            "-o",
            "jsonpath={range .items[*]}{.metadata.labels.app}{\"\\t\"}{.status.phase}{\"\\n\"}{end}",
        ])
        .output()
        .context("Failed to run kubectl")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut running = 0u32;
        let mut pending = 0u32;
        let mut failed = 0u32;
        for line in stdout.lines() {
            if line.contains("Running") {
                running += 1;
            } else if line.contains("Pending") {
                pending += 1;
            } else if !line.is_empty() {
                failed += 1;
            }
        }
        let total = running + pending + failed;
        println!("  Total: {total}  Running: {running}  Pending: {pending}  Other: {failed}");
    }

    Ok(())
}
