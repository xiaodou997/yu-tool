use crate::{
    ArchiveKind, EngineManager, EngineManifest, EnginePackage, ManagerError,
    lifecycle::write_install_metadata,
};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

pub const DEFAULT_MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
pub const DEFAULT_MAX_EXTRACTED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const DEFAULT_MAX_EXTRACTED_FILES: usize = 20_000;

pub trait Downloader {
    fn download(&self, url: &str, destination: &Path) -> Result<u64, ManagerError>;
}

#[derive(Clone)]
pub struct HttpDownloader {
    client: Client,
    max_bytes: u64,
}

impl HttpDownloader {
    pub fn new() -> Result<Self, ManagerError> {
        Self::with_max_bytes(DEFAULT_MAX_DOWNLOAD_BYTES)
    }

    pub fn with_max_bytes(max_bytes: u64) -> Result<Self, ManagerError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| {
                ManagerError::Download(format!("cannot initialize HTTP client: {error}"))
            })?;

        Ok(Self { client, max_bytes })
    }
}

impl Downloader for HttpDownloader {
    fn download(&self, url: &str, destination: &Path) -> Result<u64, ManagerError> {
        if !url.starts_with("https://") {
            return Err(ManagerError::Download(
                "managed engine downloads require an https:// URL".to_owned(),
            ));
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ManagerError::Io(format!("cannot create {}: {error}", parent.display()))
            })?;
        }

        let mut response =
            self.client.get(url).send().map_err(|error| {
                ManagerError::Download(format!("cannot download {url}: {error}"))
            })?;

        if !response.status().is_success() {
            return Err(ManagerError::Download(format!(
                "download failed for {url}: HTTP {}",
                response.status()
            )));
        }

        if let Some(length) = response.content_length()
            && length > self.max_bytes
        {
            return Err(ManagerError::Download(format!(
                "download exceeds limit of {} bytes",
                self.max_bytes
            )));
        }

        let mut file = File::create(destination).map_err(|error| {
            ManagerError::Io(format!(
                "cannot create download staging file {}: {error}",
                destination.display()
            ))
        })?;

        let mut total = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];

        loop {
            let read = response.read(&mut buffer).map_err(|error| {
                ManagerError::Download(format!("failed while reading {url}: {error}"))
            })?;

            if read == 0 {
                break;
            }

            total = total
                .checked_add(read as u64)
                .ok_or_else(|| ManagerError::Download("download size overflow".to_owned()))?;

            if total > self.max_bytes {
                let _ = fs::remove_file(destination);
                return Err(ManagerError::Download(format!(
                    "download exceeds limit of {} bytes",
                    self.max_bytes
                )));
            }

            file.write_all(&buffer[..read]).map_err(|error| {
                ManagerError::Io(format!(
                    "cannot write download staging file {}: {error}",
                    destination.display()
                ))
            })?;
        }

        file.sync_all().map_err(|error| {
            ManagerError::Io(format!(
                "cannot sync download staging file {}: {error}",
                destination.display()
            ))
        })?;

        Ok(total)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InstallLimits {
    pub max_extracted_bytes: u64,
    pub max_extracted_files: usize,
}

