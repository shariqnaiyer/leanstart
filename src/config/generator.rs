use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::clients::get_client;
use crate::config::spec::DevnetSpec;
use crate::keys::keygen::deterministic_privkey;

/// A single entry in the generated validator-config.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorEntry {
    pub name: String,
    pub privkey: String,
    #[serde(rename = "enrFields")]
    pub enr_fields: EnrFields,
    #[serde(rename = "metricsPort")]
    pub metrics_port: u16,
    #[serde(rename = "httpPort", skip_serializing_if = "Option::is_none")]
    pub http_port: Option<u16>,
    #[serde(rename = "isAggregator")]
    pub is_aggregator: bool,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrFields {
    pub ip: String,
    pub quic: u16,
}

/// Top-level validator-config.yaml structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    pub shuffle: String,
    pub deployment_mode: String,
    pub config: ValidatorConfigMeta,
    pub validators: Vec<ValidatorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfigMeta {
    #[serde(rename = "activeEpoch")]
    pub active_epoch: u32,
    #[serde(rename = "keyType")]
    pub key_type: String,
}

/// Generate the complete validator-config.yaml from a DevnetSpec.
pub fn generate_validator_config(spec: &DevnetSpec) -> Result<ValidatorConfig> {
    let client_counts = spec.validator_counts();
    let mut validators = Vec::new();
    let mut global_pod_index: u32 = 0;
    let mut first_pod = true;

    for (client_name, validator_count) in &client_counts {
        let client_def = get_client(client_name)
            .with_context(|| format!("Unknown client: {client_name}"))?;

        if *validator_count == 0 {
            bail!("Client {client_name} has 0 validators allocated");
        }

        let pod_count = validator_count.div_ceil(spec.validators_per_pod);
        let mut remaining = *validator_count;

        for pod_idx in 0..pod_count {
            let count = remaining.min(spec.validators_per_pod);
            remaining -= count;

            let name = format!("{}_{}", client_name, pod_idx);
            let privkey = deterministic_privkey(&spec.seed, global_pod_index);

            // Use 0.0.0.0 as placeholder — the genesis tool requires a valid IP.
            // Actual pod IPs are resolved at runtime by the init container.
            let entry = ValidatorEntry {
                name,
                privkey,
                enr_fields: EnrFields {
                    ip: "0.0.0.0".to_string(),
                    quic: 9000,
                },
                metrics_port: 8080,
                http_port: if client_def.has_http_port {
                    Some(5055)
                } else {
                    None
                },
                is_aggregator: first_pod,
                count,
            };

            validators.push(entry);
            global_pod_index += 1;
            first_pod = false;
        }
    }

    Ok(ValidatorConfig {
        shuffle: "roundrobin".to_string(),
        deployment_mode: "kubernetes".to_string(),
        config: ValidatorConfigMeta {
            active_epoch: spec.active_epoch,
            key_type: spec.key_type.clone(),
        },
        validators,
    })
}

/// Write the validator config to a YAML file.
pub fn write_validator_config(config: &ValidatorConfig, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join("validator-config.yaml");
    let yaml = serde_yaml::to_string(config)?;
    fs::write(&path, yaml)?;
    println!("Wrote {}", path.display());
    Ok(())
}
