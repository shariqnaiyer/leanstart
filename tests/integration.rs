use std::path::PathBuf;

use lean_devnet::config::generator::generate_validator_config;
use lean_devnet::config::spec::{ClientAllocation, DevnetSpec};
use lean_devnet::k8s::values::generate_helm_values;

fn test_spec(validators: u32, clients: Vec<(&str, u32)>) -> DevnetSpec {
    DevnetSpec {
        validators,
        clients: clients
            .into_iter()
            .map(|(name, pct)| ClientAllocation {
                name: name.to_string(),
                percentage: pct,
            })
            .collect(),
        validators_per_pod: 100,
        namespace: "test-ns".to_string(),
        output_dir: PathBuf::from("/tmp/lean-devnet-test"),
        active_epoch: 18,
        key_type: "hash-sig".to_string(),
        seed: [1u8; 32],
        genesis_offset: 120,
        storage_class: None,
        bootnode_count: 5,
    }
}

#[test]
fn test_generate_validator_config_basic() {
    let spec = test_spec(10, vec![("ethlambda", 50), ("qlean", 50)]);
    let vc = generate_validator_config(&spec).unwrap();

    // 10 validators, 100 per pod -> 1 pod per client
    assert_eq!(vc.validators.len(), 2);
    assert_eq!(vc.validators[0].name, "ethlambda_0");
    assert_eq!(vc.validators[0].count, 5);
    assert_eq!(vc.validators[1].name, "qlean_0");
    assert_eq!(vc.validators[1].count, 5);

    // First pod is aggregator
    assert!(vc.validators[0].is_aggregator);
    assert!(!vc.validators[1].is_aggregator);

    // Total validator count matches
    let total: u32 = vc.validators.iter().map(|v| v.count).sum();
    assert_eq!(total, 10);
}

#[test]
fn test_generate_multi_pod() {
    let mut spec = test_spec(250, vec![("ethlambda", 60), ("qlean", 40)]);
    spec.validators_per_pod = 100;
    let vc = generate_validator_config(&spec).unwrap();

    // ethlambda: 150 validators -> 2 pods (100 + 50)
    // qlean: 100 validators -> 1 pod (100)
    assert_eq!(vc.validators.len(), 3);
    assert_eq!(vc.validators[0].name, "ethlambda_0");
    assert_eq!(vc.validators[0].count, 100);
    assert_eq!(vc.validators[1].name, "ethlambda_1");
    assert_eq!(vc.validators[1].count, 50);
    assert_eq!(vc.validators[2].name, "qlean_0");
    assert_eq!(vc.validators[2].count, 100);
}

#[test]
fn test_deterministic_privkeys() {
    let spec = test_spec(10, vec![("ethlambda", 50), ("qlean", 50)]);
    let vc1 = generate_validator_config(&spec).unwrap();
    let vc2 = generate_validator_config(&spec).unwrap();

    // Same seed produces same keys
    for (a, b) in vc1.validators.iter().zip(vc2.validators.iter()) {
        assert_eq!(a.privkey, b.privkey);
    }

    // Different pods get different keys
    assert_ne!(vc1.validators[0].privkey, vc1.validators[1].privkey);
}

#[test]
fn test_helm_values_generation() {
    let spec = test_spec(10, vec![("ethlambda", 50), ("qlean", 50)]);
    let vc = generate_validator_config(&spec).unwrap();
    let values = generate_helm_values(&spec, &vc).unwrap();

    assert_eq!(values.namespace, "test-ns");
    assert_eq!(values.clients.len(), 2);
    assert_eq!(values.clients[0].name, "ethlambda");
    assert_eq!(values.clients[0].replicas, 1);
    assert_eq!(values.clients[1].name, "qlean");
    assert!(!values.clients[0].seccomp_unconfined);
    assert_eq!(values.bootnode_count, 5);
}

#[test]
fn test_enr_dns_hostnames() {
    let spec = test_spec(10, vec![("ethlambda", 50), ("qlean", 50)]);
    let vc = generate_validator_config(&spec).unwrap();

    assert_eq!(
        vc.validators[0].enr_fields.ip,
        "ethlambda-0.ethlambda-headless.test-ns.svc.cluster.local"
    );
    assert_eq!(vc.validators[0].enr_fields.quic, 9000);
}
