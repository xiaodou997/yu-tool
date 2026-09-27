use crate::{
    EngineManager, INSTALL_METADATA_FILE, InstalledEngineMetadata, LIFECYCLE_SCHEMA_VERSION,
    ManagerError, validate_identifier, validate_relative_entrypoint, validate_version_segment,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use yu_engine_api::{EngineDescriptor, EngineProvider, EngineState};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_PROBE_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineInventoryEntry {
    pub id: String,
    pub display_name: String,
    pub provider: EngineProvider,
    pub state: EngineState,
    pub version: Option<String>,
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub installed_versions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_version: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl EngineInventoryEntry {
    pub fn from_descriptor(descriptor: &EngineDescriptor) -> Self {
        Self {
            id: descriptor.id.clone(),
            display_name: descriptor.display_name.clone(),
            provider: descriptor.provider,
            state: descriptor.state,
            version: descriptor.version.clone(),
            capabilities: descriptor.capabilities.clone(),
            executable: None,
            installed_versions: Vec::new(),
            active_version: None,
            warnings: Vec::new(),
        }
    }

    pub fn descriptor(&self) -> EngineDescriptor {
        EngineDescriptor {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            provider: self.provider,
            state: self.state,
            version: self.version.clone(),
            capabilities: self.capabilities.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ActiveStateFile {
    schema_version: String,
    engine_id: String,
    active_version: String,
}

#[derive(Debug)]
struct ManagedVersionProbe {
    version: String,
    metadata: Option<InstalledEngineMetadata>,
    entrypoint: Option<PathBuf>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum VersionParser {
    ImageMagick,
    Ffmpeg,
    Trimmed,
}

#[derive(Debug, Clone, Copy)]
struct SystemEngineSpec {
    id: &'static str,
    display_name: &'static str,
    executable_names: &'static [&'static str],
    version_args: &'static [&'static str],
    parser: VersionParser,
}

const SYSTEM_ENGINE_SPECS: &[SystemEngineSpec] = &[
    SystemEngineSpec {
        id: "imagemagick",
        display_name: "ImageMagick",
        executable_names: &["magick"],
        version_args: &["-version"],
        parser: VersionParser::ImageMagick,
    },
    SystemEngineSpec {
        id: "ffmpeg",
        display_name: "FFmpeg",
        executable_names: &["ffmpeg"],
        version_args: &["-version"],
        parser: VersionParser::Ffmpeg,
    },
    SystemEngineSpec {
        id: "exiftool",
        display_name: "ExifTool",
        executable_names: &["exiftool"],
        version_args: &["-ver"],
        parser: VersionParser::Trimmed,
    },
];

impl EngineManager {
    pub fn discover_inventory(
        &self,
        built_in: &[EngineDescriptor],
    ) -> Result<Vec<EngineInventoryEntry>, ManagerError> {
        let mut inventory = built_in
            .iter()
            .map(EngineInventoryEntry::from_descriptor)
            .collect::<Vec<_>>();

        inventory.extend(self.discover_managed_inventory()?);
        inventory.extend(self.discover_system_inventory());

        inventory.sort_by(|left, right| {
            provider_priority(left.provider)
                .cmp(&provider_priority(right.provider))
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(inventory)
    }

    pub fn discover_managed_inventory(&self) -> Result<Vec<EngineInventoryEntry>, ManagerError> {
        let engines_dir = self.layout().engines_dir();
        if !engines_dir.exists() {
            return Ok(Vec::new());
        }

        let metadata = fs::symlink_metadata(&engines_dir).map_err(|error| {
            ManagerError::Io(format!("cannot inspect {}: {error}", engines_dir.display()))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ManagerError::State(format!(
                "managed engines root must be a real directory: {}",
                engines_dir.display()
            )));
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(&engines_dir).map_err(|error| {
            ManagerError::Io(format!("cannot read {}: {error}", engines_dir.display()))
        })? {
            let entry = entry.map_err(|error| {
                ManagerError::Io(format!(
                    "cannot read an entry from {}: {error}",
                    engines_dir.display()
                ))
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                ManagerError::Io(format!("cannot inspect {}: {error}", path.display()))
            })?;

            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }

            let id = entry
                .file_name()
                .into_string()
                .map_err(|_| ManagerError::State("managed engine id is not UTF-8".to_owned()))?;

            entries.push(self.inspect_managed_engine(&id, &path));
        }

        entries.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(entries)
    }

    pub fn discover_system_inventory(&self) -> Vec<EngineInventoryEntry> {
        SYSTEM_ENGINE_SPECS
            .iter()
            .filter_map(|spec| discover_system_engine(*spec))
            .collect()
    }

    fn inspect_managed_engine(&self, id: &str, engine_dir: &Path) -> EngineInventoryEntry {
        let mut warnings = Vec::new();

        if let Err(error) = validate_identifier("engine id", id) {
            return broken_managed_entry(id, vec![error.to_string()]);
        }

        let mut active_state_broken = false;
        let active_version = match read_active_state(self, id) {
            Ok(active) => active,
            Err(error) => {
                active_state_broken = true;
                warnings.push(error.to_string());
                None
            }
        };

        let mut versions = Vec::new();
        match fs::read_dir(engine_dir) {
            Ok(directory) => {
                for entry in directory {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            match entry.file_type() {
                                Ok(file_type) if file_type.is_dir() && !file_type.is_symlink() => {
                                    let version = match entry.file_name().into_string() {
                                        Ok(value) => value,
                                        Err(_) => {
                                            warnings.push(format!(
                                                "version directory is not UTF-8: {}",
                                                path.display()
                                            ));
                                            continue;
                                        }
                                    };
                                    versions
                                        .push(self.inspect_managed_version(id, &version, &path));
                                }
                                Ok(_) => {}
                                Err(error) => warnings.push(format!(
                                    "cannot inspect managed engine entry {}: {error}",
                                    path.display()
                                )),
                            }
                        }
                        Err(error) => warnings.push(format!(
                            "cannot read an entry from {}: {error}",
                            engine_dir.display()
                        )),
                    }
                }
            }
            Err(error) => warnings.push(format!(
                "cannot read managed engine directory {}: {error}",
                engine_dir.display()
            )),
        }

        versions.sort_by(|left, right| left.version.cmp(&right.version));

        let installed_versions = versions
            .iter()
            .map(|version| version.version.clone())
            .collect::<Vec<_>>();
        warnings.extend(
            versions
                .iter()
                .flat_map(|version| version.warnings.iter().cloned()),
        );

        let active_probe = active_version
            .as_ref()
            .and_then(|active| versions.iter().find(|version| &version.version == active));

        let valid_versions = versions
            .iter()
            .filter(|version| version.metadata.is_some() && version.entrypoint.is_some())
            .collect::<Vec<_>>();

        let display_name = active_probe
            .and_then(|version| version.metadata.as_ref())
            .and_then(|metadata| metadata.display_name.clone())
            .or_else(|| {
                valid_versions
                    .iter()
                    .find_map(|version| version.metadata.as_ref()?.display_name.clone())
            })
            .unwrap_or_else(|| id.to_owned());

        let mut capabilities = Vec::new();
        for version in &valid_versions {
            if let Some(metadata) = &version.metadata {
                for capability in &metadata.capabilities {
                    if !capabilities.contains(capability) {
                        capabilities.push(capability.clone());
                    }
                }
            }
        }
        capabilities.sort();

        let (state, version, executable) = match (&active_version, active_probe) {
            (Some(active), Some(probe))
                if probe.metadata.is_some() && probe.entrypoint.is_some() =>
            {
                (
                    EngineState::Ready,
                    Some(active.clone()),
                    probe.entrypoint.clone(),
                )
            }
            (Some(active), _) => {
                warnings.push(format!(
                    "active version {active} is missing or invalid for managed engine {id}"
                ));
                (EngineState::Broken, Some(active.clone()), None)
            }
            (None, _) if active_state_broken || valid_versions.is_empty() => {
                (EngineState::Broken, None, None)
            }
            (None, _) => (EngineState::Disabled, None, None),
        };

        EngineInventoryEntry {
            id: id.to_owned(),
            display_name,
            provider: EngineProvider::Managed,
            state,
            version,
            capabilities,
            executable,
            installed_versions,
            active_version,
            warnings,
        }
    }

    fn inspect_managed_version(
        &self,
        engine_id: &str,
        version: &str,
        version_dir: &Path,
    ) -> ManagedVersionProbe {
        let mut warnings = Vec::new();

        if let Err(error) = validate_version_segment(version) {
            warnings.push(error.to_string());
            return ManagedVersionProbe {
                version: version.to_owned(),
                metadata: None,
                entrypoint: None,
                warnings,
            };
        }

        let metadata_path = version_dir.join(INSTALL_METADATA_FILE);
        let file_metadata = match fs::symlink_metadata(&metadata_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                warnings.push(format!(
                    "cannot inspect installation metadata {}: {error}",
                    metadata_path.display()
                ));
                return ManagedVersionProbe {
                    version: version.to_owned(),
                    metadata: None,
                    entrypoint: None,
                    warnings,
                };
            }
        };

        if file_metadata.file_type().is_symlink() || !file_metadata.is_file() {
            warnings.push(format!(
                "installation metadata must be a real file: {}",
                metadata_path.display()
            ));
            return ManagedVersionProbe {
                version: version.to_owned(),
                metadata: None,
                entrypoint: None,
                warnings,
            };
        }

        let metadata: InstalledEngineMetadata = match fs::read(&metadata_path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))
        {
            Ok(metadata) => metadata,
            Err(error) => {
                warnings.push(format!(
                    "cannot parse installation metadata {}: {error}",
                    metadata_path.display()
                ));
                return ManagedVersionProbe {
                    version: version.to_owned(),
                    metadata: None,
                    entrypoint: None,
                    warnings,
                };
            }
        };

        if metadata.schema_version != LIFECYCLE_SCHEMA_VERSION {
            warnings.push(format!(
                "unsupported installation metadata schema version {} for {engine_id} {version}",
                metadata.schema_version
            ));
        }
        if metadata.engine_id != engine_id {
            warnings.push(format!(
                "installation metadata engine id mismatch for {engine_id} {version}: {}",
                metadata.engine_id
            ));
        }
        if metadata.version != version {
            warnings.push(format!(
                "installation metadata version mismatch for {engine_id} {version}: {}",
                metadata.version
            ));
        }
        if metadata.target != *self.target() {
            warnings.push(format!(
                "installed target {}/{} does not match current target {}/{}",
                metadata.target.os,
                metadata.target.arch,
                self.target().os,
                self.target().arch
            ));
        }
        if let Err(error) = validate_relative_entrypoint(&metadata.entrypoint) {
            warnings.push(error.to_string());
        }

        if !warnings.is_empty() {
            return ManagedVersionProbe {
                version: version.to_owned(),
                metadata: Some(metadata),
                entrypoint: None,
                warnings,
            };
        }

        let entrypoint = version_dir.join(&metadata.entrypoint);
        match fs::symlink_metadata(&entrypoint) {
            Ok(entry_metadata)
                if entry_metadata.is_file() && !entry_metadata.file_type().is_symlink() =>
            {
                ManagedVersionProbe {
                    version: version.to_owned(),
                    metadata: Some(metadata),
                    entrypoint: Some(entrypoint),
                    warnings,
                }
            }
            Ok(_) => {
                warnings.push(format!(
                    "managed engine entrypoint must be a real file: {}",
                    entrypoint.display()
                ));
                ManagedVersionProbe {
                    version: version.to_owned(),
                    metadata: Some(metadata),
                    entrypoint: None,
                    warnings,
                }
            }
            Err(error) => {
                warnings.push(format!(
                    "cannot inspect managed engine entrypoint {}: {error}",
                    entrypoint.display()
                ));
                ManagedVersionProbe {
                    version: version.to_owned(),
                    metadata: Some(metadata),
                    entrypoint: None,
                    warnings,
                }
            }
        }
    }
}

fn read_active_state(
    manager: &EngineManager,
    engine_id: &str,
) -> Result<Option<String>, ManagerError> {
    let path = manager
        .layout()
        .state_dir()
        .join("engines")
        .join(format!("{engine_id}.json"));

    if !path.exists() {
        return Ok(None);
    }

    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| ManagerError::Io(format!("cannot inspect {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ManagerError::State(format!(
            "engine active-state file must be a real file: {}",
            path.display()
        )));
    }

    let bytes = fs::read(&path)
        .map_err(|error| ManagerError::Io(format!("cannot read {}: {error}", path.display())))?;
    let state: ActiveStateFile = serde_json::from_slice(&bytes).map_err(|error| {
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

    Ok(Some(state.active_version))
}

fn broken_managed_entry(id: &str, warnings: Vec<String>) -> EngineInventoryEntry {
    EngineInventoryEntry {
        id: id.to_owned(),
        display_name: id.to_owned(),
        provider: EngineProvider::Managed,
        state: EngineState::Broken,
        version: None,
        capabilities: Vec::new(),
        executable: None,
        installed_versions: Vec::new(),
        active_version: None,
        warnings,
    }
}

fn discover_system_engine(spec: SystemEngineSpec) -> Option<EngineInventoryEntry> {
    let executable = crate::find_on_path(spec.executable_names)?;

    match probe_version(&executable, spec.version_args, spec.parser) {
        Ok(version) => Some(EngineInventoryEntry {
            id: spec.id.to_owned(),
            display_name: spec.display_name.to_owned(),
            provider: EngineProvider::System,
            state: EngineState::Ready,
            version: Some(version),
            capabilities: Vec::new(),
            executable: Some(executable),
            installed_versions: Vec::new(),
            active_version: None,
            warnings: Vec::new(),
        }),
        Err(error) => Some(EngineInventoryEntry {
            id: spec.id.to_owned(),
            display_name: spec.display_name.to_owned(),
            provider: EngineProvider::System,
            state: EngineState::Broken,
            version: None,
            capabilities: Vec::new(),
            executable: Some(executable),
            installed_versions: Vec::new(),
            active_version: None,
            warnings: vec![error.to_string()],
        }),
    }
}

fn probe_version(
    executable: &Path,
    args: &[&str],
    parser: VersionParser,
) -> Result<String, ManagerError> {
    let output = run_probe_command(executable, args, PROBE_TIMEOUT)?;
    parse_version_output(parser, &output).ok_or_else(|| {
        ManagerError::Probe(format!(
            "cannot parse version output from {}",
            executable.display()
        ))
    })
}

fn parse_version_output(parser: VersionParser, output: &str) -> Option<String> {
    let line = output.lines().find(|line| !line.trim().is_empty())?.trim();

    match parser {
        VersionParser::ImageMagick => line
            .split_once("ImageMagick ")
            .and_then(|(_, rest)| rest.split_whitespace().next())
            .map(str::to_owned),
        VersionParser::Ffmpeg => line
            .strip_prefix("ffmpeg version ")
            .and_then(|rest| rest.split_whitespace().next())
            .map(str::to_owned),
        VersionParser::Trimmed => Some(line.to_owned()),
    }
}

fn run_probe_command(
    executable: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<String, ManagerError> {
    let mut child = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            ManagerError::Probe(format!(
                "cannot execute version probe {}: {error}",
                executable.display()
            ))
        })?;

    let stdout = child
        .stdout
        .take()
        .expect("version probe stdout is always piped");
    let stderr = child
        .stderr
        .take()
        .expect("version probe stderr is always piped");

    let stdout_reader = thread::spawn(move || drain_probe_output(stdout));
    let stderr_reader = thread::spawn(move || drain_probe_output(stderr));

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(ManagerError::Probe(format!(
                    "version probe timed out after {} ms: {}",
                    timeout.as_millis(),
                    executable.display()
                )));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(ManagerError::Probe(format!(
                    "cannot wait for version probe {}: {error}",
                    executable.display()
                )));
            }
        }
    };

    let stdout = stdout_reader
        .join()
        .map_err(|_| ManagerError::Probe("stdout reader thread panicked".to_owned()))?
        .map_err(|error| ManagerError::Probe(format!("cannot read probe stdout: {error}")))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| ManagerError::Probe("stderr reader thread panicked".to_owned()))?
        .map_err(|error| ManagerError::Probe(format!("cannot read probe stderr: {error}")))?;

    let stdout = String::from_utf8_lossy(&stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&stderr).trim().to_owned();

    if !status.success() {
        return Err(ManagerError::Probe(format!(
            "version probe {} exited with {}: {}",
            executable.display(),
            status,
            if stderr.is_empty() { stdout } else { stderr }
        )));
    }

    if !stdout.is_empty() {
        Ok(stdout)
    } else if !stderr.is_empty() {
        Ok(stderr)
    } else {
        Err(ManagerError::Probe(format!(
            "version probe produced no output: {}",
            executable.display()
        )))
    }
}