impl Default for InstallLimits {
    fn default() -> Self {
        Self {
            max_extracted_bytes: DEFAULT_MAX_EXTRACTED_BYTES,
            max_extracted_files: DEFAULT_MAX_EXTRACTED_FILES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallReceipt {
    pub engine_id: String,
    pub version: String,
    pub target_os: String,
    pub target_arch: String,
    pub entrypoint: PathBuf,
    pub downloaded_bytes: u64,
    pub sha256: String,
}

pub struct EngineInstaller<D> {
    manager: EngineManager,
    downloader: D,
    limits: InstallLimits,
}

impl<D> EngineInstaller<D>
where
    D: Downloader,
{
    pub fn new(manager: EngineManager, downloader: D) -> Self {
        Self {
            manager,
            downloader,
            limits: InstallLimits::default(),
        }
    }

    pub fn with_limits(manager: EngineManager, downloader: D, limits: InstallLimits) -> Self {
        Self {
            manager,
            downloader,
            limits,
        }
    }

    pub fn manager(&self) -> &EngineManager {
        &self.manager
    }

    pub fn install(&self, manifest: &EngineManifest) -> Result<InstallReceipt, ManagerError> {
        manifest.validate()?;
        let _lock = self.manager.acquire_engine_lock(&manifest.id)?;

        let package = manifest.package_for(self.manager.target()).ok_or_else(|| {
            ManagerError::Incompatible(format!(
                "engine {} {} has no package for {}/{}",
                manifest.id,
                manifest.version,
                self.manager.target().os,
                self.manager.target().arch
            ))
        })?;

        self.manager.layout().ensure_base_dirs()?;

        let final_dir = self
            .manager
            .layout()
            .engine_version_dir(&manifest.id, &manifest.version);

        if final_dir.exists() {
            return Err(ManagerError::AlreadyInstalled(format!(
                "engine {} {} is already present at {}",
                manifest.id,
                manifest.version,
                final_dir.display()
            )));
        }

        let staging_dir = self.staging_dir(manifest);
        let artifact = staging_dir.join("artifact.download");
        let payload = staging_dir.join("payload");

        fs::create_dir_all(&payload).map_err(|error| {
            ManagerError::Io(format!("cannot create {}: {error}", payload.display()))
        })?;

        let result = self.install_staged(manifest, package, &artifact, &payload, &final_dir);

        if staging_dir.exists() {
            let _ = fs::remove_dir_all(&staging_dir);
        }

        result
    }

    fn install_staged(
        &self,
        manifest: &EngineManifest,
        package: &EnginePackage,
        artifact: &Path,
        payload: &Path,
        final_dir: &Path,
    ) -> Result<InstallReceipt, ManagerError> {
        let downloaded_bytes = self.downloader.download(&package.url, artifact)?;
        let actual_sha256 = sha256_file(artifact)?;

        if !actual_sha256.eq_ignore_ascii_case(&package.sha256) {
            return Err(ManagerError::Integrity(format!(
                "SHA-256 mismatch for {} {}: expected {}, got {}",
                manifest.id, manifest.version, package.sha256, actual_sha256
            )));
        }

        extract_package(artifact, payload, package, self.limits)?;

        let staged_entrypoint = payload.join(&package.entrypoint);
        if !staged_entrypoint.is_file() {
            return Err(ManagerError::Archive(format!(
                "expected entrypoint is missing after extraction: {}",
                package.entrypoint
            )));
        }

        make_entrypoint_executable(&staged_entrypoint)?;
        write_install_metadata(payload, manifest, package, &actual_sha256)?;

        if final_dir.exists() {
            return Err(ManagerError::AlreadyInstalled(format!(
                "engine {} {} became installed while staging",
                manifest.id, manifest.version
            )));
        }

        let engine_dir = self.manager.layout().engine_dir(&manifest.id);
        fs::create_dir_all(&engine_dir).map_err(|error| {
            ManagerError::Io(format!("cannot create {}: {error}", engine_dir.display()))
        })?;

        fs::rename(payload, final_dir).map_err(|error| {
            ManagerError::Io(format!(
                "cannot atomically activate {} {} at {}: {error}",
                manifest.id,
                manifest.version,
                final_dir.display()
            ))
        })?;

        let final_entrypoint = final_dir.join(&package.entrypoint);
        if !final_entrypoint.is_file() {
            let _ = fs::remove_dir_all(final_dir);
            return Err(ManagerError::Archive(
                "entrypoint disappeared during activation".to_owned(),
            ));
        }

        Ok(InstallReceipt {
            engine_id: manifest.id.clone(),
            version: manifest.version.clone(),
            target_os: package.target.os.clone(),
            target_arch: package.target.arch.clone(),
            entrypoint: final_entrypoint,
            downloaded_bytes,
            sha256: actual_sha256,
        })
    }

    fn staging_dir(&self, manifest: &EngineManifest) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        self.manager
            .layout()
            .cache_dir()
            .join("staging")
            .join(format!(
                "{}-{}-{}-{nonce}",
                manifest.id,
                manifest.version,
                std::process::id()
            ))
    }
}

pub fn sha256_file(path: &Path) -> Result<String, ManagerError> {
    let mut file = File::open(path).map_err(|error| {
        ManagerError::Io(format!(
            "cannot open {} for hashing: {error}",
            path.display()
        ))
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            ManagerError::Io(format!("cannot hash {}: {error}", path.display()))
        })?;

        if read == 0 {
            break;
        }

        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn extract_package(
    artifact: &Path,
    destination: &Path,
    package: &EnginePackage,
    limits: InstallLimits,
) -> Result<(), ManagerError> {
    match package.archive {
        ArchiveKind::Raw => extract_raw(artifact, destination, package, limits),
        ArchiveKind::Zip => extract_zip(artifact, destination, limits),
        ArchiveKind::TarGz => extract_tar_gz(artifact, destination, limits),
    }
}

fn extract_raw(
    artifact: &Path,
    destination: &Path,
    package: &EnginePackage,
    limits: InstallLimits,
) -> Result<(), ManagerError> {
    let metadata = fs::metadata(artifact).map_err(|error| {
        ManagerError::Io(format!("cannot inspect {}: {error}", artifact.display()))
    })?;

    if metadata.len() > limits.max_extracted_bytes {
        return Err(ManagerError::Archive(format!(
            "raw package exceeds extraction limit of {} bytes",
            limits.max_extracted_bytes
        )));
    }

    let output = destination.join(&package.entrypoint);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ManagerError::Io(format!("cannot create {}: {error}", parent.display()))
        })?;
    }

