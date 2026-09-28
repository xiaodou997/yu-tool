//! Known-owner startup controls. All processes, Jobs, paths and handles are test-owned.
use super::*;
use crate::process::{Running, exchange};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
    System::{
        JobObjects::{AssignProcessToJobObject, IsProcessInJob, JOB_OBJECT_QUERY, OpenJobObjectW},
        Threading::GetCurrentProcess,
    },
};
const CONFIG: &str = ".yu-startup-fixture.json";
const CHILD_TEST: &str = "process::windows_spawn::tests::fixture_child";
const DESCENDANT_TEST: &str = "process::windows_spawn::tests::fixture_descendant";

fn query_job(name: &str) -> OwnedHandle {
    let name = wide_z(OsStr::new(name)).unwrap();
    // SAFETY: opens only a uniquely named Job created by this test; query rights only.
    let handle = unsafe { OpenJobObjectW(JOB_OBJECT_QUERY, 0, name.as_ptr()) };
    assert!(!handle.is_null(), "{}", io::Error::last_os_error());
    // SAFETY: successful OpenJobObjectW transfers an owned handle reference.
    unsafe { OwnedHandle::from_raw_handle(handle) }
}
fn in_job(process: HANDLE, job: &OwnedHandle) -> bool {
    let mut yes = 0;
    // SAFETY: process is our live child/current-process handle and Job is owned and live.
    assert_ne!(
        unsafe { IsProcessInJob(process, job.as_raw_handle(), &mut yes) },
        0
    );
    yes != 0
}
fn file_identity(handle: HANDLE) -> Option<(u32, u32, u32)> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: metadata-only query of a fixture-provided handle value; Windows validates it.
    // Never close, duplicate or mutate this candidate handle in the child.
    (unsafe { GetFileInformationByHandle(handle, &mut info) } != 0).then_some((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}
fn report(prefix: &str, value: Value) {
    fs::write(format!("{prefix}.tmp"), serde_json::to_vec(&value).unwrap()).unwrap();
    fs::rename(format!("{prefix}.tmp"), format!("{prefix}.json")).unwrap();
}
fn config() -> Option<Value> {
    if !std::env::args().any(|arg| arg == "--exact") || !Path::new(CONFIG).is_file() {
        return None;
    }
    Some(serde_json::from_slice(&fs::read(CONFIG).unwrap()).unwrap())
}

#[test]
fn fixture_child() {
    let Some(config) = config() else {
        return;
    };
    let job = query_job(config["job"].as_str().unwrap());
    // Query membership at the first fixture operation, before reading any engine request.
    let assigned = in_job(unsafe { GetCurrentProcess() }, &job);
    let unexpected_file = config["handle"].as_u64().is_some_and(|raw| {
        let seen = file_identity(raw as usize as HANDLE);
        seen.is_some() && serde_json::to_value(seen.unwrap()).unwrap() == config["identity"]
    });
    let descendant = if config["descendant"] == true {
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", DESCENDANT_TEST, "--nocapture"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Some(child.id()) // The invocation Job, not this fixture, owns descendant termination.
    } else {
        None
    };
    report(
        "parent",
        json!({"pid":std::process::id(), "in_expected_job":assigned,
        "descendant":descendant, "args":std::env::args().collect::<Vec<_>>(),
        "cwd":std::env::current_dir().unwrap(), "unexpected_file":unexpected_file,
        "node_options":std::env::var_os("NODE_OPTIONS").is_some(), "node_path":std::env::var_os("NODE_PATH").is_some()}),
    );
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes).unwrap();
    println!("startup-fixture-eof:{}", bytes.len());
}

#[test]
fn fixture_descendant() {
    let Some(config) = config() else {
        return;
    };
    let job = query_job(config["job"].as_str().unwrap());
    report(
        "descendant",
        json!({"pid":std::process::id(),
        "in_expected_job":in_job(unsafe { GetCurrentProcess() }, &job)}),
    );
    thread::sleep(Duration::from_secs(20)); // Finite even if owned cleanup regresses.
}

fn setup(root: &Path, descendant: bool) -> (ManagedEngineCommand, Containment, String) {
    let name = format!(
        "Local\\YuTool-startup-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let containment = Containment::create(Some(&name)).unwrap();
    fs::write(
        root.join(CONFIG),
        serde_json::to_vec(&json!({"job":name, "descendant":descendant})).unwrap(),
    )
    .unwrap();
    let installed = ManagedEngineCommand {
        engine_id: "ag-psd".into(),
        version: "startup-fixture".into(),
        capabilities: vec![],
        working_dir: fs::canonicalize(root).unwrap(),
        entrypoint: std::env::current_exe().unwrap(),
        args: vec!["--exact".into(), CHILD_TEST.into(), "--nocapture".into()],
    };
    (installed, containment, name)
}
fn wait_report(root: &Path, prefix: &str) -> Value {
    let started = Instant::now();
    let path = root.join(format!("{prefix}.json"));
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "fixture did not report {prefix}"
        );
        thread::sleep(Duration::from_millis(5));
    }
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn running(child: Child, containment: Containment) -> Running {
    Running {
        child,
        containment,
        workers: vec![],
        cleanup_result: None,
        trace: None,
    }
}
fn finish(running: &mut Running) {
    let result = exchange(
        running,
        b"{}".to_vec(),
        Duration::from_secs(30),
        Instant::now(),
        0,
    );
    let output = running.finish(result).unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("startup-fixture-eof:2"));
}

