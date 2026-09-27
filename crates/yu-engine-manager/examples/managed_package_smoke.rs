use serde_json::json;
use std::{
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use yu_engine_api::{EngineProvider, EngineState};
use yu_engine_manager::{
    Downloader, EngineInstaller, EngineManifest, EngineTarget, ManagedLayout, ManagerError,
};

struct LocalPackageDownloader {
    package_path: PathBuf,
    expected_url: String,
}

impl Downloader for LocalPackageDownloader {
    fn download(&self, url: &str, destination: &Path) -> Result<u64, ManagerError> {
        if url != self.expected_url {
            return Err(ManagerError::Download(format!(
                "local smoke downloader expected {}, got {url}",
                self.expected_url
            )));
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ManagerError::Io(format!("cannot create {}: {error}", parent.display()))
            })?;
        }
        fs::copy(&self.package_path, destination).map_err(|error| {
            ManagerError::Io(format!(
                "cannot copy package {} to {}: {error}",
                self.package_path.display(),
                destination.display()
            ))
        })
    }
}

fn temp_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "yu-managed-ag-psd-smoke-{}-{nonce}",
        std::process::id()
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let manifest_path = PathBuf::from(
        args.next()
            .ok_or("usage: managed_package_smoke <manifest.json> <package.zip> <fixture.psd>")?,
    );
    let package_path = PathBuf::from(args.next().ok_or("missing package.zip")?);
    let fixture_path = PathBuf::from(args.next().ok_or("missing fixture.psd")?);
    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    let manifest: EngineManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    manifest.validate()?;

    let target = EngineTarget::current();
    let package = manifest
        .package_for(&target)
        .ok_or_else(|| {
            format!(
                "manifest has no package for current target {}/{}",
                target.os, target.arch
            )
        })?
        .clone();

    let root = temp_root();
    fs::create_dir_all(&root)?;
    let manager = yu_engine_manager::EngineManager::new(ManagedLayout::new(&root), target);
    let installer = EngineInstaller::new(
        manager,
        LocalPackageDownloader {
            package_path: package_path.clone(),
            expected_url: package.url.clone(),
        },
    );

    let install = installer.install(&manifest)?;
    let descriptor = installer.manager().managed_descriptor(&manifest)?;
    if descriptor.provider != EngineProvider::Managed {
        return Err("installed package is not managed".into());
    }

    let activation = installer
        .manager()
        .activate_version(&descriptor, &manifest.version)?;
    let command = installer
        .manager()
        .active_command(&descriptor)?
        .ok_or("active managed command is missing")?;

    let fixture = fs::canonicalize(&fixture_path)?;
    let request = json!({
        "protocol_version": "1",
        "request_id": "managed-package-smoke",
        "capability": "psd.inspect",
        "payload": {
            "input_path": fixture.to_string_lossy()
        }
    });

    let mut child = Command::new(&command.entrypoint)
        .args(&command.args)
        .current_dir(&command.working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .take()
        .ok_or("managed engine stdin is missing")?
        .write_all(&serde_json::to_vec(&request)?)?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "managed engine transport failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    if response["protocol_version"] != "1"
        || response["request_id"] != "managed-package-smoke"
        || response["status"] != "ok"
        || response["result"]["contract_version"] != "1"
        || response["result"]["document"]["format"] != "psd"
    {
        return Err(format!(
            "unexpected managed engine protocol response: {}",
            serde_json::to_string_pretty(&response)?
        )
        .into());
    }

    let inventory = installer.manager().discover_managed_inventory()?;
    let installed = inventory
        .iter()
        .find(|entry| entry.id == manifest.id)
        .ok_or("managed engine was not discovered after activation")?;
    if installed.state != EngineState::Ready
        || installed.active_version.as_deref() != Some(manifest.version.as_str())
    {
        return Err(format!(
            "managed engine inventory is not ready/active: {:?}",
            installed
        )
        .into());
    }

    let receipt = json!({
        "schema_version": "1",
        "engine_id": manifest.id,
        "engine_version": manifest.version,
        "target": {
            "os": installer.manager().target().os,
            "arch": installer.manager().target().arch
        },
        "install_sha256": install.sha256,
        "entrypoint": command.entrypoint,
        "args": command.args,
        "active_version": activation.active_version,
        "protocol_status": response["status"],
        "document": response["result"]["document"]
    });
    println!("{}", serde_json::to_string_pretty(&receipt)?);

    installer.manager().deactivate(&descriptor)?;
    installer
        .manager()
        .remove_version(&descriptor, &activation.active_version)?;
    let _ = fs::remove_dir_all(root);

    Ok(())
}