    fs::copy(artifact, &output).map_err(|error| {
        ManagerError::Io(format!(
            "cannot stage raw package at {}: {error}",
            output.display()
        ))
    })?;

    Ok(())
}

fn extract_zip(
    artifact: &Path,
    destination: &Path,
    limits: InstallLimits,
) -> Result<(), ManagerError> {
    let file = File::open(artifact).map_err(|error| {
        ManagerError::Io(format!("cannot open {}: {error}", artifact.display()))
    })?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| ManagerError::Archive(format!("invalid zip archive: {error}")))?;

    let mut extracted_bytes = 0_u64;
    let mut extracted_files = 0_usize;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ManagerError::Archive(format!("cannot read zip entry: {error}")))?;

        let relative = entry
            .enclosed_name()
            .ok_or_else(|| ManagerError::Archive(format!("unsafe zip path: {}", entry.name())))?;
        validate_archive_path(&relative)?;

        if let Some(mode) = entry.unix_mode()
            && mode & 0o170000 == 0o120000
        {
            return Err(ManagerError::Archive(format!(
                "zip symlinks are not allowed: {}",
                relative.display()
            )));
        }

        let output = destination.join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|error| {
                ManagerError::Io(format!("cannot create {}: {error}", output.display()))
            })?;
            continue;
        }

        extracted_files = extracted_files
            .checked_add(1)
            .ok_or_else(|| ManagerError::Archive("extracted file count overflow".to_owned()))?;
        enforce_file_limit(extracted_files, limits)?;

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ManagerError::Io(format!("cannot create {}: {error}", parent.display()))
            })?;
        }

        let mut out = File::create(&output).map_err(|error| {
            ManagerError::Io(format!("cannot create {}: {error}", output.display()))
        })?;

        copy_with_limit(
            &mut entry,
            &mut out,
            &mut extracted_bytes,
            limits.max_extracted_bytes,
        )?;
    }

    Ok(())
}

fn extract_tar_gz(
    artifact: &Path,
    destination: &Path,
    limits: InstallLimits,
) -> Result<(), ManagerError> {
    let file = File::open(artifact).map_err(|error| {
        ManagerError::Io(format!("cannot open {}: {error}", artifact.display()))
    })?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let mut extracted_bytes = 0_u64;
    let mut extracted_files = 0_usize;

    let entries = archive
        .entries()
        .map_err(|error| ManagerError::Archive(format!("invalid tar.gz archive: {error}")))?;

    for entry in entries {
        let mut entry = entry
            .map_err(|error| ManagerError::Archive(format!("cannot read tar entry: {error}")))?;
        let relative = entry
            .path()
            .map_err(|error| ManagerError::Archive(format!("invalid tar path: {error}")))?
            .into_owned();

        validate_archive_path(&relative)?;

        let entry_type = entry.header().entry_type();
        let output = destination.join(&relative);

        if entry_type.is_dir() {
            fs::create_dir_all(&output).map_err(|error| {
                ManagerError::Io(format!("cannot create {}: {error}", output.display()))
            })?;
            continue;
        }

        if !entry_type.is_file() {
            return Err(ManagerError::Archive(format!(
                "tar entry type is not allowed: {}",
                relative.display()
            )));
        }

        extracted_files = extracted_files
            .checked_add(1)
            .ok_or_else(|| ManagerError::Archive("extracted file count overflow".to_owned()))?;
        enforce_file_limit(extracted_files, limits)?;

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ManagerError::Io(format!("cannot create {}: {error}", parent.display()))
            })?;
        }

        let mut out = File::create(&output).map_err(|error| {
            ManagerError::Io(format!("cannot create {}: {error}", output.display()))
        })?;

        copy_with_limit(
            &mut entry,
            &mut out,
            &mut extracted_bytes,
            limits.max_extracted_bytes,
        )?;
    }

    Ok(())
}

