mod discovery;
mod installer;
mod lifecycle;
mod local_archive;
mod lock;
mod quarantine;

pub use discovery::EngineInventoryEntry;
pub use installer::{
    DEFAULT_MAX_DOWNLOAD_BYTES, DEFAULT_MAX_EXTRACTED_BYTES, DEFAULT_MAX_EXTRACTED_FILES,
    Downloader, EngineInstaller, HttpDownloader, InstallLimits, InstallReceipt, sha256_file,
};
pub use lifecycle::{
    ActivationReceipt, DeactivationReceipt, INSTALL_METADATA_FILE, InstalledEngineMetadata,
    InstalledVersion, LIFECYCLE_SCHEMA_VERSION, ManagedEngineCommand, RemovalReceipt,
};
pub use local_archive::LocalArchiveDownloader;
pub use quarantine::QuarantineObservation;

use serde::{Deserialize, Serialize};
use std::{
    env,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use yu_engine_api::{EngineDescriptor, EngineProvider, EngineState};

pub const MANIFEST_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineTarget {
    pub os: String,
    pub arch: String,
}

impl EngineTarget {
    pub fn current() -> Self {
        Self {
            os: env::consts::OS.to_owned(),
            arch: env::consts::ARCH.to_owned(),
        }
    }

    pub fn new(os: impl Into<String>, arch: impl Into<String>) -> Self {
        Self {
            os: os.into(),
            arch: arch.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveKind {
    Raw,
    Zip,
    TarGz,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnginePackage {
    pub target: EngineTarget,
    pub url: String,
    pub sha256: String,
    pub archive: ArchiveKind,
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineManifest {
    pub schema_version: String,
    pub id: String,
    pub display_name: String,
    pub version: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub packages: Vec<EnginePackage>,
}

impl EngineManifest {
    pub fn validate(&self) -> Result<(), ManagerError> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(ManagerError::InvalidManifest(format!(
                "unsupported engine manifest schema version: {}",
                self.schema_version
            )));
        }

        validate_identifier("engine id", &self.id)?;

        if self.display_name.trim().is_empty() {
            return Err(ManagerError::InvalidManifest(
                "engine display_name must not be empty".to_owned(),
            ));
        }

        validate_version_segment(&self.version)?;

        if self.packages.is_empty() {
            return Err(ManagerError::InvalidManifest(
                "engine manifest must contain at least one package".to_owned(),
            ));
        }

        for package in &self.packages {
            if package.target.os.trim().is_empty() || package.target.arch.trim().is_empty() {
                return Err(ManagerError::InvalidManifest(
                    "package target os/arch must not be empty".to_owned(),
                ));
            }

            if !package.url.starts_with("https://") {
                return Err(ManagerError::InvalidManifest(
                    "package url must use https://".to_owned(),
                ));
            }

            if package.sha256.len() != 64
                || !package.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(ManagerError::InvalidManifest(
                    "package sha256 must be a 64-character hexadecimal digest".to_owned(),
                ));
            }

            validate_relative_entrypoint(&package.entrypoint)?;
            validate_package_args(&package.args)?;
        }

        for (index, package) in self.packages.iter().enumerate() {
            if self.packages[..index]
                .iter()
                .any(|other| other.target == package.target)
            {
                return Err(ManagerError::InvalidManifest(format!(
                    "duplicate package target: {}/{}",
                    package.target.os, package.target.arch
                )));
            }
        }

        Ok(())
    }

    pub fn package_for(&self, target: &EngineTarget) -> Option<&EnginePackage> {
        self.packages
            .iter()
            .find(|package| package.target == *target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedLayout {
    root: PathBuf,
}

impl ManagedLayout {
    pub fn discover() -> Result<Self, ManagerError> {
        Ok(Self::new(default_data_root()?))
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn engines_dir(&self) -> PathBuf {
        self.root.join("engines")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn engine_dir(&self, engine_id: &str) -> PathBuf {
        self.engines_dir().join(engine_id)
    }

    pub fn engine_version_dir(&self, engine_id: &str, version: &str) -> PathBuf {
        self.engine_dir(engine_id).join(version)
    }

    pub fn engine_entrypoint(&self, manifest: &EngineManifest, package: &EnginePackage) -> PathBuf {
        self.engine_version_dir(&manifest.id, &manifest.version)
            .join(&package.entrypoint)
    }

    pub fn ensure_base_dirs(&self) -> Result<(), ManagerError> {
        for path in [self.engines_dir(), self.cache_dir(), self.state_dir()] {
            fs::create_dir_all(&path).map_err(|error| {
                ManagerError::Io(format!("cannot create {}: {error}", path.display()))
            })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemProbe {
    pub descriptor: EngineDescriptor,
    pub executable: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct EngineManager {
    layout: ManagedLayout,
    target: EngineTarget,
}

impl EngineManager {
    pub fn discover() -> Result<Self, ManagerError> {
        Ok(Self::new(
            ManagedLayout::discover()?,
            EngineTarget::current(),
        ))
    }

    pub fn new(layout: ManagedLayout, target: EngineTarget) -> Self {
        Self { layout, target }
    }

    pub fn layout(&self) -> &ManagedLayout {
        &self.layout
    }

    pub fn target(&self) -> &EngineTarget {
        &self.target
    }

    pub fn managed_descriptor(
        &self,
        manifest: &EngineManifest,
    ) -> Result<EngineDescriptor, ManagerError> {
        manifest.validate()?;

        let Some(package) = manifest.package_for(&self.target) else {
            return Ok(EngineDescriptor {
                id: manifest.id.clone(),
                display_name: manifest.display_name.clone(),
                provider: EngineProvider::Managed,
                state: EngineState::Incompatible,
                version: Some(manifest.version.clone()),
                capabilities: manifest.capabilities.clone(),
            });
        };

        let version_dir = self
            .layout
            .engine_version_dir(&manifest.id, &manifest.version);
        let entrypoint = self.layout.engine_entrypoint(manifest, package);

        let state = if entrypoint.is_file() {
            EngineState::Ready
        } else if version_dir.exists() {
            EngineState::Broken
        } else {
            EngineState::NotInstalled
        };

        Ok(EngineDescriptor {
            id: manifest.id.clone(),
            display_name: manifest.display_name.clone(),
            provider: EngineProvider::Managed,
            state,
            version: Some(manifest.version.clone()),
            capabilities: manifest.capabilities.clone(),
        })
    }

    pub fn probe_system(
        &self,
        id: impl Into<String>,
        display_name: impl Into<String>,
        capabilities: Vec<String>,
        executable_names: &[&str],
    ) -> SystemProbe {
        let executable = find_on_path(executable_names);

        SystemProbe {
            descriptor: EngineDescriptor {
                id: id.into(),
                display_name: display_name.into(),
                provider: EngineProvider::System,
                state: if executable.is_some() {
                    EngineState::Ready
                } else {
                    EngineState::NotInstalled
                },
                version: None,
                capabilities,
            },
            executable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerError {
    InvalidManifest(String),
    Incompatible(String),
    AlreadyInstalled(String),
    NotInstalled(String),
    Ownership(String),
    ActiveVersion(String),
    Busy(String),
    Probe(String),
    State(String),
    Download(String),
    Integrity(String),
    Archive(String),
    Environment(String),
    Io(String),
}

impl fmt::Display for ManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifest(message) => write!(f, "invalid engine manifest: {message}"),
            Self::Incompatible(message) => write!(f, "engine package is incompatible: {message}"),
            Self::AlreadyInstalled(message) => write!(f, "engine is already installed: {message}"),
            Self::NotInstalled(message) => write!(f, "engine is not installed: {message}"),
            Self::Ownership(message) => write!(f, "engine ownership violation: {message}"),
            Self::ActiveVersion(message) => {
                write!(f, "active engine version cannot be removed: {message}")
            }
            Self::Busy(message) => write!(f, "engine mutation is busy: {message}"),
            Self::Probe(message) => write!(f, "system engine probe failed: {message}"),
            Self::State(message) => write!(f, "engine lifecycle state is invalid: {message}"),
            Self::Download(message) => write!(f, "engine download failed: {message}"),
            Self::Integrity(message) => write!(f, "engine integrity check failed: {message}"),
            Self::Archive(message) => write!(f, "engine archive rejected: {message}"),
            Self::Environment(message) => write!(f, "engine manager environment error: {message}"),
            Self::Io(message) => write!(f, "engine manager I/O error: {message}"),
        }
    }
}

impl Error for ManagerError {}

pub fn default_data_root() -> Result<PathBuf, ManagerError> {
    if let Some(path) = env::var_os("YU_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    platform_default_data_root()
}

#[cfg(target_os = "windows")]
fn platform_default_data_root() -> Result<PathBuf, ManagerError> {
    if let Some(path) = env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()) {
        Ok(PathBuf::from(path).join("YuTool"))
    } else {
        Err(ManagerError::Environment(
            "LOCALAPPDATA is not available; set YU_DATA_HOME explicitly".to_owned(),
        ))
    }
}

#[cfg(target_os = "macos")]
fn platform_default_data_root() -> Result<PathBuf, ManagerError> {
    let home = env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ManagerError::Environment(
                "HOME is not available; set YU_DATA_HOME explicitly".to_owned(),
            )
        })?;

    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("YuTool"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_default_data_root() -> Result<PathBuf, ManagerError> {
    if let Some(path) = env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        Ok(PathBuf::from(path).join("yu-tool"))
    } else {
        let home = env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ManagerError::Environment(
                    "HOME is not available; set YU_DATA_HOME explicitly".to_owned(),
                )
            })?;

        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("yu-tool"))
    }
}

#[cfg(not(any(unix, target_os = "windows")))]
fn platform_default_data_root() -> Result<PathBuf, ManagerError> {
    Err(ManagerError::Environment(
        "unsupported platform; set YU_DATA_HOME explicitly".to_owned(),
    ))
}

pub fn find_on_path(executable_names: &[&str]) -> Option<PathBuf> {
    let paths = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();

    find_in_paths(executable_names, &paths)
}

fn find_in_paths(executable_names: &[&str], paths: &[PathBuf]) -> Option<PathBuf> {
    for directory in paths {
        for name in executable_names {
            for candidate in executable_candidates(name) {
                let path = directory.join(candidate);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn executable_candidates(name: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let path = Path::new(name);
        if path.extension().is_some() {
            return vec![name.to_owned()];
        }

        let extensions = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());

        let mut candidates = vec![name.to_owned()];
        candidates.extend(
            extensions
                .split(';')
                .filter(|extension| !extension.is_empty())
                .map(|extension| format!("{name}{extension}")),
        );
        candidates
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![name.to_owned()]
    }
}

fn validate_identifier(label: &str, value: &str) -> Result<(), ManagerError> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(ManagerError::InvalidManifest(format!(
            "{label} must contain only lowercase ASCII letters, digits, '.', '_' or '-'"
        )));
    }

    Ok(())
}

fn validate_version_segment(value: &str) -> Result<(), ManagerError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'))
    {
        return Err(ManagerError::InvalidManifest(
            "engine version must be a safe path segment".to_owned(),
        ));
    }

    Ok(())
}

pub(crate) fn validate_package_args(args: &[String]) -> Result<(), ManagerError> {
    if args.len() > 64 {
        return Err(ManagerError::InvalidManifest(
            "package args must contain at most 64 fixed arguments".to_owned(),
        ));
    }

    let total_bytes = args.iter().map(String::len).sum::<usize>();
    if total_bytes > 64 * 1024 {
        return Err(ManagerError::InvalidManifest(
            "package args exceed the 64 KiB manifest limit".to_owned(),
        ));
    }

    for arg in args {
        if arg.as_bytes().contains(&0) {
            return Err(ManagerError::InvalidManifest(
                "package args must not contain NUL bytes".to_owned(),
            ));
        }
    }

    Ok(())
}

fn validate_relative_entrypoint(value: &str) -> Result<(), ManagerError> {
    let path = Path::new(value);

    if value.trim().is_empty()
        || value.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(ManagerError::InvalidManifest(
            "package entrypoint must be a safe relative path".to_owned(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn manifest_for(target: EngineTarget) -> EngineManifest {
        EngineManifest {
            schema_version: MANIFEST_SCHEMA_VERSION.to_owned(),
            id: "example-engine".to_owned(),
            display_name: "Example Engine".to_owned(),
            version: "1.2.3".to_owned(),
            capabilities: vec!["image.example".to_owned()],
            packages: vec![EnginePackage {
                target,
                url: "https://example.invalid/example.zip".to_owned(),
                sha256: "a".repeat(64),
                archive: ArchiveKind::Zip,
                entrypoint: "bin/example".to_owned(),
                args: Vec::new(),
            }],
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();

        env::temp_dir().join(format!(
            "yu-engine-manager-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn manifest_selects_matching_platform_package() {
        let target = EngineTarget::new("macos", "aarch64");
        let manifest = manifest_for(target.clone());

        manifest.validate().unwrap();

        let package = manifest.package_for(&target).unwrap();
        assert_eq!(package.entrypoint, "bin/example");
    }

    #[test]
    fn manifest_accepts_fixed_entrypoint_args() {
        let target = EngineTarget::new("linux", "x86_64");
        let mut manifest = manifest_for(target);
        manifest.packages[0].entrypoint = "runtime/node".to_owned();
        manifest.packages[0].args = vec!["engine/protocol.cjs".to_owned()];

        manifest.validate().unwrap();
        assert_eq!(
            manifest.packages[0].args,
            vec!["engine/protocol.cjs".to_owned()]
        );
    }

    #[test]
    fn manifest_rejects_nul_in_fixed_entrypoint_args() {
        let target = EngineTarget::new("linux", "x86_64");
        let mut manifest = manifest_for(target);
        manifest.packages[0].args = vec!["bad\0arg".to_owned()];

        assert!(matches!(
            manifest.validate(),
            Err(ManagerError::InvalidManifest(_))
        ));
    }

    #[test]
    fn manifest_rejects_parent_directory_entrypoint() {
        let target = EngineTarget::new("linux", "x86_64");
        let mut manifest = manifest_for(target);
        manifest.packages[0].entrypoint = "../escape".to_owned();

        assert!(matches!(
            manifest.validate(),
            Err(ManagerError::InvalidManifest(_))
        ));
    }

    #[test]
    fn managed_descriptor_reports_not_installed() {
        let root = temp_root("not-installed");
        let target = EngineTarget::new("linux", "x86_64");
        let manifest = manifest_for(target.clone());
        let manager = EngineManager::new(ManagedLayout::new(&root), target);

        let descriptor = manager.managed_descriptor(&manifest).unwrap();

        assert_eq!(descriptor.provider, EngineProvider::Managed);
        assert_eq!(descriptor.state, EngineState::NotInstalled);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn managed_descriptor_reports_ready_when_entrypoint_exists() {
        let root = temp_root("ready");
        let target = EngineTarget::new("linux", "x86_64");
        let manifest = manifest_for(target.clone());
        let manager = EngineManager::new(ManagedLayout::new(&root), target);
        let package = manifest.package_for(manager.target()).unwrap();
        let entrypoint = manager.layout().engine_entrypoint(&manifest, package);

        fs::create_dir_all(entrypoint.parent().unwrap()).unwrap();
        fs::write(&entrypoint, b"fixture").unwrap();

        let descriptor = manager.managed_descriptor(&manifest).unwrap();
        assert_eq!(descriptor.state, EngineState::Ready);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn managed_descriptor_reports_incompatible_without_target_package() {
        let root = temp_root("incompatible");
        let manifest = manifest_for(EngineTarget::new("macos", "aarch64"));
        let manager = EngineManager::new(
            ManagedLayout::new(&root),
            EngineTarget::new("windows", "x86_64"),
        );

        let descriptor = manager.managed_descriptor(&manifest).unwrap();
        assert_eq!(descriptor.state, EngineState::Incompatible);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn path_discovery_finds_fixture_executable() {
        let root = temp_root("path");
        fs::create_dir_all(&root).unwrap();

        #[cfg(target_os = "windows")]
        let executable_name = "yu-fixture.exe";
        #[cfg(not(target_os = "windows"))]
        let executable_name = "yu-fixture";

        let executable = root.join(executable_name);
        fs::write(&executable, b"fixture").unwrap();

        let found = find_in_paths(&["yu-fixture"], std::slice::from_ref(&root))
            .expect("fixture executable should be discovered");
        assert_eq!(
            fs::canonicalize(found).unwrap(),
            fs::canonicalize(&executable).unwrap()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn base_layout_creation_is_scoped_under_root() {
        let root = temp_root("layout");
        let layout = ManagedLayout::new(&root);

        layout.ensure_base_dirs().unwrap();

        assert!(layout.engines_dir().is_dir());
        assert!(layout.cache_dir().is_dir());
        assert!(layout.state_dir().is_dir());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn manifest_round_trip_is_stable() {
        let manifest = manifest_for(EngineTarget::new("linux", "x86_64"));
        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: EngineManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, manifest);
    }
}
