use image::{Rgba, RgbaImage};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};
use yu_engine_manager::{
    ArchiveKind, Downloader, EngineInstaller, EngineManager, EngineManifest, EnginePackage,
    EngineTarget, MANIFEST_SCHEMA_VERSION, ManagedLayout, ManagerError,
};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yu"))
        .args(args)
        .output()
        .expect("yu should execute")
}

fn run_with_data_home(args: &[&str], data_home: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yu"))
        .args(args)
        .env("YU_DATA_HOME", data_home)
        .output()
        .expect("yu should execute")
}

fn parse_stdout(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON")
}

fn temp_path(label: &str, extension: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "yu-cli-{label}-{}-{nonce}.{extension}",
        std::process::id()
    ))
}

fn create_png(path: &Path, width: u32, height: u32) {
    let image = RgbaImage::from_pixel(width, height, Rgba([32, 64, 128, 255]));
    image.save(path).expect("fixture should save");
}

#[test]
fn doctor_json_is_machine_readable() {
    let root = temp_path("doctor-data", "dir");
    fs::create_dir_all(&root).unwrap();
    let output = run_with_data_home(&["doctor", "--json"], &root);
    let json = parse_stdout(&output);

    assert_eq!(json["schema_version"], "1");
    assert_eq!(json["operation"], "runtime.doctor");
    assert!(json["result"]["engines"]["total"].as_u64().unwrap() >= 2);
    assert!(json["result"]["engines"]["ready"].as_u64().unwrap() >= 2);
    assert_eq!(json["result"]["engines"]["built_in"], 2);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn capabilities_json_contains_runtime_and_image_capabilities() {
    let output = run(&["capabilities", "--json"]);
    let json = parse_stdout(&output);
    let capabilities = json["result"]
        .as_array()
        .expect("result should be an array");

    assert!(
        capabilities
            .iter()
            .any(|item| item["id"] == "runtime.doctor")
    );
    assert!(
        capabilities
            .iter()
            .any(|item| item["id"] == "runtime.capabilities")
    );
    assert!(capabilities.iter().any(|item| item["id"] == "engine.list"));
    assert!(capabilities.iter().any(|item| item["id"] == "engine.info"));
    assert!(capabilities.iter().any(|item| item["id"] == "image.info"));
    assert!(capabilities.iter().any(|item| item["id"] == "image.resize"));
}

#[test]
fn engine_list_json_contains_built_in_runtime_and_raster_engine() {
    let output = run(&["engine", "list", "--json"]);
    let json = parse_stdout(&output);
    let engines = json["result"]
        .as_array()
        .expect("result should be an array");

    assert!(engines.len() >= 2);
    assert!(
        engines
            .iter()
            .any(|engine| engine["id"] == "yu-runtime" && engine["state"] == "ready")
    );
    assert!(
        engines
            .iter()
            .any(|engine| engine["id"] == "raster-rs" && engine["provider"] == "built_in")
    );
}

#[test]
fn engine_info_json_reports_built_in_provider() {
    let output = run(&["engine", "info", "raster-rs", "--json"]);
    let json = parse_stdout(&output);
    let entries = json["result"]
        .as_array()
        .expect("engine.info result should be an array");

    assert_eq!(json["operation"], "engine.info");
    assert!(entries.iter().any(|engine| engine["id"] == "raster-rs"
        && engine["provider"] == "built_in"
        && engine["state"] == "ready"));
}

#[test]
fn engine_info_unknown_returns_engine_unavailable() {
    let output = run(&["engine", "info", "does-not-exist", "--json"]);

    assert_eq!(output.status.code(), Some(3));
    let json: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be structured JSON");
    assert_eq!(json["error"]["code"], "ENGINE_UNAVAILABLE");
}

#[test]
fn image_info_json_reports_fixture_metadata() {
    let input = temp_path("info-input", "png");
    create_png(&input, 4, 2);

    let output = run(&["image", "info", input.to_str().unwrap(), "--json"]);
    let json = parse_stdout(&output);

    assert_eq!(json["operation"], "image.info");
    assert_eq!(json["engine"]["id"], "raster-rs");
    assert_eq!(json["result"]["format"], "png");
    assert_eq!(json["result"]["width"], 4);
    assert_eq!(json["result"]["height"], 2);
    assert_eq!(json["result"]["channels"], 4);
    assert_eq!(json["result"]["has_alpha"], true);

    let _ = fs::remove_file(input);
}

#[test]
fn image_resize_width_only_preserves_aspect_ratio() {
    let input = temp_path("resize-input", "png");
    let output_path = temp_path("resize-output", "png");
    create_png(&input, 4, 2);

    let output = run(&[
        "image",
        "resize",
        input.to_str().unwrap(),
        "--width",
        "2",
        "-o",
        output_path.to_str().unwrap(),
        "--json",
    ]);
    let json = parse_stdout(&output);

    assert_eq!(json["operation"], "image.resize");
    assert_eq!(json["engine"]["id"], "raster-rs");
    assert_eq!(json["result"]["source_width"], 4);
    assert_eq!(json["result"]["source_height"], 2);
    assert_eq!(json["result"]["width"], 2);
    assert_eq!(json["result"]["height"], 1);

    let resized = image::open(&output_path).expect("resized image should decode");
    assert_eq!(resized.width(), 2);
    assert_eq!(resized.height(), 1);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output_path);
}

