mod support;
use serde_json::{Value, json};
use std::{fs, path::Path};
use support::{TempRoot, run, success};
use yu_engine_manager::sha256_file;

fn manifest(
    root: &TempRoot,
    archive: &Path,
    sha: &str,
    os: &str,
    kind: &str,
) -> std::path::PathBuf {
    let path = root.0.join("manifest.json");
    let value = json!({"schema_version":"1","id":"offline-fixture","display_name":"Offline fixture","version":"1.0.0","capabilities":[],"packages":[{
        "target":{"os":os,"arch":std::env::consts::ARCH},"url":"https://example.invalid/no-network-here",
        "sha256":sha,"archive":kind,"entrypoint":"bin/fixture","args":[]
    }]});
    assert!(archive.exists());
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}
fn attempt(root: &TempRoot, path: &Path, archive: &Path) -> std::process::Output {
    run(
        &root.0,
        &[
            "engine",
            "install",
            "--manifest",
            path.to_str().unwrap(),
            "--archive",
            archive.to_str().unwrap(),
            "--json",
        ],
    )
}
fn failure(output: std::process::Output, code: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["code"], code, "{value}");
}

#[test]
fn offline_install_verifies_bytes_without_activation_or_network() {
    let root = TempRoot::new("offline-roundtrip");
    let source = root.0.join("local archive ; $.bin");
    fs::write(&source, b"fixture engine bytes").unwrap();
    let hash = sha256_file(&source).unwrap();
    let path = manifest(&root, &source, &hash, std::env::consts::OS, "raw");
    let result = success(attempt(&root, &path, &source));
    assert_eq!(result["operation"], "engine.install");
    assert_eq!(result["result"]["sha256"], hash);
    assert_eq!(sha256_file(&source).unwrap(), hash);
    let versions = success(run(
        &root.0,
        &["engine", "versions", "offline-fixture", "--json"],
    ));
    assert_eq!(versions["result"][0]["active"], false);
    failure(attempt(&root, &path, &source), "OUTPUT_CONFLICT");
    let receipt = success(run(
        &root.0,
        &["engine", "remove", "offline-fixture", "1.0.0", "--json"],
    ));
    assert_eq!(receipt["result"]["cleanup_complete"], true);
    assert!(
        receipt["result"]["quarantine"]["attempts"]
            .as_u64()
            .unwrap()
            >= 1
    );
}

#[test]
fn offline_install_rejects_digest_target_and_archive_mismatch() {
    for (label, sha, os, kind, code) in [
        (
            "digest",
            "0".repeat(64),
            std::env::consts::OS,
            "raw",
            "VERIFICATION_FAILED",
        ),
        (
            "target",
            "0".repeat(64),
            "unsupported-target",
            "raw",
            "ENGINE_INCOMPATIBLE",
        ),
        (
            "archive",
            String::new(),
            std::env::consts::OS,
            "zip",
            "INVALID_INPUT",
        ),
    ] {
        let root = TempRoot::new(label);
        let source = root.0.join("source");
        fs::write(&source, b"not a zip").unwrap();
        let hash = sha256_file(&source).unwrap();
        let path = manifest(
            &root,
            &source,
            if sha.is_empty() { &hash } else { &sha },
            os,
            kind,
        );
        failure(attempt(&root, &path, &source), code);
        assert!(!root.0.join("engines/offline-fixture/1.0.0").exists());
        assert!(!root.0.join("state/engines/offline-fixture.json").exists());
        assert_eq!(sha256_file(&source).unwrap(), hash);
    }
}
