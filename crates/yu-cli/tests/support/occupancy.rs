//! Test-only post-failure observation. Never changes the captured CLI result or retries it.
use super::*;
use std::{
    process::Child,
    thread,
    time::{Duration, Instant},
};

pub(super) struct RemoveWindowSampler {
    child: Child,
    case: PathBuf,
    stop: PathBuf,
    command_started_unix_ns: u128,
}

fn valid_remove_selector(args: &[&str]) -> bool {
    args.len() >= 4
        && args[..2] == ["engine", "remove"]
        && !args[2..4]
            .iter()
            .any(|s| s.is_empty() || s.contains(['/', '\\']) || *s == ".." || *s == ".")
}

pub(super) fn start_remove_window(root: &Path, args: &[&str]) -> Option<RemoveWindowSampler> {
    if !valid_remove_selector(args) {
        return None;
    }
    let (engine, version_name) = (args[2], args[3]);
    let (Some(directory), Some(python)) = (
        std::env::var_os("YU_TEST_REMOVE_WINDOW_SAMPLING_DIR"),
        std::env::var_os("YU_TEST_FORENSICS_PYTHON"),
    ) else {
        return None;
    };
    let directory = PathBuf::from(directory);
    let python = PathBuf::from(python);
    if !directory.is_absolute() || !python.is_absolute() {
        return None;
    }
    let version = root.join("engines").join(engine).join(version_name);
    if !version.is_dir() {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let case = directory.join(format!(
        "remove-window-{}-{now}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if fs::create_dir(&case).is_err() {
        return None;
    }
    let stop = case.join("stop");
    let ready = case.join("ready");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/windows_remove_window_sampler.py");
    let mut child = Command::new(python)
        .args(["-B"])
        .arg(script)
        .arg("--version-dir")
        .arg(&version)
        .arg("--output-dir")
        .arg(case.join("sampling"))
        .arg("--stop-file")
        .arg(&stop)
        .arg("--ready-file")
        .arg(&ready)
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready.is_file() {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) => {}
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(5));
    }
    Some(RemoveWindowSampler {
        child,
        case,
        stop,
        command_started_unix_ns: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    })
}

fn message_number(message: &str, field: &str) -> Option<u64> {
    let tail = message.split(field).nth(1)?;
    let digits = tail
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

pub(super) fn finish_remove_window(mut sampler: RemoveWindowSampler, output: &Output) {
    let finished = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let _ = fs::write(&sampler.stop, b"stop\n");
    let deadline = Instant::now() + Duration::from_secs(3);
    let sampler_exit = loop {
        match sampler.child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = sampler.child.kill();
                break sampler.child.wait().ok().and_then(|status| status.code());
            }
            Err(_) => break None,
        }
    };
    let envelope = serde_json::from_slice::<Value>(if output.status.success() {
        &output.stdout
    } else {
        &output.stderr
    })
    .ok();
    let quarantine = envelope
        .as_ref()
        .and_then(|value| value.get("result"))
        .and_then(|value| value.get("quarantine"))
        .cloned();
    let message = envelope
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str);
    let normalized = if let Some(value) = quarantine {
        value
    } else {
        serde_json::json!({
            "attempts": message.and_then(|text| message_number(text, "attempts=")),
            "elapsed_ms": message.and_then(|text| message_number(text, "elapsed_ms=")),
        })
    };
    let sampling_report = fs::read(sampler.case.join("sampling/sampling-report.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let report = serde_json::json!({
        "schema_version":"1", "kind":"remove_window_observation",
        "command_started_unix_ns":sampler.command_started_unix_ns.to_string(),
        "command_finished_unix_ns":finished.to_string(),
        "exit_code":output.status.code(), "command_succeeded":output.status.success(),
        "remove_envelope":envelope, "quarantine":normalized,
        "sampler_exit_code":sampler_exit, "sampling":sampling_report,
        "failure_retried":false, "rename_blocker_proven":false,
        "root_cause_fixed":false, "public_release_ready":false,
        "limits":"test-only concurrent observation perturbs timing; directory use is not proof of the rename-blocking handle"
    });
    if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
        let _ = fs::write(sampler.case.join("report.json"), bytes);
    }
}

fn normalized_windows_path(path: &Path) -> String {
    // The Windows runner may expose the same temp directory through an 8.3 alias
    // (for example RUNNER~1) while the native sampler reports its long path.
    // Canonicalize existing test-owned paths before textual normalization so the
    // assertion compares the directory identity rather than the spelling used by
    // the caller. Failed remove controls deliberately keep this directory present.
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = canonical
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
}

pub(super) fn assert_remove_window_user(version: &Path, pid: u32, command_succeeded: bool) {
    let Some(directory) = std::env::var_os("YU_TEST_REMOVE_WINDOW_SAMPLING_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory);
    let mut matches = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let report = entry.unwrap().path().join("report.json");
        if !report.is_file() {
            continue;
        }
        let value: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        let observed_version = value["sampling"]["version_directory"]
            .as_str()
            .map(Path::new)
            .map(normalized_windows_path);
        if observed_version.as_deref() == Some(&normalized_windows_path(version))
            && value["command_succeeded"] == command_succeeded
        {
            matches.push(value);
        }
    }
    assert_eq!(
        matches.len(),
        1,
        "expected one remove-window report for {version:?}"
    );
    let value = &matches[0];
    assert_eq!(value["sampler_exit_code"], 0, "{value}");
    assert_eq!(value["rename_blocker_proven"], false);
    let identities = value["sampling"]["identities"].as_array().unwrap();
    assert!(
        identities
            .iter()
            .any(|identity| { identity["pid"] == pid && identity["identity_confirmed"] == true }),
        "expected PID {pid} in remove-window timeline: {value}"
    );
}

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
    if !valid_remove_selector(args) {
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