#[test]
fn image_resize_rejects_existing_output() {
    let input = temp_path("conflict-input", "png");
    let output_path = temp_path("conflict-output", "png");
    create_png(&input, 4, 2);
    create_png(&output_path, 1, 1);

    let output = run(&[
        "image",
        "resize",
        input.to_str().unwrap(),
        "--width",
        "2",
        "-o",
        output_path.to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(2));
    let json: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be structured JSON");
    assert_eq!(json["error"]["code"], "OUTPUT_CONFLICT");

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output_path);
}

#[test]
fn explicit_incompatible_engine_returns_structured_error() {
    let input = temp_path("engine-input", "png");
    create_png(&input, 4, 2);

    let output = run(&[
        "image",
        "info",
        input.to_str().unwrap(),
        "--engine",
        "yu-runtime",
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(3));
    let json: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be structured JSON");
    assert_eq!(json["error"]["code"], "ENGINE_INCOMPATIBLE");

    let _ = fs::remove_file(input);
}

#[derive(Clone)]
struct FixtureDownloader {
    bytes: Vec<u8>,
}

impl Downloader for FixtureDownloader {
    fn download(&self, _url: &str, destination: &Path) -> Result<u64, ManagerError> {
        fs::write(destination, &self.bytes)
            .map_err(|error| ManagerError::Io(format!("fixture download failed: {error}")))?;
        Ok(self.bytes.len() as u64)
    }
}

fn fixture_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn install_fixture_engine(data_home: &Path, version: &str) {
    let bytes = format!("fixture-{version}").into_bytes();
    let target = EngineTarget::current();
    let manifest = EngineManifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_owned(),
        id: "fixture-engine".to_owned(),
        display_name: "Fixture Engine".to_owned(),
        version: version.to_owned(),
        capabilities: vec!["fixture.run".to_owned()],
        packages: vec![EnginePackage {
            target: target.clone(),
            url: "https://example.invalid/fixture".to_owned(),
            sha256: fixture_sha256(&bytes),
            archive: ArchiveKind::Raw,
            entrypoint: "bin/fixture".to_owned(),
            args: Vec::new(),
        }],
    };
    let manager = EngineManager::new(ManagedLayout::new(data_home), target);
    let installer = EngineInstaller::new(manager, FixtureDownloader { bytes });
    installer
        .install(&manifest)
        .expect("fixture install should succeed");
}

#[test]
fn engine_versions_activate_deactivate_remove_round_trip() {
    let root = temp_path("engine-lifecycle", "dir");
    fs::create_dir_all(&root).unwrap();
    install_fixture_engine(&root, "1.0.0");
    install_fixture_engine(&root, "2.0.0");

    let output = run_with_data_home(&["engine", "list", "--json"], &root);
    let json = parse_stdout(&output);
    let managed = json["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|engine| engine["id"] == "fixture-engine" && engine["provider"] == "managed")
        .expect("managed fixture should be discovered");
    assert_eq!(managed["state"], "disabled");
    assert_eq!(managed["installed_versions"].as_array().unwrap().len(), 2);

    let output = run_with_data_home(&["engine", "versions", "fixture-engine", "--json"], &root);
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "engine.versions");
    assert_eq!(json["result"].as_array().unwrap().len(), 2);

    let output = run_with_data_home(
        &["engine", "activate", "fixture-engine", "2.0.0", "--json"],
        &root,
    );
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "engine.activate");
    assert_eq!(json["result"]["active_version"], "2.0.0");

    let output = run_with_data_home(&["engine", "info", "fixture-engine", "--json"], &root);
    let json = parse_stdout(&output);
    let managed = json["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|engine| engine["provider"] == "managed")
        .expect("managed fixture info should be discovered");
    assert_eq!(managed["state"], "ready");
    assert_eq!(managed["active_version"], "2.0.0");
    assert_eq!(managed["display_name"], "Fixture Engine");
    assert_eq!(managed["capabilities"][0], "fixture.run");

    let output = run_with_data_home(
        &["engine", "remove", "fixture-engine", "2.0.0", "--json"],
        &root,
    );
    assert_eq!(output.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "OUTPUT_CONFLICT");

    let output = run_with_data_home(&["engine", "deactivate", "fixture-engine", "--json"], &root);
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "engine.deactivate");
    assert_eq!(json["result"]["previous_version"], "2.0.0");

    let output = run_with_data_home(
        &["engine", "remove", "fixture-engine", "2.0.0", "--json"],
        &root,
    );
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "engine.remove");
    assert_eq!(json["result"]["version"], "2.0.0");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn engine_install_rejects_invalid_manifest_before_network() {
    let root = temp_path("engine-invalid", "dir");
    fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
          "schema_version":"1",
          "id":"fixture-engine",
          "display_name":"Fixture",
          "version":"1.0.0",
          "capabilities":[],
          "packages":[{
            "target":{"os":"linux","arch":"x86_64"},
            "url":"http://example.invalid/fixture",
            "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "archive":"raw",
            "entrypoint":"bin/fixture"
          }]
        }"#,
    )
    .unwrap();

    let output = run_with_data_home(
        &[
            "engine",
            "install",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--json",
        ],
        &root,
    );

    assert_eq!(output.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(json["error"]["code"], "INVALID_INPUT");

    let _ = fs::remove_dir_all(root);
}
