use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_yaml::Value;

use crate::config::generator::ValidatorConfig;

/// Write the initial config.yaml with GENESIS_TIME, ACTIVE_EPOCH, and VALIDATOR_COUNT.
pub fn write_config_yaml(
    vc: &ValidatorConfig,
    genesis_offset: u32,
    output_dir: &Path,
) -> Result<()> {
    let genesis_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock before UNIX epoch")?
        .as_secs()
        + genesis_offset as u64;

    let total_validators: u32 = vc.validators.iter().map(|v| v.count).sum();

    let config_path = output_dir.join("config.yaml");
    let content = format!(
        "GENESIS_TIME: {genesis_time}\n\
         ACTIVE_EPOCH: {}\n\
         VALIDATOR_COUNT: {total_validators}\n",
        vc.config.active_epoch
    );
    fs::write(&config_path, &content)?;
    println!("Wrote {}", config_path.display());
    Ok(())
}

/// Run the eth-beacon-genesis Docker tool to generate genesis artifacts.
///
/// Produces: genesis.ssz, genesis.json, nodes.yaml, validators.yaml, updated config.yaml
pub fn run_genesis_tool(output_dir: &Path) -> Result<()> {
    let parent_dir = output_dir
        .parent()
        .context("output_dir has no parent directory")?;

    let genesis_rel = output_dir
        .file_name()
        .context("output_dir has no directory name")?
        .to_string_lossy();

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    let status = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--user",
            &format!("{uid}:{gid}"),
            "-v",
            &format!("{}:/data", parent_dir.display()),
            "ethpandaops/eth-beacon-genesis:pk910-leanchain",
            "leanchain",
            "--config",
            &format!("/data/{genesis_rel}/config.yaml"),
            "--mass-validators",
            &format!("/data/{genesis_rel}/validator-config.yaml"),
            "--state-output",
            &format!("/data/{genesis_rel}/genesis.ssz"),
            "--json-output",
            &format!("/data/{genesis_rel}/genesis.json"),
            "--nodes-output",
            &format!("/data/{genesis_rel}/nodes.yaml"),
            "--validators-output",
            &format!("/data/{genesis_rel}/validators.yaml"),
            "--config-output",
            &format!("/data/{genesis_rel}/config.yaml"),
        ])
        .status()
        .context("Failed to run eth-beacon-genesis Docker container")?;

    if !status.success() {
        bail!("eth-beacon-genesis exited with status {status}");
    }

    println!("Genesis artifacts generated in {}", output_dir.display());
    Ok(())
}

/// Post-process config.yaml: append GENESIS_VALIDATORS pubkeys from
/// the hash-sig-keys manifest, in validator-config.yaml order.
///
/// Each validator entry's pubkey is repeated `count` times.
pub fn append_genesis_validators(
    vc: &ValidatorConfig,
    output_dir: &Path,
) -> Result<()> {
    let manifest_path = output_dir.join("hash-sig-keys/validator-keys-manifest.yaml");
    let manifest_content = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let manifest: Value = serde_yaml::from_str(&manifest_content)?;

    // Detect pubkey field name (pubkey_hex, public_key_file, or publicKey)
    let pubkey_field = detect_pubkey_field(&manifest)?;

    let validators_arr = manifest
        .get("validators")
        .and_then(|v| v.as_sequence())
        .context("manifest missing 'validators' array")?;

    let mut pubkey_lines = Vec::new();
    for (i, entry) in vc.validators.iter().enumerate() {
        let manifest_entry = validators_arr
            .get(i)
            .with_context(|| format!("manifest has no entry at index {i}"))?;
        let pubkey_hex = manifest_entry
            .get(&pubkey_field)
            .and_then(|v| v.as_str())
            .with_context(|| format!("manifest entry {i} missing field '{pubkey_field}'"))?;
        // Strip 0x prefix if present
        let pubkey = pubkey_hex.strip_prefix("0x").unwrap_or(pubkey_hex);
        for _ in 0..entry.count {
            pubkey_lines.push(format!("    - \"{pubkey}\""));
        }
    }

    let config_path = output_dir.join("config.yaml");
    let mut config_content = fs::read_to_string(&config_path)?;
    config_content.push_str("GENESIS_VALIDATORS:\n");
    for line in &pubkey_lines {
        config_content.push_str(line);
        config_content.push('\n');
    }
    fs::write(&config_path, &config_content)?;
    println!(
        "Appended {} GENESIS_VALIDATORS entries to config.yaml",
        pubkey_lines.len()
    );
    Ok(())
}

/// Detect which field name holds the pubkey in the manifest.
fn detect_pubkey_field(manifest: &Value) -> Result<String> {
    let first = manifest
        .get("validators")
        .and_then(|v| v.as_sequence())
        .and_then(|s| s.first())
        .context("manifest has no validators")?;

    let candidates = ["pubkey_hex", "public_key_file", "publicKey"];
    for field in &candidates {
        if first.get(*field).is_some() {
            return Ok(field.to_string());
        }
    }

    bail!(
        "Could not detect pubkey field in manifest. Available keys: {:?}",
        first.as_mapping().map(|m| m.keys().collect::<Vec<_>>())
    )
}
