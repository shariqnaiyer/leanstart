use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Generate a deterministic Ed25519 private key from seed + index.
pub fn deterministic_privkey(seed: &[u8; 32], index: u32) -> String {
    let mut mac = HmacSha256::new_from_slice(seed).expect("HMAC accepts any key size");
    mac.update(&index.to_be_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Write node private key files to the output directory.
/// Each validator entry gets a `{name}.key` file containing its hex privkey.
pub fn write_node_keys(
    validators: &[(String, String)], // (name, privkey_hex)
    output_dir: &Path,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    for (name, privkey) in validators {
        let path = output_dir.join(format!("{name}.key"));
        fs::write(&path, privkey)?;
    }
    println!("Wrote {} node key files to {}", validators.len(), output_dir.display());
    Ok(())
}

/// Generate hash-sig validator keys using the hash-sig-cli Docker image.
///
/// Runs: `docker run blockblaz/hash-sig-cli:devnet2 generate --num-validators N
///        --log-num-active-epochs E --output-dir /genesis/hash-sig-keys --export-format both`
pub fn generate_hash_sig_keys(
    num_validators: u32,
    active_epoch: u32,
    output_dir: &Path,
) -> Result<()> {
    let keys_dir = output_dir.join("hash-sig-keys");
    fs::create_dir_all(&keys_dir)?;

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    let status = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--user",
            &format!("{uid}:{gid}"),
            "-v",
            &format!("{}:/genesis", output_dir.display()),
            "blockblaz/hash-sig-cli:devnet2",
            "generate",
            "--num-validators",
            &num_validators.to_string(),
            "--log-num-active-epochs",
            &active_epoch.to_string(),
            "--output-dir",
            "/genesis/hash-sig-keys",
            "--export-format",
            "both",
        ])
        .status()
        .context("Failed to run hash-sig-cli Docker container")?;

    if !status.success() {
        bail!("hash-sig-cli exited with status {status}");
    }

    println!(
        "Generated hash-sig keys for {num_validators} validators in {}",
        keys_dir.display()
    );
    Ok(())
}
