#![allow(dead_code)]
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use yu_engine_manager::{EngineManager, EngineManifest, EngineTarget, ManagedLayout};
static NEXT: AtomicU64 = AtomicU64::new(1);

#[cfg(windows)]
mod occupancy;

#[cfg(windows)]
pub fn assert_remove_window_user(version: &Path, pid: u32, command_succeeded: bool) {
    occupancy::assert_remove_window_user(version, pid, command_succeeded);
}

#[cfg(windows)]
pub fn assert_remove_file_window_user(version: &Path, pid: u32, relative_path: &str) {
    occupancy::assert_remove_file_window_user(version, pid, relative_path);
}

pub struct TempRoot(pub PathBuf);
impl TempRoot {
    pub fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "yu-psd-{label}-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    pub fn manager(&self) -> EngineManager {
        EngineManager::new(ManagedLayout::new(&self.0), EngineTarget::current())
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn install(root: &TempRoot, manifest: &EngineManifest, archive: &Path) {
    let path = root.0.join("install-manifest.json");
    fs::write(&path, serde_json::to_vec(manifest).unwrap()).unwrap();
    success(run(
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
    ));
}

pub fn cli_binary() -> PathBuf {
    let path = std::env::var_os("YU_TEST_CLI")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_yu")));
    fs::canonicalize(path).expect("test CLI must exist")
}

pub fn run(root: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(cli_binary());
    command
        .args(args)
        .env("YU_DATA_HOME", root)
        .env("PATH", "")
        .env("NODE_OPTIONS", "--require=should-not-be-loaded-by-yu")
        .env("NODE_PATH", "should-not-be-used-by-yu");
    #[cfg(windows)]
    let sampler = occupancy::start_remove_window(root, args);
    let output = command
        .output()
        .expect("CLI must execute without PATH lookup");
    #[cfg(windows)]
    if let Some(sampler) = sampler {
        occupancy::finish_remove_window(sampler, &output);
    }
    #[cfg(windows)]
    occupancy::capture(root, args, &output);
    output
}
pub fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("single JSON success envelope")
}
pub fn error(output: Output, exit: i32, code: &str) -> Value {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).expect("single JSON error envelope");
    assert_eq!(value["schema_version"], "1");
    assert_eq!(value["error"]["code"], code);
    value
}
pub fn activate(root: &Path, version: &str) {
    success(run(
        root,
        &["engine", "activate", "ag-psd", version, "--json"],
    ));
}
