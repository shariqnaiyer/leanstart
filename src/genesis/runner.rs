use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_yaml::Value;

use crate::config::generator::ValidatorConfig;

/// Write the initial config.yaml with GENESIS_TIME, ATTESTATION_COMMITTEE_COUNT,
/// ACTIVE_EPOCH, and VALIDATOR_COUNT.
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
    let committee_count = vc.config.attestation_committee_count.unwrap_or(1);

    let config_path = output_dir.join("config.yaml");
    let content = format!(
        "# Genesis Settings\n\
         GENESIS_TIME: {genesis_time}\n\
         \n\
         # Chain Settings\n\
         ATTESTATION_COMMITTEE_COUNT: {committee_count}\n\
         \n\
         # Key Settings\n\
         ACTIVE_EPOCH: {}\n\
         \n\
         # Validator Settings\n\
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

/// Layout of the hash-sig-cli validator-keys-manifest.yaml.
enum ManifestLayout {
    /// devnet4+: separate attester and proposer pubkeys per validator.
    DualKey,
    /// Legacy: single pubkey per validator under the named field.
    SinglePubkey(String),
}

/// Post-process config.yaml: append GENESIS_VALIDATORS pubkeys from
/// the hash-sig-keys manifest, in validator-config.yaml order.
///
/// Dual-key manifests (devnet4+) emit `attestation_pubkey` / `proposal_pubkey`
/// pairs; legacy manifests emit a flat list. Each validator entry's pubkey(s)
/// are repeated `count` times.
pub fn append_genesis_validators(
    vc: &ValidatorConfig,
    output_dir: &Path,
) -> Result<()> {
    let manifest_path = output_dir.join("hash-sig-keys/validator-keys-manifest.yaml");
    let manifest_content = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let manifest: Value = serde_yaml::from_str(&manifest_content)?;

    let validators_arr = manifest
        .get("validators")
        .and_then(|v| v.as_sequence())
        .context("manifest missing 'validators' array")?;

    let layout = detect_manifest_layout(validators_arr)?;

    let mut block = String::new();
    block.push('\n');
    block.push_str("# List of Genesis Validators' Public Keys (attestation + proposal)\n");
    block.push_str("GENESIS_VALIDATORS:\n");

    let mut count = 0usize;
    let mut global_idx = 0usize;
    for entry in &vc.validators {
        for _ in 0..entry.count {
            let manifest_entry = validators_arr
                .get(global_idx)
                .with_context(|| format!("manifest has no entry at index {global_idx}"))?;

            match &layout {
                ManifestLayout::DualKey => {
                    let attester = read_hex_field(manifest_entry, "attester_key_pubkey_hex", global_idx)?;
                    let proposer = read_hex_field(manifest_entry, "proposer_key_pubkey_hex", global_idx)?;
                    block.push_str(&format!("  - attestation_pubkey: \"{attester}\"\n"));
                    block.push_str(&format!("    proposal_pubkey: \"{proposer}\"\n"));
                }
                ManifestLayout::SinglePubkey(field) => {
                    let pk = read_hex_field(manifest_entry, field, global_idx)?;
                    block.push_str(&format!("    - \"{pk}\"\n"));
                }
            }
            count += 1;
            global_idx += 1;
        }
    }

    let config_path = output_dir.join("config.yaml");
    let mut config_content = fs::read_to_string(&config_path)?;
    config_content.push_str(&block);
    fs::write(&config_path, &config_content)?;
    println!("Appended {count} GENESIS_VALIDATORS entries to config.yaml");
    Ok(())
}

/// Build annotated_validators.yaml: per-node list of validator entries with
/// pubkey + privkey filename. In dual-key mode, each validator index produces
/// two entries (attester + proposer).
pub fn generate_annotated_validators(
    output_dir: &Path,
) -> Result<()> {
    let validators_path = output_dir.join("validators.yaml");
    let validators_content = fs::read_to_string(&validators_path)
        .with_context(|| format!("Failed to read {}", validators_path.display()))?;
    let validators_root: Value = serde_yaml::from_str(&validators_content)?;

    // Assignments may be wrapped under `validators:` or be the root mapping itself.
    let assignments = match validators_root.get("validators") {
        Some(v) if v.is_mapping() => v,
        _ => &validators_root,
    };
    let assignments_map = assignments
        .as_mapping()
        .context("validators.yaml top-level is not a mapping")?;

    let manifest_path = output_dir.join("hash-sig-keys/validator-keys-manifest.yaml");
    let manifest_content = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let manifest: Value = serde_yaml::from_str(&manifest_content)?;
    let manifest_validators = manifest
        .get("validators")
        .and_then(|v| v.as_sequence())
        .context("manifest missing 'validators' array")?;
    let layout = detect_manifest_layout(manifest_validators)?;

    let mut output = String::new();
    let total = assignments_map.len();
    for (i, (node_key, indices_val)) in assignments_map.iter().enumerate() {
        let node = node_key
            .as_str()
            .context("validators.yaml node name is not a string")?;
        output.push_str(&format!("{node}:\n"));

        let indices: Vec<usize> = indices_val
            .as_sequence()
            .map(|s| {
                s.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        if indices.is_empty() {
            output.push_str("  []\n");
        } else {
            for idx in indices {
                let entry = manifest_validators.get(idx).with_context(|| {
                    format!("manifest has no entry at index {idx} (referenced by node {node})")
                })?;
                match &layout {
                    ManifestLayout::DualKey => {
                        let attester = read_hex_field(entry, "attester_key_pubkey_hex", idx)?;
                        let proposer = read_hex_field(entry, "proposer_key_pubkey_hex", idx)?;
                        output.push_str(&format!("  - index: {idx}\n"));
                        output.push_str(&format!("    pubkey_hex: {attester}\n"));
                        output.push_str(&format!(
                            "    privkey_file: validator_{idx}_attester_key_sk.ssz\n"
                        ));
                        output.push_str(&format!("  - index: {idx}\n"));
                        output.push_str(&format!("    pubkey_hex: {proposer}\n"));
                        output.push_str(&format!(
                            "    privkey_file: validator_{idx}_proposer_key_sk.ssz\n"
                        ));
                    }
                    ManifestLayout::SinglePubkey(field) => {
                        let pk = read_hex_field(entry, field, idx)?;
                        output.push_str(&format!("  - index: {idx}\n"));
                        output.push_str(&format!("    pubkey_hex: {pk}\n"));
                        output.push_str(&format!(
                            "    privkey_file: validator_{idx}_sk.ssz\n"
                        ));
                    }
                }
            }
        }

        if i + 1 < total {
            output.push('\n');
        }
    }

    let out_path = output_dir.join("annotated_validators.yaml");
    fs::write(&out_path, &output)?;
    println!("Wrote {}", out_path.display());
    Ok(())
}

fn detect_manifest_layout(validators: &[Value]) -> Result<ManifestLayout> {
    let first = validators
        .first()
        .context("manifest has no validator entries")?;

    if first.get("attester_key_pubkey_hex").is_some()
        && first.get("proposer_key_pubkey_hex").is_some()
    {
        return Ok(ManifestLayout::DualKey);
    }

    for field in ["pubkey_hex", "public_key_file", "publicKey"] {
        if first.get(field).is_some() {
            return Ok(ManifestLayout::SinglePubkey(field.to_string()));
        }
    }

    bail!(
        "Could not detect pubkey layout in manifest. Available keys: {:?}",
        first.as_mapping().map(|m| m.keys().collect::<Vec<_>>())
    )
}

fn read_hex_field(entry: &Value, field: &str, idx: usize) -> Result<String> {
    let raw = entry
        .get(field)
        .and_then(|v| v.as_str())
        .with_context(|| format!("manifest entry {idx} missing field '{field}'"))?;
    let stripped = raw.strip_prefix("0x").unwrap_or(raw);
    Ok(stripped.to_string())
}
