use std::path::PathBuf;

use leanstart::config::generator::generate_validator_config;
use leanstart::config::spec::{ClientAllocation, DevnetSpec, parse_client_spec};
use leanstart::k8s::values::generate_helm_values;

fn test_spec(clients: Vec<(&str, u32)>) -> DevnetSpec {
    DevnetSpec {
        clients: clients
            .into_iter()
            .map(|(name, instances)| ClientAllocation {
                name: name.to_string(),
                instances,
            })
            .collect(),
        validators_per_pod: 1,
        namespace: "test-ns".to_string(),
        output_dir: PathBuf::from("/tmp/leanstart-test"),
        active_epoch: 18,
        key_type: "hash-sig".to_string(),
        seed: [1u8; 32],
        genesis_offset: 120,
        storage_class: None,
        bootnode_count: 5,
        subnets: 1,
        attestation_committee_count: None,
    }
}

#[test]
fn test_parse_client_spec() {
    let c = parse_client_spec("ream").unwrap();
    assert_eq!(c.name, "ream");
    assert_eq!(c.instances, 1);

    let c = parse_client_spec("zeam:3").unwrap();
    assert_eq!(c.name, "zeam");
    assert_eq!(c.instances, 3);

    assert!(parse_client_spec("bad:spec:extra").is_err());
    assert!(parse_client_spec("zeam:abc").is_err());
}

#[test]
fn test_generate_validator_config_basic() {
    let spec = test_spec(vec![("ethlambda", 1), ("qlean", 1)]);
    let vc = generate_validator_config(&spec).unwrap();

    assert_eq!(vc.validators.len(), 2);
    assert_eq!(vc.validators[0].name, "ethlambda_0");
    assert_eq!(vc.validators[0].count, 1);
    assert_eq!(vc.validators[1].name, "qlean_0");
    assert_eq!(vc.validators[1].count, 1);

    assert!(vc.validators[0].is_aggregator);
    assert!(!vc.validators[1].is_aggregator);
}

#[test]
fn test_generate_multi_instance() {
    let spec = test_spec(vec![("ream", 1), ("zeam", 2)]);
    let vc = generate_validator_config(&spec).unwrap();

    assert_eq!(vc.validators.len(), 3);
    assert_eq!(vc.validators[0].name, "ream_0");
    assert_eq!(vc.validators[1].name, "zeam_0");
    assert_eq!(vc.validators[2].name, "zeam_1");

    let total: u32 = vc.validators.iter().map(|v| v.count).sum();
    assert_eq!(total, 3);
}

#[test]
fn test_deterministic_privkeys() {
    let spec = test_spec(vec![("ethlambda", 1), ("qlean", 1)]);
    let vc1 = generate_validator_config(&spec).unwrap();
    let vc2 = generate_validator_config(&spec).unwrap();

    for (a, b) in vc1.validators.iter().zip(vc2.validators.iter()) {
        assert_eq!(a.privkey, b.privkey);
    }
    assert_ne!(vc1.validators[0].privkey, vc1.validators[1].privkey);
}

#[test]
fn test_helm_values_per_pod_statefulset() {
    let spec = test_spec(vec![("ream", 1), ("zeam", 2)]);
    let vc = generate_validator_config(&spec).unwrap();
    let values = generate_helm_values(&spec, &vc).unwrap();

    // One StatefulSet per pod (replicas=1 each)
    assert_eq!(values.clients.len(), 3);
    assert_eq!(values.clients[0].name, "ream-0");
    assert_eq!(values.clients[0].replicas, 1);
    assert_eq!(values.clients[1].name, "zeam-0");
    assert_eq!(values.clients[1].replicas, 1);
    assert_eq!(values.clients[2].name, "zeam-1");
    assert_eq!(values.clients[2].replicas, 1);

    // zeam should have seccomp unconfined
    assert!(values.clients[1].seccomp_unconfined);
    assert!(values.clients[2].seccomp_unconfined);
    assert!(!values.clients[0].seccomp_unconfined);
}

#[test]
fn test_total_validators() {
    let spec = test_spec(vec![("ream", 1), ("zeam", 2)]);
    assert_eq!(spec.total_validators(), 3);

    let mut spec2 = test_spec(vec![("ream", 3), ("zeam", 5)]);
    spec2.validators_per_pod = 10;
    assert_eq!(spec2.total_validators(), 80);
}
