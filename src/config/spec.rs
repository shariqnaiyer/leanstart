use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// High-level specification for a devnet deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevnetSpec {
    /// Client allocations as (client_name, instance_count).
    pub clients: Vec<ClientAllocation>,
    /// Number of validators assigned to each pod.
    pub validators_per_pod: u32,
    /// Kubernetes namespace.
    pub namespace: String,
    /// Output directory for generated artifacts.
    pub output_dir: PathBuf,
    /// Exponent for hash-sig active epochs (2^active_epoch).
    pub active_epoch: u32,
    /// Key type (e.g., "hash-sig").
    pub key_type: String,
    /// Seed for deterministic key generation.
    pub seed: [u8; 32],
    /// Seconds from now until genesis time.
    pub genesis_offset: u32,
    /// Kubernetes storage class for PVCs.
    pub storage_class: Option<String>,
    /// Number of bootnode pods per client type.
    pub bootnode_count: u32,
}

/// A client type and how many instances (pods) to run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientAllocation {
    pub name: String,
    pub instances: u32,
}

impl DevnetSpec {
    /// Return (client_name, validator_count) for each client.
    /// Each instance gets `validators_per_pod` validators.
    pub fn validator_counts(&self) -> Vec<(String, u32)> {
        self.clients
            .iter()
            .map(|c| (c.name.clone(), c.instances * self.validators_per_pod))
            .collect()
    }

    /// Total number of validators across all clients.
    pub fn total_validators(&self) -> u32 {
        self.clients.iter().map(|c| c.instances).sum::<u32>() * self.validators_per_pod
    }
}

/// Parse a client spec string like "ream", "zeam:2", or "grandine:5".
pub fn parse_client_spec(spec: &str) -> anyhow::Result<ClientAllocation> {
    let parts: Vec<&str> = spec.split(':').collect();
    match parts.len() {
        1 => Ok(ClientAllocation {
            name: parts[0].to_string(),
            instances: 1,
        }),
        2 => Ok(ClientAllocation {
            name: parts[0].to_string(),
            instances: parts[1]
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid instance count in '{spec}'"))?,
        }),
        _ => anyhow::bail!("Invalid client spec '{spec}'. Use 'name' or 'name:count'"),
    }
}
