use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use crate::config::generator::{generate_validator_config, write_validator_config};
use crate::config::spec::{ClientAllocation, DevnetSpec};
use crate::genesis::runner::{append_genesis_validators, run_genesis_tool, write_config_yaml};
use crate::k8s::values::{generate_helm_values, generate_pod_secrets, write_helm_values};
use crate::keys::keygen::{generate_hash_sig_keys, write_node_keys};

#[derive(Debug, Args)]
pub struct GenerateArgs {
    /// Total number of validators.
    #[arg(long, default_value = "100")]
    pub validators: u32,

    /// Client allocations as "name:pct,name:pct,..." (must sum to 100).
    #[arg(long, default_value = "ethlambda:50,qlean:50")]
    pub clients: String,

    /// Validators per Kubernetes pod.
    #[arg(long, default_value = "100")]
    pub validators_per_pod: u32,

    /// Kubernetes namespace.
    #[arg(long, default_value = "lean-devnet")]
    pub namespace: String,

    /// Output directory for all generated artifacts.
    #[arg(long, default_value = "./output")]
    pub output_dir: PathBuf,

    /// Hash-sig active epoch exponent (2^N).
    #[arg(long, default_value = "18")]
    pub active_epoch: u32,

    /// Key type.
    #[arg(long, default_value = "hash-sig")]
    pub key_type: String,

    /// Hex-encoded 32-byte seed for deterministic key generation.
    #[arg(long, default_value = "0000000000000000000000000000000000000000000000000000000000000001")]
    pub seed: String,

    /// Seconds until genesis time from now.
    #[arg(long, default_value = "120")]
    pub genesis_offset: u32,

    /// Kubernetes storage class for PVCs.
    #[arg(long)]
    pub storage_class: Option<String>,

    /// Number of bootnode pods per client type.
    #[arg(long, default_value = "5")]
    pub bootnode_count: u32,

    /// Skip Docker-based genesis generation (config-only mode).
    #[arg(long)]
    pub config_only: bool,
}

impl GenerateArgs {
    fn parse_clients(&self) -> Result<Vec<ClientAllocation>> {
        self.clients
            .split(',')
            .map(|s| {
                let parts: Vec<&str> = s.trim().split(':').collect();
                if parts.len() != 2 {
                    anyhow::bail!("Invalid client allocation: '{s}'. Expected 'name:percentage'");
                }
                Ok(ClientAllocation {
                    name: parts[0].to_string(),
                    percentage: parts[1]
                        .parse()
                        .with_context(|| format!("Invalid percentage in '{s}'"))?,
                })
            })
            .collect()
    }

    fn parse_seed(&self) -> Result<[u8; 32]> {
        let bytes = hex::decode(&self.seed).context("Invalid hex seed")?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|v: Vec<u8>| anyhow::anyhow!("Seed must be 32 bytes, got {}", v.len()))?;
        Ok(arr)
    }
}

pub fn run(args: GenerateArgs) -> Result<()> {
    let spec = DevnetSpec {
        validators: args.validators,
        clients: args.parse_clients()?,
        validators_per_pod: args.validators_per_pod,
        namespace: args.namespace.clone(),
        output_dir: args.output_dir.clone(),
        active_epoch: args.active_epoch,
        key_type: args.key_type.clone(),
        seed: args.parse_seed()?,
        genesis_offset: args.genesis_offset,
        storage_class: args.storage_class.clone(),
        bootnode_count: args.bootnode_count,
    };

    let genesis_dir = args.output_dir.join("genesis");

    // Step 1: Generate validator-config.yaml
    println!("==> Generating validator-config.yaml...");
    let vc = generate_validator_config(&spec)?;
    write_validator_config(&vc, &genesis_dir)?;

    // Step 2: Write node key files
    println!("==> Writing node key files...");
    let key_pairs: Vec<(String, String)> = vc
        .validators
        .iter()
        .map(|v| (v.name.clone(), v.privkey.clone()))
        .collect();
    write_node_keys(&key_pairs, &genesis_dir)?;

    if !args.config_only {
        // Step 3: Generate hash-sig keys
        let total_validators: u32 = vc.validators.iter().map(|v| v.count).sum();
        println!("==> Generating hash-sig keys for {total_validators} validators...");
        generate_hash_sig_keys(total_validators, spec.active_epoch, &genesis_dir)?;

        // Step 4: Write config.yaml and run genesis tool
        println!("==> Writing config.yaml...");
        write_config_yaml(&vc, spec.genesis_offset, &genesis_dir)?;

        println!("==> Running genesis generation tool...");
        run_genesis_tool(&genesis_dir)?;

        // Step 5: Append GENESIS_VALIDATORS to config.yaml
        println!("==> Appending GENESIS_VALIDATORS to config.yaml...");
        append_genesis_validators(&vc, &genesis_dir)?;
    }

    // Step 6: Generate Helm values and pod secrets
    println!("==> Generating Helm values...");
    let helm_values = generate_helm_values(&spec, &vc)?;
    write_helm_values(&helm_values, &args.output_dir)?;

    println!("==> Generating pod secret manifests...");
    generate_pod_secrets(&vc, &spec.namespace, &args.output_dir)?;

    println!("\n✓ Generation complete. Output in {}", args.output_dir.display());
    Ok(())
}
