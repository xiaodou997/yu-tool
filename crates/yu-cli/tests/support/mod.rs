#![allow(dead_code)]
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use yu_engine_manager::{
    Downloader, EngineInstaller, EngineManager, EngineManifest, EngineTarget, ManagedLayout,
    ManagerError,
};
static NEXT: AtomicU64 = AtomicU64::new(1);

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

struct LocalDownloader {
    path: PathBuf,
    url: String,
}
impl Downloader for LocalDownloader {
    fn download(&self, url: &str, destination: &Path) -> Result<u64, ManagerError> {
        if url != self.url {
            return Err(ManagerError::Download("unexpected test URL".into()));
        }
        fs::copy(&self.path, destination).map_err(|e| ManagerError::Io(e.to_string()))
    }
}
pub fn install(root: &TempRoot, manifest: &EngineManifest, archive: &Path) {
    let package = manifest.package_for(&EngineTarget::current()).unwrap();
    EngineInstaller::new(
        root.manager(),
        LocalDownloader {
            path: archive.to_owned(),
            url: package.url.clone(),
        },
    )
    .install(manifest)
    .expect("test package must pass real installer verification");
}

pub fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yu"))
        .args(args)
        .env("YU_DATA_HOME", root)
        .env("PATH", "")
        .env("NODE_OPTIONS", "--require=should-not-be-loaded-by-yu")
        .env("NODE_PATH", "should-not-be-used-by-yu")
        .output()
        .expect("CLI must execute without PATH lookup")
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
