//! Test-only post-failure observation. Never changes the captured CLI result or retries it.
use super::*;

pub(super) fn capture(root: &Path, args: &[&str], output: &Output) {
    if output.status.success() || args.len() < 4 || args[..2] != ["engine", "remove"] {
        return;
    }
    let (Some(directory), Some(python)) = (
        std::env::var_os("YU_TEST_FORENSICS_DIR"),
        std::env::var_os("YU_TEST_FORENSICS_PYTHON"),
    ) else {
        return;
    };
    let directory = PathBuf::from(directory);
    let python = PathBuf::from(python);
    if !directory.is_absolute() || !python.is_absolute() {
        return;
    }
    // Inputs here come from test-owned commands; refuse path-like selectors defensively.
    if args[2..4]
        .iter()
        .any(|s| s.is_empty() || s.contains(['/', '\\']) || *s == ".." || *s == ".")
    {
        return;
    }
    let finished = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let case = directory.join(format!(
        "occupancy-{}-{finished}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if fs::create_dir(&case).is_err() {
        return;
    }
    let version = root.join("engines").join(args[2]).join(args[3]);
    let context = serde_json::json!({
        "schema_version":"1", "test_pid":std::process::id(), "version_directory":version,
        "command_finished_unix_ns":finished.to_string(), "exit_code":output.status.code(),
        "original_error":serde_json::from_slice::<Value>(&output.stderr).ok(),
        "failure_retried":false, "root_cause_fixed":false,
    });
    if let Ok(bytes) = serde_json::to_vec_pretty(&context) {
        let _ = fs::write(case.join("context.json"), bytes);
    }
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/collect_windows_occupancy.py");
    let mut command = Command::new(python);
    command
        .args(["-B"])
        .arg(script)
        .arg("--version-dir")
        .arg(&version)
        .arg("--output-dir")
        .arg(case.join("snapshot"));
    if let Some(traces) = std::env::var_os("YU_WINDOWS_LIFECYCLE_TRACE_DIR") {
        command.arg("--traces-dir").arg(traces);
    }
    // The trusted collector supervises a native-query worker with its own 10s budget.
    // This runs before assertion unwinding/TempRoot teardown, after the CLI has exited.
    let status = command.status();
    eprintln!(
        "YU_OCCUPANCY_CAPTURE path={case:?} status={status:?}; original command result unchanged"
    );
}