#[test]
fn creation_attributes_cover_parent_and_immediate_descendant_before_request() {
    let root = tempfile::tempdir().unwrap();
    let (installed, containment, _) = setup(root.path(), true);
    let child = spawn_in_job(&installed, &containment.job).unwrap();
    let mut owned = running(child, containment);
    let parent = wait_report(root.path(), "parent");
    let descendant = wait_report(root.path(), "descendant");
    assert_eq!(parent["in_expected_job"], true);
    assert_eq!(descendant["in_expected_job"], true);
    assert_eq!(parent["descendant"], descendant["pid"]);
    assert!(in_job(owned.child.as_raw_handle(), &owned.containment.job));
    finish(&mut owned);
}

#[test]
fn controlled_legacy_post_spawn_assignment_exposes_the_startup_gap() {
    let root = tempfile::tempdir().unwrap();
    let (installed, containment, _) = setup(root.path(), false);
    struct Legacy(std::process::Child);
    impl Drop for Legacy {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Legacy(
        Command::new(&installed.entrypoint)
            .args(&installed.args)
            .current_dir(&installed.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let observed = wait_report(root.path(), "parent");
    // Only the controlled old-order fixture uses post-spawn assignment; production never does.
    assert_eq!(observed["in_expected_job"], false);
    assert_ne!(
        unsafe {
            AssignProcessToJobObject(containment.job.as_raw_handle(), child.0.as_raw_handle())
        },
        0
    );
    assert!(in_job(child.0.as_raw_handle(), &containment.job));
    drop(child.0.stdin.take());
    assert!(child.0.wait().unwrap().success());
}

#[test]
fn job_without_assignment_rights_fails_before_fixture_code_and_has_no_fallback() {
    let root = tempfile::tempdir().unwrap();
    let (installed, containment, name) = setup(root.path(), false);
    let restricted = query_job(&name); // No JOB_OBJECT_ASSIGN_PROCESS permission.
    let error = match spawn_in_job(&installed, &restricted) {
        Ok(child) => {
            let mut owned = running(child, containment);
            let _ = owned.cleanup();
            panic!("query-only Job unexpectedly permitted creation");
        }
        Err(error) => error,
    };
    assert_eq!(error.raw_os_error(), Some(5), "{error}");
    assert!(!root.path().join("parent.json").exists());
    assert!(!root.path().join("descendant.json").exists());
    // Same fixture and full-rights Job work normally after the refused creation.
    let child = spawn_in_job(&installed, &containment.job).unwrap();
    let mut owned = running(child, containment);
    assert_eq!(wait_report(root.path(), "parent")["in_expected_job"], true);
    finish(&mut owned);
}

#[test]
fn explicit_argv_cwd_and_handle_allowlist_survive_real_child_startup() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("engine 中文 space");
    fs::create_dir(&root).unwrap();
    let (mut installed, containment, _) = setup(&root, false);
    let values = [
        "spaces here",
        "a\"quote",
        "slash\\\"quote",
        "trailing\\",
        "中文",
        "%PATH% & ! ^",
        "tab\tvalue",
    ];
    for value in values {
        installed.args.extend(["--skip".into(), value.into()]);
    }
    let sentinel = root.join("unrelated-handle.txt");
    fs::write(&sentinel, b"test-owned only").unwrap();
    let file = File::open(&sentinel).unwrap();
    assert_ne!(
        unsafe {
            SetHandleInformation(
                file.as_raw_handle(),
                HANDLE_FLAG_INHERIT,
                HANDLE_FLAG_INHERIT,
            )
        },
        0
    );
    let mut config: Value = serde_json::from_slice(&fs::read(root.join(CONFIG)).unwrap()).unwrap();
    config["handle"] = json!(file.as_raw_handle() as usize);
    config["identity"] = json!(file_identity(file.as_raw_handle()).unwrap());
    fs::write(root.join(CONFIG), serde_json::to_vec(&config).unwrap()).unwrap();
    let child = spawn_in_job(&installed, &containment.job).unwrap();
    assert_ne!(
        unsafe { SetHandleInformation(file.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) },
        0
    );
    let mut owned = running(child, containment);
    let observed = wait_report(&root, "parent");
    let actual: Vec<String> = serde_json::from_value(observed["args"].clone()).unwrap();
    assert_eq!(actual[1..], installed.args);
    assert_eq!(
        Path::new(observed["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        installed.working_dir
    );
    assert_eq!(observed["unexpected_file"], false);
    assert_eq!(observed["node_options"], false);
    assert_eq!(observed["node_path"], false);
    finish(&mut owned);
}
