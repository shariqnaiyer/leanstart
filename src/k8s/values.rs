use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::clients::{build_args, get_client};
use crate::config::generator::ValidatorConfig;
use crate::config::spec::DevnetSpec;

/// Top-level Helm values structure.
#[derive(Debug, Serialize, Deserialize)]
pub struct HelmValues {
    pub namespace: String,
    pub genesis: GenesisValues,
    pub clients: Vec<ClientValues>,
    #[serde(rename = "initScripts")]
    pub init_scripts: InitScriptsValues,
    #[serde(rename = "bootnodeCount")]
    pub bootnode_count: u32,
    pub prometheus: PrometheusValues,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenesisValues {
    #[serde(rename = "configMapName")]
    pub config_map_name: String,
    #[serde(rename = "pvcName")]
    pub pvc_name: String,
    #[serde(rename = "storageClass")]
    pub storage_class: String,
    #[serde(rename = "storageSize")]
    pub storage_size: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientValues {
    pub name: String,
    pub image: String,
    pub replicas: u32,
    pub args: Vec<Vec<String>>,
    #[serde(rename = "seccompUnconfined")]
    pub seccomp_unconfined: bool,
    #[serde(rename = "hasHttpPort")]
    pub has_http_port: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitScriptsValues {
    #[serde(rename = "resolverImage")]
    pub resolver_image: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusValues {
    pub enabled: bool,
}

/// Generate Helm values.yaml from DevnetSpec and ValidatorConfig.
///
/// Each validator entry becomes its own StatefulSet with replicas=1,
/// ensuring every pod gets its correct per-pod args (node-id, keys, etc).
pub fn generate_helm_values(
    spec: &DevnetSpec,
    vc: &ValidatorConfig,
) -> Result<HelmValues> {
    let mut clients = Vec::new();

    for (vc_idx, entry) in vc.validators.iter().enumerate() {
        // Extract client type from name (e.g. "zeam_1" -> "zeam")
        let client_name = entry
            .name
            .rsplit_once('_')
            .map(|(name, _)| name)
            .unwrap_or(&entry.name);

        let client_def = get_client(client_name).unwrap();

        let args = build_args(
            client_def,
            &entry.name,
            vc_idx,
            entry.is_aggregator,
            None,
        );

        let image = if client_def.arch_aware {
            format!("{}-amd64", client_def.image)
        } else {
            client_def.image.to_string()
        };

        // K8s-safe name: zeam_0 -> zeam-0
        let k8s_name = entry.name.replace('_', "-");

        clients.push(ClientValues {
            name: k8s_name,
            image,
            replicas: 1,
            args: vec![args],
            seccomp_unconfined: client_def.seccomp_unconfined,
            has_http_port: client_def.has_http_port,
        });
    }

    Ok(HelmValues {
        namespace: spec.namespace.clone(),
        genesis: GenesisValues {
            config_map_name: "genesis-config".into(),
            pvc_name: "genesis-data".into(),
            storage_class: spec.storage_class.clone().unwrap_or_default(),
            storage_size: "5Gi".into(),
        },
        clients,
        init_scripts: InitScriptsValues {
            resolver_image: "busybox:1.36".into(),
        },
        bootnode_count: spec.bootnode_count,
        prometheus: PrometheusValues { enabled: true },
    })
}

/// Write Helm values to a YAML file.
pub fn write_helm_values(values: &HelmValues, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join("helm-values.yaml");
    let yaml = serde_yaml::to_string(values)?;
    fs::write(&path, yaml)?;
    println!("Wrote {}", path.display());
    Ok(())
}

/// Generate per-pod Secret manifests for node keys.
pub fn generate_pod_secrets(
    vc: &ValidatorConfig,
    namespace: &str,
    output_dir: &Path,
) -> Result<()> {
    let secrets_dir = output_dir.join("secrets");
    fs::create_dir_all(&secrets_dir)?;

    for entry in &vc.validators {
        let secret = serde_yaml::to_string(&serde_yaml::Value::Mapping({
            let mut m = serde_yaml::Mapping::new();
            m.insert("apiVersion".into(), "v1".into());
            m.insert("kind".into(), "Secret".into());
            let mut metadata = serde_yaml::Mapping::new();
            let k8s_name = entry.name.replace('_', "-");
            metadata.insert("name".into(), format!("{k8s_name}-keys").into());
            metadata.insert("namespace".into(), namespace.into());
            m.insert("metadata".into(), serde_yaml::Value::Mapping(metadata));
            m.insert("type".into(), "Opaque".into());
            let mut data = serde_yaml::Mapping::new();
            data.insert("node.key".into(), entry.privkey.clone().into());
            m.insert("stringData".into(), serde_yaml::Value::Mapping(data));
            m
        }))?;

        let k8s_name = entry.name.replace('_', "-");
        let path = secrets_dir.join(format!("{k8s_name}-keys.yaml"));
        fs::write(&path, secret)?;
    }

    println!(
        "Wrote {} pod secret manifests to {}",
        vc.validators.len(),
        secrets_dir.display()
    );
    Ok(())
}
