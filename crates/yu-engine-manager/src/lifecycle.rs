use crate::{
    EngineManager, EngineManifest, EnginePackage, EngineTarget, ManagerError, validate_identifier,
    validate_package_args, validate_relative_entrypoint, validate_version_segment,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use yu_engine_api::{EngineDescriptor, EngineProvider};

pub const LIFECYCLE_SCHEMA_VERSION: &str = "1";
pub const INSTALL_METADATA_FILE: &str = ".yu-install.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledEngineMetadata {
    pub schema_version: String,
    pub engine_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    pub target: EngineTarget,
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledVersion {
    pub engine_id: String,
    pub version: String,
    pub active: bool,
    pub path: PathBuf,
    pub entrypoint: PathBuf,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedEngineCommand {
    pub engine_id: String,
    pub version: String,
    pub entrypoint: PathBuf,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivationReceipt {
    pub engine_id: String,
    pub previous_version: Option<String>,
    pub active_version: String,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeactivationReceipt {
    pub engine_id: String,
    pub previous_version: Option<String>,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemovalReceipt {
    pub engine_id: String,
    pub version: String,
    pub cleanup_complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ActiveEngineState {
    schema_version: String,
    engine_id: String,
    active_version: String,
}

impl EngineManager {
    pub fn active_version(
        &self,
        descriptor: &EngineDescriptor,
    ) -> Result<Option<String>, ManagerError> {
        ensure_managed(descriptor)?;
        validate_identifier("engine id", &descriptor.id)?;

        let Some(state) = self.read_active_state(&descriptor.id)? else {
            return Ok(None);
        };

        let metadata = self.read_installed_metadata(&descriptor.id, &state.active_version)?;
        ensure_metadata_matches_target(self, &metadata)?;

        Ok(Some(state.active_version))
    }

    pub fn list_managed_versions(
        &self,
        descriptor: &EngineDescriptor,
    ) -> Result<Vec<InstalledVersion>, ManagerError> {
        ensure_managed(descriptor)?;
        validate_identifier("engine id", &descriptor.id)?;

        let active = self.active_version(descriptor)?;
        let engine_dir = self.layout().engine_dir(&descriptor.id);

        if !engine_dir.exists() {
            return Ok(Vec::new());
        }

        ensure_real_directory(&engine_dir, "managed engine directory")?;

        let mut versions = Vec::new();
        let entries = fs::read_dir(&engine_dir).map_err(|error| {
            ManagerError::Io(format!("cannot read {}: {error}", engine_dir.display()))
        })?;

        for entry in entries {
            let entry = entry.map_err(|error| {
                ManagerError::Io(format!(
                    "cannot read an entry from {}: {error}",
                    engine_dir.display()
                ))
            })?;

            let file_type = entry.file_type().map_err(|error| {
                ManagerError::Io(format!(
                    "cannot inspect {}: {error}",
                    entry.path().display()
                ))
            })?;

            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }

            let version = entry
                .file_name()
                .into_string()
                .map_err(|_| ManagerError::State("engine version name is not UTF-8".to_owned()))?;
            validate_version_segment(&version)?;

            let metadata = self.read_installed_metadata(&descriptor.id, &version)?;
            ensure_metadata_matches_target(self, &metadata)?;
            let entrypoint = entry.path().join(&metadata.entrypoint);
            ensure_real_file(&entrypoint, "engine entrypoint")?;

            versions.push(InstalledVersion {
                engine_id: descriptor.id.clone(),
                active: active.as_deref() == Some(version.as_str()),
                version,
                path: entry.path(),
                entrypoint,
                args: metadata.args.clone(),
            });
        }

        versions.sort_by(|left, right| left.version.cmp(&right.version));
        Ok(versions)
    }

    pub fn active_command(
        &self,
        descriptor: &EngineDescriptor,
    ) -> Result<Option<ManagedEngineCommand>, ManagerError> {
        ensure_managed(descriptor)?;
        validate_identifier("engine id", &descriptor.id)?;

        let Some(state) = self.read_active_state(&descriptor.id)? else {
            return Ok(None);
        };

        let metadata = self.read_installed_metadata(&descriptor.id, &state.active_version)?;
        ensure_metadata_matches_target(self, &metadata)?;
        let entrypoint = self
            .layout()
            .engine_version_dir(&descriptor.id, &state.active_version)
            .join(&metadata.entrypoint);
        ensure_real_file(&entrypoint, "engine entrypoint")?;

        Ok(Some(ManagedEngineCommand {
            engine_id: descriptor.id.clone(),
            version: state.active_version,
            entrypoint,
            args: metadata.args,
        }))
    }

    pub fn activate_version(
        &self,
        descriptor: &EngineDescriptor,
        version: &str,
    ) -> Result<ActivationReceipt, ManagerError> {
        ensure_managed(descriptor)?;
        validate_identifier("engine id", &descriptor.id)?;
        validate_version_segment(version)?;
        let _lock = self.acquire_engine_lock(&descriptor.id)?;

        let metadata = self.read_installed_metadata(&descriptor.id, version)?;
        ensure_metadata_matches_target(self, &metadata)?;
        let entrypoint = self
            .layout()
            .engine_version_dir(&descriptor.id, version)
            .join(&metadata.entrypoint);
        ensure_real_file(&entrypoint, "engine entrypoint")?;

        let previous_version = self
            .read_active_state(&descriptor.id)?
            .map(|state| state.active_version);

        if previous_version.as_deref() == Some(version) {
            return Ok(ActivationReceipt {
                engine_id: descriptor.id.clone(),
                previous_version: previous_version.clone(),
                active_version: version.to_owned(),
                changed: false,
            });
        }

        let state = ActiveEngineState {
            schema_version: LIFECYCLE_SCHEMA_VERSION.to_owned(),
            engine_id: descriptor.id.clone(),
            active_version: version.to_owned(),
        };
        self.write_active_state(&state)?;

        Ok(ActivationReceipt {
            engine_id: descriptor.id.clone(),
            previous_version,
            active_version: version.to_owned(),
            changed: true,
        })
    }

    pub fn deactivate(
        &self,
        descriptor: &EngineDescriptor,
    ) -> Result<DeactivationReceipt, ManagerError> {
        ensure_managed(descriptor)?;
        validate_identifier("engine id", &descriptor.id)?;
        let _lock = self.acquire_engine_lock(&descriptor.id)?;

        let Some(state) = self.read_active_state(&descriptor.id)? else {
            return Ok(DeactivationReceipt {
                engine_id: descriptor.id.clone(),
                previous_version: None,
                changed: false,
            });
        };

        let state_path = active_state_path(self, &descriptor.id);
        let trash = unique_trash_path(
            self,
            &format!("state-{}-{}", descriptor.id, state.active_version),
        );
        fs::create_dir_all(
            trash
                .parent()
                .expect("lifecycle trash path always has a parent"),
        )
        .map_err(|error| {
            ManagerError::Io(format!("cannot create lifecycle trash directory: {error}"))
        })?;

        fs::rename(&state_path, &trash).map_err(|error| {
            ManagerError::Io(format!(
                "cannot deactivate engine {}: {error}",
                descriptor.id
            ))
        })?;
        let _ = fs::remove_file(&trash);

        Ok(DeactivationReceipt {
            engine_id: descriptor.id.clone(),
            previous_version: Some(state.active_version),
            changed: true,
        })
    }

    pub fn remove_version(
        &self,
        descriptor: &EngineDescriptor,
        version: &str,
    ) -> Result<RemovalReceipt, ManagerError> {
        ensure_managed(descriptor)?;
        validate_identifier("engine id", &descriptor.id)?;
        validate_version_segment(version)?;
        let _lock = self.acquire_engine_lock(&descriptor.id)?;

        if self.active_version(descriptor)?.as_deref() == Some(version) {
            return Err(ManagerError::ActiveVersion(format!(
                "{} {} is active; switch or deactivate it before removal",
                descriptor.id, version
            )));
        }

        let version_dir = self.layout().engine_version_dir(&descriptor.id, version);
        if !version_dir.exists() {
            return Err(ManagerError::NotInstalled(format!(
                "{} {} is not installed",
                descriptor.id, version
            )));
        }
        ensure_real_directory(&version_dir, "managed engine version directory")?;

        let metadata = self.read_installed_metadata(&descriptor.id, version)?;
        ensure_metadata_matches_target(self, &metadata)?;

        let trash = unique_trash_path(self, &format!("{}-{version}", descriptor.id));
        fs::create_dir_all(
            trash
                .parent()
                .expect("lifecycle trash path always has a parent"),
        )
        .map_err(|error| {
            ManagerError::Io(format!("cannot create lifecycle trash directory: {error}"))
        })?;

        fs::rename(&version_dir, &trash).map_err(|error| {
            ManagerError::Io(format!(
                "cannot quarantine {} {} before removal: {error}",
                descriptor.id, version
            ))
        })?;

        let cleanup_complete = fs::remove_dir_all(&trash).is_ok();
        let cleanup_path = (!cleanup_complete).then_some(trash);

        let engine_dir = self.layout().engine_dir(&descriptor.id);
        if engine_dir.is_dir()
            && fs::read_dir(&engine_dir)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false)
        {
            let _ = fs::remove_dir(&engine_dir);
        }

        Ok(RemovalReceipt {
            engine_id: descriptor.id.clone(),
            version: version.to_owned(),
            cleanup_complete,
            cleanup_path,
        })
    }

    fn read_active_state(
        &self,
        engine_id: &str,
    ) -> Result<Option<ActiveEngineState>, ManagerError> {
        let path = active_state_path(self, engine_id);

        if !path.exists() {
            return Ok(None);
        }
        ensure_real_file(&path, "engine active-state file")?;

        let bytes = fs::read(&path).map_err(|error| {
            ManagerError::Io(format!("cannot read {}: {error}", path.display()))
        })?;
        let state: ActiveEngineState = serde_json::from_slice(&bytes).map_err(|error| {
            ManagerError::State(format!(
                "cannot parse active state {}: {error}",
                path.display()
            ))
        })?;

        if state.schema_version != LIFECYCLE_SCHEMA_VERSION {
            return Err(ManagerError::State(format!(
                "unsupported lifecycle state schema version: {}",
                state.schema_version
            )));
        }
        if state.engine_id != engine_id {
            return Err(ManagerError::State(format!(
                "active state engine id mismatch: expected {engine_id}, got {}",
                state.engine_id
            )));
        }

        validate_version_segment(&state.active_version)?;
        Ok(Some(state))
    }

    fn write_active_state(&self, state: &ActiveEngineState) -> Result<(), ManagerError> {
        let path = active_state_path(self, &state.engine_id);
        let parent = path
            .parent()
            .expect("active-state path always has a parent");
        fs::create_dir_all(parent).map_err(|error| {
            ManagerError::Io(format!("cannot create {}: {error}", parent.display()))
        })?;

        let temporary = parent.join(format!(
            ".{}.{}-{}.tmp",
            state.engine_id,
            std::process::id(),
            nonce()
        ));
        let bytes = serde_json::to_vec_pretty(state).map_err(|error| {
            ManagerError::State(format!("cannot serialize active state: {error}"))
        })?;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| {
                ManagerError::Io(format!(
                    "cannot create active-state staging file {}: {error}",
                    temporary.display()
                ))
            })?;
        file.write_all(&bytes).map_err(|error| {
            ManagerError::Io(format!(
                "cannot write active-state staging file {}: {error}",
                temporary.display()
            ))
        })?;
        file.sync_all().map_err(|error| {
            ManagerError::Io(format!(
                "cannot sync active-state staging file {}: {error}",
                temporary.display()
            ))
        })?;
        drop(file);

        if let Err(error) = atomic_replace_file(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        Ok(())
    }

    fn read_installed_metadata(
        &self,
        engine_id: &str,
        version: &str,
    ) -> Result<InstalledEngineMetadata, ManagerError> {
        let version_dir = self.layout().engine_version_dir(engine_id, version);
        if !version_dir.exists() {
            return Err(ManagerError::NotInstalled(format!(
                "{engine_id} {version} is not installed"
            )));
        }
        ensure_real_directory(&version_dir, "managed engine version directory")?;

        let path = version_dir.join(INSTALL_METADATA_FILE);
        ensure_real_file(&path, "managed engine installation metadata")?;

        let bytes = fs::read(&path).map_err(|error| {
            ManagerError::Io(format!("cannot read {}: {error}", path.display()))
        })?;
        let metadata: InstalledEngineMetadata =
            serde_json::from_slice(&bytes).map_err(|error| {
                ManagerError::State(format!(
                    "cannot parse installation metadata {}: {error}",
                    path.display()
                ))
            })?;

        validate_metadata(&metadata, engine_id, version)?;
        Ok(metadata)
    }
}

pub(crate) fn write_install_metadata(
    payload: &Path,
    manifest: &EngineManifest,
    package: &EnginePackage,
    sha256: &str,
) -> Result<(), ManagerError> {
    let path = payload.join(INSTALL_METADATA_FILE);
    if path.exists() {
        return Err(ManagerError::Archive(format!(
            "package contains reserved YuTool metadata path: {INSTALL_METADATA_FILE}"
        )));
    }

    let metadata = InstalledEngineMetadata {
        schema_version: LIFECYCLE_SCHEMA_VERSION.to_owned(),
        engine_id: manifest.id.clone(),
        version: manifest.version.clone(),
        display_name: Some(manifest.display_name.clone()),
        capabilities: manifest.capabilities.clone(),
        target: package.target.clone(),
        entrypoint: package.entrypoint.clone(),
        args: package.args.clone(),
        sha256: sha256.to_owned(),
    };
    validate_metadata(&metadata, &manifest.id, &manifest.version)?;

    let bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| {
        ManagerError::State(format!("cannot serialize installation metadata: {error}"))
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            ManagerError::Io(format!(
                "cannot create installation metadata {}: {error}",
                path.display()
            ))
        })?;
    file.write_all(&bytes).map_err(|error| {
        ManagerError::Io(format!(
            "cannot write installation metadata {}: {error}",
            path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        ManagerError::Io(format!(
            "cannot sync installation metadata {}: {error}",
            path.display()
        ))
    })?;

    Ok(())
}

fn validate_metadata(
    metadata: &InstalledEngineMetadata,
    expected_engine_id: &str,
    expected_version: &str,
) -> Result<(), ManagerError> {
    if metadata.schema_version != LIFECYCLE_SCHEMA_VERSION {
        return Err(ManagerError::State(format!(
            "unsupported installation metadata schema version: {}",
            metadata.schema_version
        )));
    }
    if metadata.engine_id != expected_engine_id {
        return Err(ManagerError::State(format!(
            "installation metadata engine id mismatch: expected {expected_engine_id}, got {}",
            metadata.engine_id
        )));
    }
    if metadata.version != expected_version {
        return Err(ManagerError::State(format!(
            "installation metadata version mismatch: expected {expected_version}, got {}",
            metadata.version
        )));
    }

    validate_identifier("engine id", &metadata.engine_id)?;
    validate_version_segment(&metadata.version)?;
    validate_relative_entrypoint(&metadata.entrypoint)?;
    validate_package_args(&metadata.args)?;

    if metadata.sha256.len() != 64 || !metadata.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ManagerError::State(
            "installation metadata contains an invalid SHA-256 digest".to_owned(),
        ));
    }

    Ok(())
}

fn ensure_metadata_matches_target(
    manager: &EngineManager,
    metadata: &InstalledEngineMetadata,
) -> Result<(), ManagerError> {
    if metadata.target != *manager.target() {
        return Err(ManagerError::Incompatible(format!(
            "installed engine {} {} targets {}/{} but current target is {}/{}",
            metadata.engine_id,
            metadata.version,
            metadata.target.os,
            metadata.target.arch,
            manager.target().os,
            manager.target().arch
        )));
    }

    Ok(())
}

fn ensure_managed(descriptor: &EngineDescriptor) -> Result<(), ManagerError> {
    if descriptor.provider != EngineProvider::Managed {
        return Err(ManagerError::Ownership(format!(
            "engine {} is {} and is not owned by YuTool",
            descriptor.id, descriptor.provider
        )));
    }

    Ok(())
}

fn active_state_path(manager: &EngineManager, engine_id: &str) -> PathBuf {
    manager
        .layout()
        .state_dir()
        .join("engines")
        .join(format!("{engine_id}.json"))
}

fn unique_trash_path(manager: &EngineManager, label: &str) -> PathBuf {
    manager.layout().cache_dir().join("trash").join(format!(
        "{label}-{}-{}",
        std::process::id(),
        nonce()
    ))
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn ensure_real_directory(path: &Path, label: &str) -> Result<(), ManagerError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ManagerError::NotInstalled(format!("{label} does not exist: {}", path.display()))
        } else {
            ManagerError::Io(format!("cannot inspect {}: {error}", path.display()))
        }
    })?;

    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ManagerError::State(format!(
            "{label} must be a real directory: {}",
            path.display()
        )));
    }

    Ok(())
}