fn validate_archive_path(path: &Path) -> Result<(), ManagerError> {
    if path.as_os_str().is_empty() {
        return Err(ManagerError::Archive(
            "archive path must not be empty".to_owned(),
        ));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ManagerError::Archive(format!(
                    "unsafe archive path: {}",
                    path.display()
                )));
            }
        }
    }

    Ok(())
}

fn enforce_file_limit(files: usize, limits: InstallLimits) -> Result<(), ManagerError> {
    if files > limits.max_extracted_files {
        return Err(ManagerError::Archive(format!(
            "archive exceeds file-count limit of {}",
            limits.max_extracted_files
        )));
    }

    Ok(())
}

fn copy_with_limit<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    total: &mut u64,
    max_bytes: u64,
) -> Result<(), ManagerError> {
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            ManagerError::Archive(format!("cannot read archive entry: {error}"))
        })?;

        if read == 0 {
            break;
        }

        *total = total
            .checked_add(read as u64)
            .ok_or_else(|| ManagerError::Archive("extracted size overflow".to_owned()))?;

        if *total > max_bytes {
            return Err(ManagerError::Archive(format!(
                "archive exceeds extraction limit of {max_bytes} bytes"
            )));
        }

        writer
            .write_all(&buffer[..read])
            .map_err(|error| ManagerError::Io(format!("cannot write extracted file: {error}")))?;
    }

    Ok(())
}