fn drain_probe_output<R: Read>(mut reader: R) -> std::io::Result<Vec<u8>> {
    let mut stored = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        if stored.len() < MAX_PROBE_OUTPUT_BYTES {
            let remaining = MAX_PROBE_OUTPUT_BYTES - stored.len();
            stored.extend_from_slice(&buffer[..read.min(remaining)]);
        }
    }

    Ok(stored)
}

fn provider_priority(provider: EngineProvider) -> u8 {
    match provider {
        EngineProvider::BuiltIn => 0,
        EngineProvider::Managed => 1,
        EngineProvider::System => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::write_install_metadata;
    use crate::{
        ArchiveKind, EngineManifest, EnginePackage, EngineTarget, MANIFEST_SCHEMA_VERSION,
        ManagedLayout,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "yu-discovery-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn manager(root: &Path) -> EngineManager {
        EngineManager::new(
            ManagedLayout::new(root),
            EngineTarget::new("test-os", "test-arch"),
        )
    }

    fn prepare_version(manager: &EngineManager, engine_id: &str, version: &str) {
        let version_dir = manager.layout().engine_version_dir(engine_id, version);
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(version_dir.join("bin/fixture"), b"fixture").unwrap();

        let manifest = EngineManifest {
            schema_version: MANIFEST_SCHEMA_VERSION.to_owned(),
            id: engine_id.to_owned(),
            display_name: "Fixture Engine".to_owned(),
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
    fn managed_discovery_reports_inactive_and_active_versions() {
        let root = temp_root("managed");
        let manager = manager(&root);
        prepare_version(&manager, "fixture-engine", "1.0.0");
        prepare_version(&manager, "fixture-engine", "2.0.0");

        let inactive = manager.discover_managed_inventory().unwrap();
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].state, EngineState::Disabled);
        assert_eq!(inactive[0].installed_versions, vec!["1.0.0", "2.0.0"]);
        assert_eq!(inactive[0].display_name, "Fixture Engine");
        assert_eq!(inactive[0].capabilities, vec!["fixture.run"]);

        let descriptor = EngineDescriptor {
            id: "fixture-engine".to_owned(),
            display_name: "Fixture Engine".to_owned(),
            provider: EngineProvider::Managed,
            state: EngineState::Ready,
            version: None,
            capabilities: vec![],
        };
        manager.activate_version(&descriptor, "2.0.0").unwrap();

        let active = manager.discover_managed_inventory().unwrap();
        assert_eq!(active[0].state, EngineState::Ready);
        assert_eq!(active[0].version.as_deref(), Some("2.0.0"));
        assert_eq!(active[0].active_version.as_deref(), Some("2.0.0"));
        assert!(active[0].executable.as_ref().unwrap().is_file());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn managed_discovery_marks_stale_active_state_broken() {
        let root = temp_root("stale");
        let manager = manager(&root);
        prepare_version(&manager, "fixture-engine", "1.0.0");

        let state_dir = manager.layout().state_dir().join("engines");
        fs::create_dir_all(&state_dir).unwrap();
        fs::write(
            state_dir.join("fixture-engine.json"),
            r#"{"schema_version":"1","engine_id":"fixture-engine","active_version":"9.9.9"}"#,
        )
        .unwrap();

        let inventory = manager.discover_managed_inventory().unwrap();
        assert_eq!(inventory[0].state, EngineState::Broken);
        assert!(
            inventory[0]
                .warnings
                .iter()
                .any(|warning| warning.contains("active version 9.9.9"))
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn version_parsers_handle_known_tool_shapes() {
        assert_eq!(
            parse_version_output(
                VersionParser::ImageMagick,
                "Version: ImageMagick 7.1.2-3 Q16-HDRI"
            )
            .as_deref(),
            Some("7.1.2-3")
        );
        assert_eq!(
            parse_version_output(VersionParser::Ffmpeg, "ffmpeg version 8.0 Copyright (c)")
                .as_deref(),
            Some("8.0")
        );
        assert_eq!(
            parse_version_output(VersionParser::Trimmed, "13.40\n").as_deref(),
            Some("13.40")
        );
    }

    #[test]
    fn descriptor_compatibility_fields_remain_top_level() {
        let descriptor = EngineDescriptor {
            id: "raster-rs".to_owned(),
            display_name: "Raster".to_owned(),
            provider: EngineProvider::BuiltIn,
            state: EngineState::Ready,
            version: Some("0.1.0".to_owned()),
            capabilities: vec!["image.info".to_owned()],
        };
        let json =
            serde_json::to_value(EngineInventoryEntry::from_descriptor(&descriptor)).unwrap();

        assert_eq!(json["id"], "raster-rs");
        assert_eq!(json["provider"], "built_in");
        assert_eq!(json["state"], "ready");
        assert_eq!(json["version"], "0.1.0");
        assert_eq!(json["capabilities"][0], "image.info");
    }
}
