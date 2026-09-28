//! Controlled external file occupancy is not a reproduction of an unknown historical holder.
use super::*;
use std::os::windows::fs::OpenOptionsExt;

fn capture_preserves_original_error(root: &TempRoot, version: &Path, original: &serde_json::Value) {
    let Some(directory) = std::env::var_os("YU_TEST_FORENSICS_DIR") else {
        return; // Ordinary CI has no collector; the package control explicitly enables it.
    };
    assert!(std::env::var_os("YU_TEST_FORENSICS_PYTHON").is_some());
    let mut matches = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let context = entry.unwrap().path().join("context.json");
        if !context.is_file() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&context).unwrap()).unwrap();
        if value["version_directory"].as_str().map(Path::new) == Some(version) {
            matches.push((context, value));
        }
    }
    assert_eq!(
        matches.len(),
        1,
        "enabled collection must retain this exact failed invocation"
    );
    let (context, value) = &matches[0];
    assert_eq!(value["original_error"], *original);
    assert_eq!(value["exit_code"], 1);
    assert_eq!(value["failure_retried"], false);
    assert_eq!(value["root_cause_fixed"], false);
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(context.parent().unwrap().join("snapshot/report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["status"], "collected", "{report}");
    assert!(root.0.join("engines/ag-psd/1.0").is_dir());
}

#[test]
fn held_nested_file_preserves_installation_and_other_active_version_until_explicit_release() {
    let root = TempRoot::new("controlled-nested-file-removal");
    let source = input(&root);
    provision(&root, "ok", "1.0", &CAPS);
    provision(&root, "ok", "2.0", &CAPS);
    activate(&root.0, "2.0"); // No engine executes anywhere in this control.
    let version = root.0.join("engines/ag-psd/1.0");
    let metadata = version.join(".yu-install.json");
    let binary = version.join("runtime/fixture.exe");
    let other_metadata = root.0.join("engines/ag-psd/2.0/.yu-install.json");
    let before = [&metadata, &binary, &other_metadata, &source].map(|p| sha256_file(p).unwrap());
    // Hold only a nested regular file, NOT the directory or a running engine image.
    // Documented READ | WRITE sharing without DELETE; this is a test-owned handle.
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2)
        .open(&binary)
        .unwrap();
    let denied = error(
        run(&root.0, &["engine", "remove", "ag-psd", "1.0", "--json"]),
        1,
        "EXECUTION_FAILED",
    );
    let message = denied["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("cannot quarantine")
            && message.contains("original version was not deleted"),
        "{message}"
    );
    assert!(
        [5, 32, 33]
            .iter()
            .any(|code| message.contains(&format!("os_error=Some({code})"))),
        "{message}"
    );
    assert!(version.is_dir());
    assert_eq!(
        [&metadata, &binary, &other_metadata, &source].map(|p| sha256_file(p).unwrap()),
        before
    );
    let info = success(run(&root.0, &["engine", "info", "ag-psd", "--json"]));
    assert_eq!(info["result"][0]["active_version"], "2.0");
    assert_eq!(
        info["result"][0]["installed_versions"],
        serde_json::json!(["1.0", "2.0"])
    );
    assert_eq!(psd_caps(&root), CAPS);
    capture_preserves_original_error(&root, &version, &denied);
    // Collection and failure handling must not unlock or delete the held test resource.
    let still_held = fs::rename(&binary, version.join("runtime/moved.exe"));
    assert!(
        still_held.is_err(),
        "collector must leave the holder intact"
    );
    drop(held); // The fixture explicitly changes the precondition before a NEW CLI invocation.
    let removed = success(run(
        &root.0,
        &["engine", "remove", "ag-psd", "1.0", "--json"],
    ));
    assert_eq!(removed["result"]["cleanup_complete"], true);
    assert!(!version.exists());
    assert!(root.0.join("engines/ag-psd/2.0").is_dir());
    assert_eq!(sha256_file(&other_metadata).unwrap(), before[2]);
    assert_eq!(sha256_file(&source).unwrap(), before[3]);
    let info = success(run(&root.0, &["engine", "info", "ag-psd", "--json"]));
    assert_eq!(info["result"][0]["active_version"], "2.0");
    assert_eq!(
        info["result"][0]["installed_versions"],
        serde_json::json!(["2.0"])
    );
    println!(
        "{}",
        serde_json::json!({
            "schema_version":"1", "kind":"controlled_nested_file_removal",
            "holder_pid":std::process::id(), "holder_test_owned":true, "engine_executed":false,
            "original_error":denied, "installation_preserved":true, "other_active_version_preserved":true,
            "released_by_fixture":true, "explicit_removal_after_release":true,
            "failed_case_retries":0, "historical_failure_attributed":false, "public_release_ready":false
        })
    );
}