#[cfg(unix)]
fn make_entrypoint_executable(path: &Path) -> Result<(), ManagerError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)
        .map_err(|error| ManagerError::Io(format!("cannot inspect {}: {error}", path.display())))?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(path, permissions).map_err(|error| {
        ManagerError::Io(format!(
            "cannot mark engine entrypoint executable {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn make_entrypoint_executable(_path: &Path) -> Result<(), ManagerError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EngineTarget, MANIFEST_SCHEMA_VERSION, ManagedLayout};
    use flate2::{Compression, write::GzEncoder};
    use std::io::{Cursor, Write};
    use tar::{Builder, EntryType, Header};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    #[derive(Clone)]
    struct FixtureDownloader {
        bytes: Vec<u8>,
    }

    impl Downloader for FixtureDownloader {
        fn download(&self, _url: &str, destination: &Path) -> Result<u64, ManagerError> {
            fs::write(destination, &self.bytes).map_err(|error| {
                ManagerError::Io(format!("cannot write fixture download: {error}"))
            })?;
            Ok(self.bytes.len() as u64)
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "yu-installer-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    fn manifest(archive: ArchiveKind, bytes: &[u8], entrypoint: &str) -> EngineManifest {
        EngineManifest {
            schema_version: MANIFEST_SCHEMA_VERSION.to_owned(),
            id: "fixture-engine".to_owned(),
            display_name: "Fixture Engine".to_owned(),
            version: "1.2.3".to_owned(),
            capabilities: vec!["fixture.run".to_owned()],
            packages: vec![EnginePackage {
                target: EngineTarget::new("test-os", "test-arch"),
                url: "https://example.invalid/fixture".to_owned(),
                sha256: sha256_bytes(bytes),
                archive,
                entrypoint: entrypoint.to_owned(),
                args: Vec::new(),
            }],
        }
    }

    fn installer(bytes: Vec<u8>, root: &Path) -> EngineInstaller<FixtureDownloader> {
        EngineInstaller::new(
            EngineManager::new(
                ManagedLayout::new(root),
                EngineTarget::new("test-os", "test-arch"),
            ),
            FixtureDownloader { bytes },
        )
    }

    fn zip_fixture(path: &str, contents: &[u8]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file(path, options).unwrap();
        writer.write_all(contents).unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn tar_gz_fixture(path: &str, contents: &[u8]) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);

        let mut header = Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, contents).unwrap();

        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn raw_install_verifies_and_activates() {
        let root = temp_root("raw");
        let bytes = b"fixture executable".to_vec();
        let manifest = manifest(ArchiveKind::Raw, &bytes, "bin/fixture");
        let installer = installer(bytes.clone(), &root);

        let receipt = installer.install(&manifest).unwrap();

        assert_eq!(fs::read(&receipt.entrypoint).unwrap(), bytes);
        assert_eq!(receipt.sha256, manifest.packages[0].sha256);

        let descriptor = installer.manager().managed_descriptor(&manifest).unwrap();
        assert_eq!(descriptor.state, yu_engine_api::EngineState::Ready);
        assert!(
            receipt
                .entrypoint
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(crate::INSTALL_METADATA_FILE)
                .is_file()
        );

        let activation = installer
            .manager()
            .activate_version(&descriptor, &manifest.version)
            .unwrap();
        assert_eq!(activation.active_version, manifest.version);
        assert_eq!(
            installer.manager().active_version(&descriptor).unwrap(),
            Some(manifest.version.clone())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn digest_mismatch_does_not_activate() {
        let root = temp_root("digest");
        let bytes = b"actual bytes".to_vec();
        let mut manifest = manifest(ArchiveKind::Raw, &bytes, "bin/fixture");
        manifest.packages[0].sha256 = "0".repeat(64);
        let installer = installer(bytes, &root);

        let error = installer.install(&manifest).unwrap_err();

        assert!(matches!(error, ManagerError::Integrity(_)));
        assert!(
            !installer
                .manager()
                .layout()
                .engine_version_dir(&manifest.id, &manifest.version)
                .exists()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn zip_install_extracts_expected_entrypoint() {
        let root = temp_root("zip");
        let bytes = zip_fixture("bin/fixture", b"zip fixture");
        let manifest = manifest(ArchiveKind::Zip, &bytes, "bin/fixture");
        let installer = installer(bytes, &root);

        let receipt = installer.install(&manifest).unwrap();

        assert_eq!(fs::read(receipt.entrypoint).unwrap(), b"zip fixture");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tar_gz_install_extracts_expected_entrypoint() {
        let root = temp_root("tar-gz");
        let bytes = tar_gz_fixture("bin/fixture", b"tar fixture");
        let manifest = manifest(ArchiveKind::TarGz, &bytes, "bin/fixture");
        let installer = installer(bytes, &root);

        let receipt = installer.install(&manifest).unwrap();

        assert_eq!(fs::read(receipt.entrypoint).unwrap(), b"tar fixture");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn zip_traversal_is_rejected() {
        let root = temp_root("zip-traversal");
        let bytes = zip_fixture("../escape", b"bad");
        let manifest = manifest(ArchiveKind::Zip, &bytes, "bin/fixture");
        let installer = installer(bytes, &root);

        let error = installer.install(&manifest).unwrap_err();

        assert!(matches!(error, ManagerError::Archive(_)));
        assert!(
            !installer
                .manager()
                .layout()
                .engine_version_dir(&manifest.id, &manifest.version)
                .exists()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extraction_limit_is_enforced() {
        let root = temp_root("limit");
        let bytes = zip_fixture("bin/fixture", b"too large");
        let manifest = manifest(ArchiveKind::Zip, &bytes, "bin/fixture");
        let installer = EngineInstaller::with_limits(
            EngineManager::new(
                ManagedLayout::new(&root),
                EngineTarget::new("test-os", "test-arch"),
            ),
            FixtureDownloader { bytes },
            InstallLimits {
                max_extracted_bytes: 3,
                max_extracted_files: 10,
            },
        );

        let error = installer.install(&manifest).unwrap_err();

        assert!(matches!(error, ManagerError::Archive(_)));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn existing_version_is_not_overwritten() {
        let root = temp_root("existing");
        let bytes = b"fixture".to_vec();
        let manifest = manifest(ArchiveKind::Raw, &bytes, "bin/fixture");
        let installer = installer(bytes, &root);

        let version_dir = installer
            .manager()
            .layout()
            .engine_version_dir(&manifest.id, &manifest.version);
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join("marker"), b"keep").unwrap();

        let error = installer.install(&manifest).unwrap_err();

        assert!(matches!(error, ManagerError::AlreadyInstalled(_)));
        assert_eq!(fs::read(version_dir.join("marker")).unwrap(), b"keep");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsafe_archive_paths_are_rejected() {
        assert!(validate_archive_path(Path::new("../escape")).is_err());
        assert!(validate_archive_path(Path::new("/absolute")).is_err());
        assert!(validate_archive_path(Path::new("safe/path")).is_ok());
    }

    #[test]
    fn tar_symlink_is_rejected() {
        let root = temp_root("tar-symlink");

        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_link_name("../../escape").unwrap();
        header.set_cksum();
        builder
            .append_data(&mut header, "bin/fixture", std::io::empty())
            .unwrap();
        let encoder = builder.into_inner().unwrap();
        let bytes = encoder.finish().unwrap();

        let manifest = manifest(ArchiveKind::TarGz, &bytes, "bin/fixture");
        let installer = installer(bytes, &root);

        let error = installer.install(&manifest).unwrap_err();
        assert!(matches!(error, ManagerError::Archive(_)));

        let _ = fs::remove_dir_all(root);
    }
}
