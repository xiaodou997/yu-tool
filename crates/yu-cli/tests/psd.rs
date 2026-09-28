mod support;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    time::{Duration, Instant},
};
use support::{TempRoot, activate, error, install, run, success};
use yu_engine_manager::{ArchiveKind, EngineManifest, EnginePackage, EngineTarget, sha256_file};

#[path = "support/psd_exports.rs"]
mod exports;

#[path = "support/transport_lifecycle.rs"]
mod transport_lifecycle;

#[cfg(windows)]
#[path = "support/removal_contract.rs"]
mod removal_contract;

const CAPS: [&str; 4] = [
    "psd.inspect",
    "psd.tree",
    "psd.layer.list",
    "psd.layer.info",
];
static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
fn fixture_binary() -> &'static Path {
    FIXTURE.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("psd-helper-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let binary = dir.join(if cfg!(windows) {
            "fixture.exe"
        } else {
            "fixture"
        });
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(compiler)
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/protocol_fixture.rs"))
            .args([
                "--edition=2024",
                "-C",
                "opt-level=1",
                "-C",
                "strip=symbols",
                "-o",
            ])
            .arg(&binary)
            .output()
            .expect("rustc is required for the native test engine");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        binary
    })
}
fn provision(root: &TempRoot, mode: &str, version: &str, caps: &[&str]) {
    provision_with_args(root, mode, version, caps, &[]);
}
fn provision_with_args(
    root: &TempRoot,
    mode: &str,
    version: &str,
    caps: &[&str],
    extra: &[String],
) {
    let binary = fixture_binary();
    let manifest = EngineManifest {
        schema_version: "1".into(),
        id: "ag-psd".into(),
        display_name: "Native Protocol Test Fixture".into(),
        version: version.into(),
        capabilities: caps.iter().map(|s| (*s).into()).collect(),
        packages: vec![EnginePackage {
            target: EngineTarget::current(),
            url: "https://example.invalid/native-test-engine".into(),
            sha256: sha256_file(binary).unwrap(),
            archive: ArchiveKind::Raw,
            entrypoint: if cfg!(windows) {
                "runtime/fixture.exe"
            } else {
                "runtime/fixture"
            }
            .into(),
            args: std::iter::once(mode.to_owned())
                .chain(extra.iter().cloned())
                .collect(),
        }],
    };
    install(root, &manifest, binary);
}
fn input(root: &TempRoot) -> PathBuf {
    let input = root.0.join("设计 file ; dollar$.psd");
    fs::write(&input, b"fixture input must remain unchanged").unwrap();
    input
}
fn inspect(root: &TempRoot, file: &Path) -> std::process::Output {
    run(
        &root.0,
        &["psd", "inspect", file.to_str().unwrap(), "--json"],
    )
}
fn psd_caps(root: &TempRoot) -> Vec<String> {
    success(run(&root.0, &["capabilities", "--json"]))["result"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| {
            item["id"]
                .as_str()
                .filter(|id| id.starts_with("psd."))
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn four_readonly_commands_round_trip_with_engine_identity_and_no_input_changes() {
    let root = TempRoot::new("readonly");
    let file = input(&root);
    let before = sha256_file(&file).unwrap();
    provision(&root, "environment", "1.0", &CAPS);
    assert!(psd_caps(&root).is_empty());
    error(inspect(&root, &file), 3, "ENGINE_UNAVAILABLE");
    activate(&root.0, "1.0");
    assert_eq!(psd_caps(&root), CAPS);
    for (args, capability) in [
        (
            vec!["psd", "inspect", file.to_str().unwrap()],
            "psd.inspect",
        ),
        (vec!["psd", "tree", file.to_str().unwrap()], "psd.tree"),
        (
            vec!["psd", "layer", "list", file.to_str().unwrap()],
            "psd.layer.list",
        ),
        (
            vec![
                "psd",
                "layer",
                "info",
                file.to_str().unwrap(),
                "--id",
                "L0001",
            ],
            "psd.layer.info",
        ),
    ] {
        let mut args = args;
        args.extend(["--engine", "ag-psd", "--json"]);
        let value = success(run(&root.0, &args));
        assert_eq!(value["schema_version"], "1");
        assert_eq!(value["operation"], capability);
        assert_eq!(value["engine"]["id"], "ag-psd");
        assert_eq!(value["engine"]["provider"], "managed");
        assert_eq!(value["engine"]["version"], "1.0");
        assert_eq!(value["result"]["contract_version"], "1");
        assert_eq!(value["warnings"][0], "fixture warning");
    }
    let human = run(&root.0, &["psd", "tree", file.to_str().unwrap()]);
    assert!(human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).contains("L0001"));
    assert!(String::from_utf8_lossy(&human.stderr).contains("fixture warning"));
    assert_eq!(sha256_file(&file).unwrap(), before);
    success(run(&root.0, &["engine", "deactivate", "ag-psd", "--json"]));
    assert!(psd_caps(&root).is_empty());
    error(inspect(&root, &file), 3, "ENGINE_UNAVAILABLE");
    success(run(
        &root.0,
        &["engine", "remove", "ag-psd", "1.0", "--json"],
    ));
}

#[test]
fn active_version_capabilities_are_not_the_inventory_union() {
    let root = TempRoot::new("active-caps");
    let file = input(&root);
    provision(&root, "ok", "1.0", &["psd.inspect"]);
    provision(&root, "ok", "2.0", &CAPS);
    activate(&root.0, "1.0");
    assert_eq!(psd_caps(&root), ["psd.inspect"]);
    let value = error(
        run(&root.0, &["psd", "tree", file.to_str().unwrap(), "--json"]),
        3,
        "ENGINE_INCOMPATIBLE",
    );
    assert_eq!(value["engine"]["version"], "1.0");
    activate(&root.0, "2.0");
    assert_eq!(psd_caps(&root), CAPS);
}

#[test]
fn missing_engine_input_and_explicit_alternate_do_not_install_or_fall_back() {
    let root = TempRoot::new("missing");
    let file = input(&root);
    error(inspect(&root, &file), 3, "ENGINE_UNAVAILABLE");
    error(
        run(
            &root.0,
            &[
                "psd",
                "inspect",
                file.to_str().unwrap(),
                "--engine",
                "psd-tools",
                "--json",
            ],
        ),
        3,
        "ENGINE_UNAVAILABLE",
    );
    error(
        inspect(&root, &root.0.join("missing.psd")),
        2,
        "INVALID_INPUT",
    );
    error(inspect(&root, &root.0), 2, "INVALID_INPUT");
    assert!(!root.0.join("engines").exists());
    assert!(!root.0.join("state").exists());
}

#[test]
fn invalid_layer_ids_and_argument_errors_are_json() {
    let root = TempRoot::new("arguments");
    let file = input(&root);
    for id in ["L0000", "L01", "L00001", "same", "L-001"] {
        error(
            run(
                &root.0,
                &[
                    "psd",
                    "layer",
                    "info",
                    file.to_str().unwrap(),
                    "--id",
                    id,
                    "--json",
                ],
            ),
            2,
            "INVALID_ARGUMENT",
        );
    }
    error(
        run(
            &root.0,
            &["psd", "layer", "info", file.to_str().unwrap(), "--json"],
        ),
        2,
        "INVALID_ARGUMENT",
    );
    for seconds in ["0", "3601", "invalid"] {
        error(
            run(
                &root.0,
                &[
                    "psd",
                    "inspect",
                    file.to_str().unwrap(),
                    "--timeout-secs",
                    seconds,
                    "--json",
                ],
            ),
            2,
            "INVALID_ARGUMENT",
        );
    }
    let help = run(&root.0, &["psd", "--help", "--json"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("inspect"));
    error(
        run(
            &root.0,
            &["psd", "layer", "export", file.to_str().unwrap(), "--json"],
        ),
        2,
        "INVALID_ARGUMENT",
    );
}

#[test]
fn protocol_failures_and_business_errors_preserve_stable_exit_codes() {
    for (mode, exit, code) in [
        ("garbage", 1, "EXECUTION_FAILED"),
        ("multiple-json", 1, "EXECUTION_FAILED"),
        ("invalid-utf8", 1, "EXECUTION_FAILED"),
        ("wrong-id", 1, "EXECUTION_FAILED"),
        ("protocol-v2", 1, "EXECUTION_FAILED"),
        ("contract-v2", 1, "EXECUTION_FAILED"),
        ("bad-schema", 1, "EXECUTION_FAILED"),
        ("nonzero", 1, "EXECUTION_FAILED"),
        ("stdout-limit", 1, "EXECUTION_FAILED"),
        ("stderr-limit", 1, "EXECUTION_FAILED"),
        ("invalid-input", 2, "INVALID_INPUT"),
        ("invalid-argument", 2, "INVALID_ARGUMENT"),
        ("unsupported", 3, "UNSUPPORTED_CAPABILITY"),
    ] {
        let root = TempRoot::new(mode);
        let file = input(&root);
        provision(&root, mode, "1.0", &CAPS);
        activate(&root.0, "1.0");
        let value = error(inspect(&root, &file), exit, code);
        assert_eq!(value["engine"]["id"], "ag-psd", "mode={mode}");
        assert_eq!(value["engine"]["version"], "1.0");
    }
}

#[test]
fn layer_info_must_echo_the_selected_layer_id() {
    let root = TempRoot::new("wrong-layer");
    let file = input(&root);
    provision(&root, "wrong-layer", "1.0", &CAPS);
    activate(&root.0, "1.0");
    error(
        run(
            &root.0,
            &[
                "psd",
                "layer",
                "info",
                file.to_str().unwrap(),
                "--id",
                "L0001",
                "--json",
            ],
        ),
        1,
        "EXECUTION_FAILED",
    );
}

#[test]
fn timeout_covers_silent_engines_and_inherited_output_pipes() {
    for mode in ["sleep", "inherited-pipes"] {
        let root = TempRoot::new(mode);
        let file = input(&root);
        provision(&root, mode, "1.0", &CAPS);
        activate(&root.0, "1.0");
        let started = Instant::now();
        let value = error(
            run(
                &root.0,
                &[
                    "psd",
                    "inspect",
                    file.to_str().unwrap(),
                    "--timeout-secs",
                    "1",
                    "--json",
                ],
            ),
            1,
            "EXECUTION_FAILED",
        );
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("timed out")
        );
        let message = value["error"]["message"].as_str().unwrap();
        for field in [
            "transport diagnostic:",
            "spawn_ms=",
            "child_exited=",
            "stdin_complete=",
            "stdout_complete=",
            "stderr_complete=",
        ] {
            assert!(message.contains(field), "missing {field}: {message}");
        }
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "cleanup exceeded timeout budget for {mode}"
        );
    }
}

#[test]
fn broken_active_entrypoint_is_unavailable_and_not_advertised() {
    let root = TempRoot::new("broken");
    let file = input(&root);
    provision(&root, "ok", "1.0", &CAPS);
    activate(&root.0, "1.0");
    let bin = if cfg!(windows) {
        "runtime/fixture.exe"
    } else {
        "runtime/fixture"
    };
    fs::remove_file(root.0.join("engines/ag-psd/1.0").join(bin)).unwrap();
    error(inspect(&root, &file), 3, "ENGINE_UNAVAILABLE");
    assert!(psd_caps(&root).is_empty());
}

#[test]
fn relative_input_and_data_home_are_resolved_before_engine_cwd() {
    let root = TempRoot::new("relative");
    provision(&root, "environment", "1.0", &CAPS);
    activate(&root.0, "1.0");
    let file = input(&root);
    let output = Command::new(env!("CARGO_BIN_EXE_yu"))
        .current_dir(&root.0)
        .args(["psd", "inspect"])
        .arg(file.file_name().unwrap())
        .arg("--json")
        .env("YU_DATA_HOME", ".")
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(success(output)["engine"]["id"], "ag-psd");
}

#[cfg(unix)]
#[test]
fn non_utf8_input_path_is_rejected_without_lossy_conversion() {
    use std::os::unix::ffi::OsStringExt;
    let root = TempRoot::new("non-utf8");
    let file = root.0.join(std::ffi::OsString::from_vec(vec![b'a', 255]));
    // Some Unix filesystems reject non-UTF-8 names. A real lossy alias must not be used instead.
    fs::write(file.to_string_lossy().as_ref(), b"fixture").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_yu"))
        .args(["psd", "inspect"])
        .arg(file)
        .arg("--json")
        .env("YU_DATA_HOME", &root.0)
        .output()
        .unwrap();
    error(output, 2, "INVALID_INPUT");
}
