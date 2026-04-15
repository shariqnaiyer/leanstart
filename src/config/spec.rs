use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// High-level specification for a devnet deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevnetSpec {
    /// Total number of validators across all clients.
    pub validators: u32,
    /// Client allocations as (client_name, percentage).
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

/// A client type and its share of the total validator count (as a percentage).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientAllocation {
    pub name: String,
    pub percentage: u32,
}

impl DevnetSpec {
    /// Compute how many validators each client gets, distributing remainders
    /// to the first client(s) in order.
    pub fn validator_counts(&self) -> Vec<(String, u32)> {
        let total_pct: u32 = self.clients.iter().map(|c| c.percentage).sum();
        assert!(total_pct == 100, "Client percentages must sum to 100, got {total_pct}");

        let mut counts: Vec<(String, u32)> = self
            .clients
            .iter()
            .map(|c| {
                let count = (self.validators as u64 * c.percentage as u64 / 100) as u32;
                (c.name.clone(), count)
            })
            .collect();

        let assigned: u32 = counts.iter().map(|(_, c)| *c).sum();
        let remainder = self.validators - assigned;
        for i in 0..remainder as usize {
            counts[i].1 += 1;
        }

        counts
    }
}
