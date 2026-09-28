mod support;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use support::{TempRoot, activate, error, install, run, success};
use yu_engine_manager::{EngineManifest, sha256_file};

#[path = "support/psd_export_managed.rs"]
mod export_tests;
#[path = "support/psd_lifecycle_managed.rs"]
mod lifecycle_tests;

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/psd")
        .join(relative)
}
fn command(root: &Path, file: &Path, action: &[&str]) -> Value {
    let mut args = vec!["psd"];
    args.extend_from_slice(action);
    args.push(file.to_str().unwrap());
    args.push("--json");
    success(run(root, &args))
}

#[test]
#[ignore = "requires YU_TEST_MANIFEST and YU_TEST_PACKAGE from the verified package builder"]
fn managed_package_readonly_cli() {
    let manifest_path =
        PathBuf::from(std::env::var_os("YU_TEST_MANIFEST").expect("YU_TEST_MANIFEST is required"));
    let package_path =
        PathBuf::from(std::env::var_os("YU_TEST_PACKAGE").expect("YU_TEST_PACKAGE is required"));
    let manifest: EngineManifest =
        serde_json::from_slice(&fs::read(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest.id, "ag-psd");
    let root = TempRoot::new("real-managed-package");
    install(&root, &manifest, &package_path);
    let psd = fixture("upstream/psd-tools/2layers.psd");
    error(
        run(
            &root.0,
            &["psd", "inspect", psd.to_str().unwrap(), "--json"],
        ),
        3,
        "ENGINE_UNAVAILABLE",
    );
    activate(&root.0, &manifest.version);

    let corpus: Value = serde_json::from_slice(&fs::read(fixture("corpus.json")).unwrap()).unwrap();
    let mut checked = Vec::new();
    for item in corpus["fixtures"].as_array().unwrap() {
        let file = fixture(item["path"].as_str().unwrap());
        let hash = sha256_file(&file).unwrap();
        if item["expected"]["parse"] == "reject" {
            error(
                run(
                    &root.0,
                    &["psd", "inspect", file.to_str().unwrap(), "--json"],
                ),
                2,
                "INVALID_INPUT",
            );
            assert_eq!(sha256_file(&file).unwrap(), hash);
            checked.push(item["id"].clone());
            continue;
        }
        let inspect = command(&root.0, &file, &["inspect"]);
        let tree = command(&root.0, &file, &["tree"]);
        let list = command(&root.0, &file, &["layer", "list"]);
        for (value, operation) in [
            (&inspect, "psd.inspect"),
            (&tree, "psd.tree"),
            (&list, "psd.layer.list"),
        ] {
            assert_eq!(value["schema_version"], "1");
            assert_eq!(value["operation"], operation);
            assert_eq!(value["engine"]["id"], "ag-psd");
            assert_eq!(value["engine"]["provider"], "managed");
            assert_eq!(value["engine"]["version"], manifest.version);
            assert_eq!(value["result"]["contract_version"], "1");
            assert_eq!(
                value["result"]["document"]["width"],
                item["expected"]["width"]
            );
            assert_eq!(
                value["result"]["document"]["height"],
                item["expected"]["height"]
            );
            assert_eq!(
                value["result"]["document"]["layer_count"],
                item["expected"]["layer_count"]
            );
            assert_eq!(value["result"]["document"]["format"], item["format"]);
        }
        let layers = list["result"]["layers"].as_array().unwrap();
        for (index, layer) in layers.iter().enumerate() {
            assert_eq!(layer["id"], format!("L{:04}", index + 1));
            let id = layer["id"].as_str().unwrap();
            let info = success(run(
                &root.0,
                &[
                    "psd",
                    "layer",
                    "info",
                    file.to_str().unwrap(),
                    "--id",
                    id,
                    "--engine",
                    "ag-psd",
                    "--json",
                ],
            ));
            assert_eq!(info["operation"], "psd.layer.info");
            assert_eq!(&info["result"]["layer"], layer);
        }
        if item["id"] == "duplicate-layer-names" {
            assert_eq!(layers[0]["name"], layers[1]["name"]);
            assert_ne!(layers[0]["id"], layers[1]["id"]);
        }
        assert_eq!(sha256_file(&file).unwrap(), hash);
        checked.push(item["id"].clone());
    }
    assert_eq!(checked.len(), 7);
    error(
        run(
            &root.0,
            &[
                "psd",
                "layer",
                "info",
                psd.to_str().unwrap(),
                "--id",
                "L9999",
                "--json",
            ],
        ),
        2,
        "INVALID_ARGUMENT",
    );
    error(
        run(
            &root.0,
            &[
                "psd",
                "inspect",
                psd.to_str().unwrap(),
                "--engine",
                "psd-tools",
                "--json",
            ],
        ),
        3,
        "ENGINE_UNAVAILABLE",
    );
    let renamed = root.0.join("设计 file ; dollar$.psd");
    fs::copy(&psd, &renamed).unwrap();
    assert_eq!(
        command(&root.0, &renamed, &["inspect"])["result"]["document"]["width"],
        101
    );
    let available = success(run(&root.0, &["capabilities", "--json"]));
    assert_eq!(
        available["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["id"].as_str().unwrap().starts_with("psd."))
            .count(),
        5
    );
    let info = success(run(&root.0, &["engine", "info", "ag-psd", "--json"]));
    assert_eq!(info["result"][0]["state"], "ready");
    success(run(&root.0, &["engine", "deactivate", "ag-psd", "--json"]));
    error(
        run(
            &root.0,
            &["psd", "inspect", psd.to_str().unwrap(), "--json"],
        ),
        3,
        "ENGINE_UNAVAILABLE",
    );
    let inactive = success(run(&root.0, &["capabilities", "--json"]));
    assert!(
        !inactive["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"].as_str().unwrap().starts_with("psd."))
    );
    success(run(
        &root.0,
        &["engine", "remove", "ag-psd", &manifest.version, "--json"],
    ));
    assert!(
        !root
            .0
            .join("engines/ag-psd")
            .join(&manifest.version)
            .exists()
    );
    println!(
        "{}",
        serde_json::json!({
            "schema_version":"1", "engine_id":"ag-psd", "engine_version":manifest.version,
            "target":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH},
            "corpus":checked, "readonly_capabilities":4, "system_node_required":false,
            "node_environment_sanitized":true, "source_files_unchanged":true, "lifecycle_cleanup":true
        })
    );
}
