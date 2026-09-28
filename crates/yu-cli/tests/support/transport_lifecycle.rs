//! Diagnostic reproductions; no relaxed deadline, failed-case retry, or external unlocker.
use super::*;

fn remove_inactive(root: &TempRoot) {
    success(run(&root.0, &["engine", "deactivate", "ag-psd", "--json"]));
    let value = success(run(
        &root.0,
        &["engine", "remove", "ag-psd", "1.0", "--json"],
    ));
    assert_eq!(value["result"]["cleanup_complete"], true);
    assert!(!root.0.join("engines/ag-psd/1.0").exists());
}

fn timeout_case(mode: &str, phase: &str, exited: bool) {
    let root = TempRoot::new(mode);
    let file = input(&root);
    let hash = sha256_file(&file).unwrap();
    provision(&root, mode, "1.0", &CAPS);
    activate(&root.0, "1.0");
    // Establish native-fixture startup separately, once, before the strict 1s
    // semantic probe. This is not a retry and is not used by real-package cycles.
    let binary = root
        .0
        .join("engines/ag-psd/1.0/runtime")
        .join(if cfg!(windows) {
            "fixture.exe"
        } else {
            "fixture"
        });
    let ready = Command::new(binary)
        .arg("lifecycle-ready")
        .output()
        .unwrap();
    assert!(ready.status.success());
    assert_eq!(ready.stdout, b"fixture-ready");
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
    let message = value["error"]["message"].as_str().unwrap();
    assert!(message.contains("timed out after 1000 ms"), "{message}");
    assert!(message.contains(&format!("phase={phase}")), "{message}");
    assert!(
        message.contains(&format!("child_exited={exited}")),
        "{message}"
    );
    let bytes: usize = message
        .split("stdout_bytes=")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        bytes > 0,
        "fixture must have emitted a response before timeout: {message}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "owned cleanup exceeded budget: {message}"
    );
    assert_eq!(sha256_file(&file).unwrap(), hash);
    remove_inactive(&root);
}

#[test]
fn complete_json_does_not_replace_process_exit() {
    timeout_case("lifecycle-live-parent", "process_exit", false);
}

#[test]
fn exited_parent_does_not_replace_inherited_pipe_eof() {
    timeout_case("lifecycle-pipe-descendant", "pipe_eof", true);
}

#[test]
fn fragmented_response_is_not_mistaken_for_eof() {
    let root = TempRoot::new("lifecycle-chunks");
    let file = input(&root);
    provision(&root, "lifecycle-chunks", "1.0", &CAPS);
    activate(&root.0, "1.0");
    let value = success(inspect(&root, &file));
    assert_eq!(value["result"]["document"]["width"], 2);
    remove_inactive(&root);
}

#[cfg(windows)]
fn assert_owned_trace(root: &TempRoot) {
    let Some(directory) = std::env::var_os("YU_WINDOWS_LIFECYCLE_TRACE_DIR") else {
        return;
    };
    let version = fs::canonicalize(root.0.join("engines/ag-psd/1.0")).unwrap();
    let mut records = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let Ok(record) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        if record["version_directory"].as_str().map(Path::new) == Some(version.as_path()) {
            records.push(record);
        }
    }
    let end = records
        .iter()
        .find(|v| v["stage"] == "cleanup_end")
        .expect("enabled trace must observe cleanup end");
    assert_eq!(end["observations"]["direct_wait_succeeded"], true);
    assert_eq!(end["observations"]["workers_joined"], 0);
    assert_eq!(end["observations"]["io_transport"], "nonblocking_poll");
    assert_eq!(end["observations"]["io_endpoints_closed"], true);
    assert_eq!(end["observations"]["job_empty_confirmed"], true);
    assert_eq!(end["observations"]["cleanup_succeeded"], true);
    assert_eq!(end["observations"]["cleanup_errors"], serde_json::json!([]));
    assert_eq!(end["job"]["status"], "observed");
    assert!(end["child_created_filetime"].is_string());
    for stage in ["started", "cleanup_begin"] {
        assert!(
            records
                .iter()
                .any(|v| v["stage"] == stage && v["invocation"] == end["invocation"])
        );
    }
    // ActiveProcesses is sampled while native handles are retained, not a leak assertion.
}

#[test]
fn quiet_descendant_does_not_prevent_immediate_engine_removal() {
    let root = TempRoot::new("lifecycle-quiet-descendant");
    let file = input(&root);
    provision(&root, "lifecycle-quiet-descendant", "1.0", &CAPS);
    activate(&root.0, "1.0");
    success(inspect(&root, &file));
    #[cfg(windows)]
    assert_owned_trace(&root);
    assert!(
        root.0
            .join("engines/ag-psd/1.0/.yu-lifecycle-child-ready")
            .is_file()
    );
    remove_inactive(&root);
}

#[cfg(windows)]
#[test]
fn external_directory_occupancy_fails_closed_then_recovers_after_release() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = TempRoot::new("lifecycle-held-directory");
    provision(&root, "ok", "1.0", &CAPS); // Inactive; no engine execution.
    let version = root.0.join("engines/ag-psd/1.0");
    let metadata = version.join(".yu-install.json");
    let before = sha256_file(&metadata).unwrap();
    // Documented FILE_SHARE_READ | FILE_SHARE_WRITE; omit FILE_SHARE_DELETE.
    // FILE_FLAG_BACKUP_SEMANTICS permits opening this test-owned directory.
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x00000001 | 0x00000002)
        .custom_flags(0x02000000)
        .open(&version)
        .unwrap();
    let value = error(
        run(&root.0, &["engine", "remove", "ag-psd", "1.0", "--json"]),
        1,
        "EXECUTION_FAILED",
    );
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("quarantine diagnostic:")
    );
    assert!(version.is_dir());
    assert_eq!(sha256_file(&metadata).unwrap(), before);
    drop(held); // Release only the handle created by THIS test, never external handles.
    let removed = success(run(
        &root.0,
        &["engine", "remove", "ag-psd", "1.0", "--json"],
    ));
    assert_eq!(removed["result"]["cleanup_complete"], true);
    assert!(!version.exists());
}