fn ensure_real_file(path: &Path, label: &str) -> Result<(), ManagerError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ManagerError::State(format!("{label} is missing: {}", path.display()))
        } else {
            ManagerError::Io(format!("cannot inspect {}: {error}", path.display()))
        }
    })?;

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ManagerError::State(format!(
            "{label} must be a real file: {}",
            path.display()
        )));
    }

    Ok(())
}

#[cfg(unix)]
fn atomic_replace_file(source: &Path, destination: &Path) -> Result<(), ManagerError> {
    fs::rename(source, destination).map_err(|error| {
        ManagerError::Io(format!(
            "cannot atomically replace {} with {}: {error}",
            destination.display(),
            source.display()
        ))
    })
}

#[cfg(windows)]
fn atomic_replace_file(source: &Path, destination: &Path) -> Result<(), ManagerError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();

    let result = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if result == 0 {
        return Err(ManagerError::Io(format!(
            "cannot atomically replace {} with {}: {}",
            destination.display(),
            source.display(),
            std::io::Error::last_os_error()
        )));
    }

    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn atomic_replace_file(source: &Path, destination: &Path) -> Result<(), ManagerError> {
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| {
            ManagerError::Io(format!("cannot replace {}: {error}", destination.display()))
        })?;
    }
    fs::rename(source, destination).map_err(|error| {
        ManagerError::Io(format!(
            "cannot move {} to {}: {error}",
            source.display(),
            destination.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArchiveKind, MANIFEST_SCHEMA_VERSION};
    use std::time::{SystemTime, UNIX_EPOCH};
    use yu_engine_api::{EngineProvider, EngineState};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "yu-lifecycle-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn manager(root: &Path) -> EngineManager {
        EngineManager::new(
            crate::ManagedLayout::new(root),
            EngineTarget::new("test-os", "test-arch"),
        )
    }

    fn descriptor(id: &str, provider: EngineProvider) -> EngineDescriptor {
        EngineDescriptor {
            id: id.to_owned(),
            display_name: "Fixture".to_owned(),
            provider,
            state: EngineState::Ready,
            version: None,
            capabilities: vec!["fixture.run".to_owned()],
        }
    }

    fn prepare_version(manager: &EngineManager, engine_id: &str, version: &str) {
        let version_dir = manager.layout().engine_version_dir(engine_id, version);
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(version_dir.join("bin/fixture"), b"fixture").unwrap();

        let manifest = EngineManifest {
            schema_version: MANIFEST_SCHEMA_VERSION.to_owned(),
            id: engine_id.to_owned(),
            display_name: "Fixture".to_owned(),
            version: version.to_owned(),
            capabilities: vec!["fixture.run".to_owned()],
            packages: vec![EnginePackage {
                target: manager.target().clone(),
                url: "https://example.invalid/fixture".to_owned(),
                sha256: "a".repeat(64),
                archive: ArchiveKind::Raw,
                entrypoint: "bin/fixture".to_owned(),
                args: Vec::new(),
            }],
        };

        write_install_metadata(
            &version_dir,
            &manifest,
            &manifest.packages[0],
            &"a".repeat(64),
        )
        .unwrap();
    }

    #[test]
    fn multiple_versions_can_coexist_and_be_discovered() {
        let root = temp_root("versions");
        let manager = manager(&root);
        let descriptor = descriptor("fixture-engine", EngineProvider::Managed);

        prepare_version(&manager, &descriptor.id, "1.0.0");
        prepare_version(&manager, &descriptor.id, "2.0.0");

        let versions = manager.list_managed_versions(&descriptor).unwrap();

        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, "1.0.0");
        assert_eq!(versions[1].version, "2.0.0");
        assert!(!versions[0].active);
        assert!(!versions[1].active);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn activation_switch_is_persisted_and_reports_previous_version() {
        let root = temp_root("switch");
        let manager = manager(&root);
        let descriptor = descriptor("fixture-engine", EngineProvider::Managed);

        prepare_version(&manager, &descriptor.id, "1.0.0");
        prepare_version(&manager, &descriptor.id, "2.0.0");

        let first = manager.activate_version(&descriptor, "1.0.0").unwrap();
        assert_eq!(first.previous_version, None);
        assert!(first.changed);

        let second = manager.activate_version(&descriptor, "2.0.0").unwrap();
        assert_eq!(second.previous_version.as_deref(), Some("1.0.0"));
        assert_eq!(second.active_version, "2.0.0");
        assert!(second.changed);
        assert_eq!(
            manager.active_version(&descriptor).unwrap().as_deref(),
            Some("2.0.0")
        );

        let command = manager
            .active_command(&descriptor)
            .unwrap()
            .expect("active command should exist");
        assert_eq!(command.engine_id, "fixture-engine");
        assert_eq!(command.version, "2.0.0");
        assert!(command.entrypoint.ends_with("bin/fixture"));
        assert!(command.args.is_empty());

        let versions = manager.list_managed_versions(&descriptor).unwrap();
        assert!(
            !versions
                .iter()
                .find(|item| item.version == "1.0.0")
                .unwrap()
                .active
        );
        assert!(
            versions
                .iter()
                .find(|item| item.version == "2.0.0")
                .unwrap()
                .active
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn activating_same_version_is_idempotent() {
        let root = temp_root("idempotent");
        let manager = manager(&root);
        let descriptor = descriptor("fixture-engine", EngineProvider::Managed);
        prepare_version(&manager, &descriptor.id, "1.0.0");

        manager.activate_version(&descriptor, "1.0.0").unwrap();
        let receipt = manager.activate_version(&descriptor, "1.0.0").unwrap();

        assert!(!receipt.changed);
        assert_eq!(receipt.previous_version.as_deref(), Some("1.0.0"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn active_version_removal_is_rejected() {
        let root = temp_root("active-remove");
        let manager = manager(&root);
        let descriptor = descriptor("fixture-engine", EngineProvider::Managed);
        prepare_version(&manager, &descriptor.id, "1.0.0");
        manager.activate_version(&descriptor, "1.0.0").unwrap();

        let error = manager.remove_version(&descriptor, "1.0.0").unwrap_err();

        assert!(matches!(error, ManagerError::ActiveVersion(_)));
        assert!(
            manager
                .layout()
                .engine_version_dir(&descriptor.id, "1.0.0")
                .is_dir()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deactivate_then_remove_cleans_managed_version() {
        let root = temp_root("remove");
        let manager = manager(&root);
        let descriptor = descriptor("fixture-engine", EngineProvider::Managed);
        prepare_version(&manager, &descriptor.id, "1.0.0");
        manager.activate_version(&descriptor, "1.0.0").unwrap();

        let deactivation = manager.deactivate(&descriptor).unwrap();
        assert_eq!(deactivation.previous_version.as_deref(), Some("1.0.0"));
        assert!(deactivation.changed);

        let receipt = manager.remove_version(&descriptor, "1.0.0").unwrap();
        assert!(receipt.cleanup_complete);
        assert!(receipt.cleanup_path.is_none());
        assert!(
            !manager
                .layout()
                .engine_version_dir(&descriptor.id, "1.0.0")
                .exists()
        );
        assert_eq!(manager.active_version(&descriptor).unwrap(), None);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn removing_one_version_preserves_other_versions() {
        let root = temp_root("preserve");
        let manager = manager(&root);
        let descriptor = descriptor("fixture-engine", EngineProvider::Managed);
        prepare_version(&manager, &descriptor.id, "1.0.0");
        prepare_version(&manager, &descriptor.id, "2.0.0");
        manager.activate_version(&descriptor, "2.0.0").unwrap();

        manager.remove_version(&descriptor, "1.0.0").unwrap();

        assert!(
            manager
                .layout()
                .engine_version_dir(&descriptor.id, "2.0.0")
                .is_dir()
        );
        assert_eq!(
            manager.active_version(&descriptor).unwrap().as_deref(),
            Some("2.0.0")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn built_in_and_system_engines_are_read_only_to_managed_lifecycle() {
        let root = temp_root("ownership");
        let manager = manager(&root);

        for provider in [EngineProvider::BuiltIn, EngineProvider::System] {
            let descriptor = descriptor("fixture-engine", provider);

            assert!(matches!(
                manager.activate_version(&descriptor, "1.0.0"),
                Err(ManagerError::Ownership(_))
            ));
            assert!(matches!(
                manager.remove_version(&descriptor, "1.0.0"),
                Err(ManagerError::Ownership(_))
            ));
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stale_active_state_is_rejected() {
        let root = temp_root("stale");
        let manager = manager(&root);
        let descriptor = descriptor("fixture-engine", EngineProvider::Managed);
        let state = ActiveEngineState {
            schema_version: LIFECYCLE_SCHEMA_VERSION.to_owned(),
            engine_id: descriptor.id.clone(),
            active_version: "9.9.9".to_owned(),
        };
        manager.write_active_state(&state).unwrap();

        assert!(matches!(
            manager.active_version(&descriptor),
            Err(ManagerError::NotInstalled(_))
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn install_metadata_rejects_reserved_collision() {
        let root = temp_root("reserved");
        let manager = manager(&root);
        let version_dir = manager
            .layout()
            .engine_version_dir("fixture-engine", "1.0.0");
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join(INSTALL_METADATA_FILE), b"reserved").unwrap();

        let manifest = EngineManifest {
            schema_version: MANIFEST_SCHEMA_VERSION.to_owned(),
            id: "fixture-engine".to_owned(),
            display_name: "Fixture".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![],
            packages: vec![EnginePackage {
                target: manager.target().clone(),
                url: "https://example.invalid/fixture".to_owned(),
                sha256: "a".repeat(64),
                archive: ArchiveKind::Raw,
                entrypoint: "bin/fixture".to_owned(),
                args: Vec::new(),
            }],
        };

        assert!(matches!(
            write_install_metadata(
                &version_dir,
                &manifest,
                &manifest.packages[0],
                &"a".repeat(64)
            ),
            Err(ManagerError::Archive(_))
        ));

        let _ = fs::remove_dir_all(root);
    }
}
