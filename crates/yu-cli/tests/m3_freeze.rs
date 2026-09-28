//! Offline consistency guards for the accepted M3 receipt, not live CI or pixel proof.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::PathBuf};
use yu_capability_psd::{PSD_CONTRACT_VERSION, PSD_LAYER_EXPORT};
use yu_engine_manager::EngineManifest;
use yu_runtime_psd::{DEFAULT_TIMEOUT_SECS, ENGINE_ID, READ_ONLY_CAPABILITIES};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(root().join(path)).expect("freeze evidence must exist"))
        .expect("freeze evidence must be valid JSON")
}
fn receipt() -> Value {
    read_json("docs/data/m3-implementation-freeze-v1.json")
}
fn strings(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn m3_freeze_contract_matches_wired_capabilities() {
    let r = receipt();
    assert_eq!(r["schema_version"], "1");
    assert_eq!(r["receipt_id"], "m3-implementation-freeze-v1");
    assert_eq!(r["status"], "frozen");
    assert_eq!(
        r["source"]["functional_baseline"],
        "c793279e3c131cae85adf73571590ab022b2ed05"
    );
    assert_eq!(
        r["source"]["functional_tree"],
        "3d42f311aa9d5cfca213ba6f852d9f9966767044"
    );
    assert_eq!(r["contracts"]["psd_contract_version"], PSD_CONTRACT_VERSION);
    assert_eq!(r["contracts"]["json_schema_version"], "1");
    assert_eq!(r["contracts"]["engine_manifest_version"], "1");
    assert_eq!(r["contracts"]["external_engine_protocol_version"], "1");
    assert_eq!(r["contracts"]["export_capability"], PSD_LAYER_EXPORT);
    let readonly: BTreeSet<_> = READ_ONLY_CAPABILITIES
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .collect();
    assert_eq!(strings(&r["contracts"]["readonly_capabilities"]), readonly);
    let mut wired = readonly;
    wired.insert(PSD_LAYER_EXPORT.to_owned());
    let contract = read_json("docs/data/psd-capability-contract-v1.json");
    assert_eq!(
        strings(&contract["reference_adapter"]["implemented_capabilities"]),
        wired
    );
    assert_eq!(
        r["limits"]["process_timeout_default_seconds"],
        DEFAULT_TIMEOUT_SECS
    );
    assert_eq!(r["contracts"]["automatic_fallback"], false);
    assert_eq!(r["contracts"]["explicit_install_and_activation"], true);
    assert_eq!(r["export"]["pixel_format"], "rgba8");
    assert_eq!(r["export"]["container"], "png");
    assert_eq!(r["export"]["source_bit_depth"], 8);
    assert_eq!(r["export"]["source_color_mode"], "rgb");
    assert_eq!(r["export"]["semantics"], "stored_layer_bitmap");
    assert_eq!(r["export"]["high_bit_depth"], "unsupported");
    assert_eq!(r["export"]["overwrite"], false);
    assert_eq!(r["export"]["hard_links_required"], true);
}

#[test]
fn m3_freeze_package_and_provenance_agree() {
    let r = receipt();
    let manifest_json = read_json(r["engine"]["manifest_path"].as_str().unwrap());
    let manifest: EngineManifest = serde_json::from_value(manifest_json.clone()).unwrap();
    let sources = read_json("packaging/ag-psd-engine/node-runtime-sources-v1.json");
    assert_eq!(manifest.id, ENGINE_ID);
    assert_eq!(manifest.schema_version, "1");
    assert_eq!(manifest.version, "31.0.2+node22.23.3.yu2");
    assert_eq!(r["engine"]["version"], manifest.version);
    assert_eq!(sources["engine_version"], manifest.version);
    assert_eq!(sources["ag_psd_version"], r["engine"]["ag_psd_version"]);
    assert_eq!(sources["node_version"], r["engine"]["node_version"]);
    assert_eq!(
        strings(&manifest_json["capabilities"]),
        strings(&sources["package_capabilities"])
    );
    assert_eq!(manifest.capabilities.len(), 5);
    assert_eq!(manifest.packages.len(), 3);
    let targets = r["package_provenance"]["targets"].as_array().unwrap();
    assert_eq!(targets.len(), 3);
    let expected_targets: BTreeSet<_> = [
        ("linux", "x86_64"),
        ("macos", "aarch64"),
        ("windows", "x86_64"),
    ]
    .into_iter()
    .map(|(os, arch)| (os.to_owned(), arch.to_owned()))
    .collect();
    let actual_targets: BTreeSet<_> = manifest
        .packages
        .iter()
        .map(|p| (p.target.os.clone(), p.target.arch.clone()))
        .collect();
    assert_eq!(actual_targets, expected_targets);
    for package in &manifest.packages {
        let matches: Vec<_> = targets
            .iter()
            .filter(|t| t["os"] == package.target.os && t["arch"] == package.target.arch)
            .collect();
        assert_eq!(matches.len(), 1);
        let proof = matches[0];
        assert_eq!(proof["package_sha256"], package.sha256);
        assert_eq!(package.sha256.len(), 64);
        assert!(package.sha256.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(proof["cli_tests_passed"], 2);
        assert_ne!(proof["actions_artifact_sha256"], proof["package_sha256"]);
        assert!(proof["actions_artifact_id"].as_u64().unwrap() > 0);
        assert!(
            package
                .url
                .starts_with("https://example.invalid/yu-tool/ag-psd-engine-export-v2/")
        );
        assert_eq!(package.args, ["engine/ag_psd_protocol.cjs"]);
        assert_eq!(
            package.entrypoint,
            if package.target.os == "windows" {
                "runtime/node.exe"
            } else {
                "runtime/node"
            }
        );
        assert!(
            sources["validated_targets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["target"] == json!({"os":package.target.os,"arch":package.target.arch}))
        );
    }
}

#[test]
fn m3_freeze_preserves_historical_evidence_and_release_boundary() {
    let r = receipt();
    assert_eq!(
        r["historical_evidence_hash_normalization"],
        "utf8_crlf_to_lf"
    );
    let evidence = r["historical_evidence"].as_array().unwrap();
    assert_eq!(evidence.len(), 3);
    for item in evidence {
        // Git may check text out with CRLF on Windows; the recorded digests use LF.
        let text = fs::read_to_string(root().join(item["path"].as_str().unwrap())).unwrap();
        let digest = format!(
            "{:x}",
            Sha256::digest(text.replace("\r\n", "\n").as_bytes())
        );
        assert_eq!(
            item["sha256"], digest,
            "historical evidence must not be silently rewritten"
        );
    }
    let workflows = r["implementation_workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), 3);
    let ids: BTreeSet<_> = workflows
        .iter()
        .map(|w| w["run_id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, BTreeSet::from([36366437339, 36366437367, 36366437379]));
    for workflow in workflows {
        assert_eq!(workflow["status"], "completed");
        assert_eq!(workflow["conclusion"], "success");
        assert_eq!(workflow["head_sha"], r["source"]["reviewed_head"]);
    }
    assert_eq!(r["acceptance"]["independent_rgb8_workloads"], 5);
    assert_eq!(r["acceptance"]["high_bit_rejection_workloads"], 2);
    assert_eq!(r["acceptance"]["rgba_bytes_per_simple_fixture"], 37860);
    assert_eq!(
        r["acceptance"]["review_local_tests"]["real_package_executed_locally"],
        false
    );
    assert_eq!(r["release"]["status"], "not_released");
    assert_eq!(r["release"]["distribution"], "prototype_ci_artifact_only");
    for field in [
        "public_catalog",
        "production_hosting",
        "signing_and_notarization_accepted",
        "whole_application_reproducible_build_accepted",
        "project_license_decided",
    ] {
        assert_eq!(r["release"][field], false);
    }
    assert_eq!(r["limits"]["whole_process_memory_guarantee"], false);
    assert_eq!(r["limits"]["hard_filesystem_or_decoder_deadline"], false);
}
