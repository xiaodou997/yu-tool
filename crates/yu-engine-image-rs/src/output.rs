//! Destination-version binding and publish policy for built-in raster operations.
//! These guards coordinate YuTool writers. A non-cooperating external writer can
//! still race a final hash check and rename: filesystem rename is not a CAS.

use image::{DynamicImage, ImageError as NativeImageError, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use yu_capability_image::{ImageOperationError, OutputPolicy, OutputReceipt, VerifiedOutput};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(crate) struct OutputTransaction {
    output: PathBuf,
    expected_sha256: Option<String>,
    dry_run: bool,
    replacing: bool,
    // Keep this alive throughout staging, final verification and publication.
    _lock: Option<SidecarLock>,
}

impl OutputTransaction {
    pub(crate) fn prepare(
        output: &Path,
        policy: &OutputPolicy,
        dry_run: bool,
    ) -> Result<Self, ImageOperationError> {
        let expected_sha256 = policy
            .expected_sha256
            .as_ref()
            .map(|value| {
                if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(ImageOperationError::invalid_input(
                        "expected output SHA-256 must contain exactly 64 hexadecimal digits",
                    ));
                }
                Ok(value.to_ascii_lowercase())
            })
            .transpose()?;

        // Dry-run remains completely read-only. Real replacement serializes
        // cooperating YuTool writes with an exclusive sidecar create-new lock.
        let lock = if !dry_run && expected_sha256.is_some() {
            Some(SidecarLock::acquire(output)?)
        } else {
            None
        };
        let replacing = validate_destination(output, expected_sha256.as_deref())?;
        Ok(Self {
            output: output.to_path_buf(),
            expected_sha256,
            dry_run,
            replacing,
            _lock: lock,
        })
    }

    pub(crate) fn would_replace(&self) -> bool {
        self.dry_run && self.replacing
    }

    pub(crate) fn replaced(&self) -> bool {
        !self.dry_run && self.replacing
    }

    pub(crate) fn planned_receipt(&self) -> OutputReceipt {
        OutputReceipt::planned(self.expected_sha256.clone())
    }

    pub(crate) fn publish(
        &self,
        image: &DynamicImage,
        format: ImageFormat,
    ) -> Result<OutputReceipt, ImageOperationError> {
        self.publish_with_hooks(image, format, || {}, || {})
    }

    #[cfg(test)]
    fn publish_with_hook(
        &self,
        image: &DynamicImage,
        format: ImageFormat,
        before_final_verification: impl FnOnce(),
    ) -> Result<OutputReceipt, ImageOperationError> {
        self.publish_with_hooks(image, format, before_final_verification, || {})
    }

    fn publish_with_hooks(
        &self,
        image: &DynamicImage,
        format: ImageFormat,
        before_final_verification: impl FnOnce(),
        after_publication: impl FnOnce(),
    ) -> Result<OutputReceipt, ImageOperationError> {
        if self.dry_run {
            return Err(ImageOperationError::execution(
                "internal error: dry-run cannot publish an output",
            ));
        }
        let (mut file, stage) = StagedFile::create(&self.output)?;
        if let Err(error) = image.write_to(&mut file, format) {
            return Err(match error {
                NativeImageError::Unsupported(_) => {
                    ImageOperationError::unsupported(format!("cannot encode image: {error}"))
                }
                _ => ImageOperationError::execution(format!(
                    "cannot encode image to {}: {error}",
                    self.output.display()
                )),
            });
        }
        file.sync_all().map_err(|error| {
            ImageOperationError::execution(format!(
                "cannot sync staged image for {}: {error}",
                self.output.display()
            ))
        })?;
        drop(file);

        // Validate candidate bytes before publication. A broken encoding
        // cannot replace a valid existing destination.
        let staged = verify_image_file(&stage.path, format, image.width(), image.height())?;
        before_final_verification();
        // Reject changes that occurred while encoding. This check is deliberately
        // repeated as close as practical to publication. It cannot atomically
        // compare-and-swap an uncooperative external process.
        validate_destination(&self.output, self.expected_sha256.as_deref())?;
        let publish = if self.replacing {
            fs::rename(&stage.path, &self.output)
        } else {
            fs::hard_link(&stage.path, &self.output)
        };
        match publish {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && !self.replacing => {
                return Err(ImageOperationError::output_conflict(format!(
                    "output already exists: {}",
                    self.output.display()
                )));
            }
            Err(error) => {
                return Err(ImageOperationError::execution(format!(
                    "cannot publish completed image to {}: {error}",
                    self.output.display()
                )));
            }
        }
        after_publication();
        // A staged hash alone cannot establish the published file identity.
        // Explicit failure after publication must never look like success.
        let observed = verify_image_file(
            &self.output, format, image.width(), image.height()
        ).map_err(|error| ImageOperationError::verification(format!(
            "output may already be published at {}; post-publication verification failed: {}",
            self.output.display(), error.message
        )))?;
        if observed != staged {
            return Err(ImageOperationError::verification(format!(
                "output may already be published at {}; published bytes differ from verified staged output",
                self.output.display()
            )));
        }
        // Stage Drop removes the temporary name (also after hard-link creation).
        Ok(OutputReceipt::verified(
            self.expected_sha256.clone(),
            observed,
        ))
    }
}

fn verify_image_file(
    path: &Path,
    expected_format: ImageFormat,
    expected_width: u32,
    expected_height: u32,
) -> Result<VerifiedOutput, ImageOperationError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ImageOperationError::verification(format!(
            "cannot inspect encoded output {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.file_type().is_file() {
        return Err(ImageOperationError::verification(format!(
            "encoded output is not a regular file: {}",
            path.display()
        )));
    }
    let reader = ImageReader::open(path)
        .map_err(|error| {
            ImageOperationError::verification(format!(
                "cannot reopen encoded output {}: {error}",
                path.display()
            ))
        })?
        .with_guessed_format()
        .map_err(|error| {
            ImageOperationError::verification(format!(
                "cannot detect encoded format {}: {error}",
                path.display()
            ))
        })?;
    if reader.format() != Some(expected_format) {
        return Err(ImageOperationError::verification(format!(
            "output format does not match requested encoding at {}",
            path.display()
        )));
    }
    let decoded = reader.decode().map_err(|error| {
        ImageOperationError::verification(format!(
            "cannot decode encoded output {}: {error}",
            path.display()
        ))
    })?;
    if (decoded.width(), decoded.height()) != (expected_width, expected_height) {
        return Err(ImageOperationError::verification(format!(
            "output dimensions differ at {}: expected {}x{}, found {}x{}",
            path.display(),
            expected_width,
            expected_height,
            decoded.width(),
            decoded.height()
        )));
    }

    // Hash in chunks, avoiding an additional whole-file memory allocation.
    let mut file = File::open(path).map_err(|error| {
        ImageOperationError::verification(format!(
            "cannot reopen output bytes {}: {error}",
            path.display()
        ))
    })?;
    let mut digest = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            ImageOperationError::verification(format!(
                "cannot hash output {}: {error}",
                path.display()
            ))
        })?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .ok_or_else(|| ImageOperationError::verification("encoded output length overflow"))?;
        digest.update(&buffer[..count]);
    }
    if bytes != metadata.len() {
        return Err(ImageOperationError::verification(format!(
            "encoded output length changed during verification: {}",
            path.display()
        )));
    }
    Ok(VerifiedOutput {
        sha256: format!("{:x}", digest.finalize()),
        bytes,
        width: expected_width,
        height: expected_height,
        format: format!("{expected_format:?}").to_ascii_lowercase(),
    })
}

fn validate_destination(
    output: &Path,
    expected: Option<&str>,
) -> Result<bool, ImageOperationError> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    match fs::metadata(parent) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return Err(ImageOperationError::invalid_input(format!(
                "output parent is not a directory: {}",
                parent.display()
            )));
        }
        Err(error) => {
            return Err(ImageOperationError::invalid_input(format!(
                "output parent is unavailable ({}): {error}",
                parent.display()
            )));
        }
    }
    let metadata = match fs::symlink_metadata(output) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return if expected.is_some() {
                Err(ImageOperationError::output_conflict(format!(
                    "expected existing destination was not found: {}",
                    output.display()
                )))
            } else {
                Ok(false)
            };
        }
        Err(error) => {
            return Err(ImageOperationError::execution(format!(
                "cannot inspect output {}: {error}",
                output.display()
            )));
        }
    };

    if expected.is_none() {
        return Err(ImageOperationError::output_conflict(format!(
            "output already exists: {}",
            output.display()
        )));
    }
    if !metadata.file_type().is_file() {
        return Err(ImageOperationError::invalid_input(format!(
            "replacement destination must be a regular file, not a symlink or directory: {}",
            output.display()
        )));
    }
    let mut file = File::open(output).map_err(|error| {
        ImageOperationError::execution(format!(
            "cannot read replacement destination {}: {error}",
            output.display()
        ))
    })?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buf).map_err(|error| {
            ImageOperationError::execution(format!(
                "cannot hash replacement destination {}: {error}",
                output.display()
            ))
        })?;
        if count == 0 {
            break;
        }
        digest.update(&buf[..count]);
    }
    let actual = format!("{:x}", digest.finalize());
    if Some(actual.as_str()) != expected {
        return Err(ImageOperationError::output_conflict(format!(
            "destination SHA-256 differs from expected version: {}",
            output.display()
        )));
    }
    // Refuse a symlink swap that occurred while hashing a handle. This check
    // narrows but cannot eliminate OS-level replacement races.
    let after = fs::symlink_metadata(output).map_err(|error| {
        ImageOperationError::execution(format!(
            "cannot re-inspect destination {}: {error}",
            output.display()
        ))
    })?;
    if !after.file_type().is_file() || after.len() != metadata.len() {
        return Err(ImageOperationError::output_conflict(
            "replacement destination changed during verification",
        ));
    }
    Ok(true)
}

struct StagedFile {
    path: PathBuf,
}

impl StagedFile {
    fn create(output: &Path) -> Result<(File, Self), ImageOperationError> {
        let basename = output
            .file_name()
            .ok_or_else(|| {
                ImageOperationError::invalid_input("output path must contain a filename")
            })?
            .to_string_lossy();
        for _ in 0..16 {
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = output.with_file_name(format!(
                ".{basename}.yu-{}-{time}-{serial}.tmp",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok((file, Self { path })),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(ImageOperationError::execution(format!(
                        "cannot create staged output in {}: {error}",
                        output.display()
                    )));
                }
            }
        }
        Err(ImageOperationError::execution(
            "unable to allocate a unique staged output",
        ))
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct SidecarLock {
    path: PathBuf,
    file: Option<File>,
}

impl SidecarLock {
    fn acquire(output: &Path) -> Result<Self, ImageOperationError> {
        let basename = output
            .file_name()
            .ok_or_else(|| {
                ImageOperationError::invalid_input("output path must contain a filename")
            })?
            .to_string_lossy();
        let path = output.with_file_name(format!(".{basename}.yu-replace.lock"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => Ok(Self {
                path,
                file: Some(file),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(ImageOperationError::output_conflict(format!(
                    "YuTool replacement lock already exists: {}",
                    path.display()
                )))
            }
            Err(error) => Err(ImageOperationError::execution(format!(
                "cannot create YuTool replacement lock {}: {error}",
                path.display()
            ))),
        }
    }
}

impl Drop for SidecarLock {
    fn drop(&mut self) {
        // Windows cannot reliably remove an open regular-file handle.
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};

    #[test]
    fn reject_stale_hash_after_staging_without_overwriting_new_content() {
        let folder = std::env::temp_dir().join(format!(
            "yu-output-cas-test-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&folder).unwrap();
        let destination = folder.join("output.png");
        fs::write(&destination, b"old").unwrap();
        let expected = format!("{:x}", Sha256::digest(b"old"));
        let txn = OutputTransaction::prepare(
            &destination,
            &OutputPolicy {
                expected_sha256: Some(expected),
            },
            false,
        )
        .unwrap();
        let replacement =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255])));
        let error = txn
            .publish_with_hook(&replacement, ImageFormat::Png, || {
                fs::write(&destination, b"newer").unwrap();
            })
            .unwrap_err();
        assert_eq!(
            error.kind,
            yu_capability_image::ImageErrorKind::OutputConflict
        );
        assert_eq!(fs::read(&destination).unwrap(), b"newer");
        drop(txn);
        assert_eq!(
            fs::read_dir(&folder).unwrap().count(),
            1,
            "no temp or lock remains"
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn a_second_yutool_replacement_cannot_take_the_same_output_lock() {
        let folder = std::env::temp_dir().join(format!(
            "yu-output-lock-test-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&folder).unwrap();
        let destination = folder.join("output.png");
        fs::write(&destination, b"original").unwrap();
        let guard = OutputPolicy {
            expected_sha256: Some(format!("{:x}", Sha256::digest(b"original"))),
        };
        let first = OutputTransaction::prepare(&destination, &guard, false).unwrap();
        let error = OutputTransaction::prepare(&destination, &guard, false)
            .err()
            .expect("second real writer must conflict");
        assert_eq!(
            error.kind,
            yu_capability_image::ImageErrorKind::OutputConflict
        );
        assert_eq!(fs::read(&destination).unwrap(), b"original");
        drop(first);
        let second = OutputTransaction::prepare(&destination, &guard, false).unwrap();
        drop(second);
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn default_new_file_publish_preserves_racing_external_creation() {
        let folder = std::env::temp_dir().join(format!(
            "yu-output-create-test-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&folder).unwrap();
        let destination = folder.join("output.png");
        let transaction =
            OutputTransaction::prepare(&destination, &OutputPolicy::default(), false).unwrap();
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255])));
        let error = transaction
            .publish_with_hook(&image, ImageFormat::Png, || {
                fs::write(&destination, b"created-by-another-process").unwrap();
            })
            .unwrap_err();
        assert_eq!(
            error.kind,
            yu_capability_image::ImageErrorKind::OutputConflict
        );
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"created-by-another-process"
        );
        drop(transaction);
        assert_eq!(
            fs::read_dir(&folder).unwrap().count(),
            1,
            "no staged file remains"
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn verification_rejects_corrupt_format_and_incorrect_dimensions() {
        let folder = std::env::temp_dir().join(format!(
            "yu-post-verify-input-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&folder).unwrap();
        let path = folder.join("artifact.png");
        fs::write(&path, b"not-an-image").unwrap();
        let error = verify_image_file(&path, ImageFormat::Png, 2, 2).unwrap_err();
        assert_eq!(
            error.kind,
            yu_capability_image::ImageErrorKind::Verification
        );

        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255])));
        image.save_with_format(&path, ImageFormat::Png).unwrap();
        let wrong_format = verify_image_file(&path, ImageFormat::WebP, 2, 2).unwrap_err();
        assert_eq!(
            wrong_format.kind,
            yu_capability_image::ImageErrorKind::Verification
        );
        let wrong_size = verify_image_file(&path, ImageFormat::Png, 1, 2).unwrap_err();
        assert_eq!(
            wrong_size.kind,
            yu_capability_image::ImageErrorKind::Verification
        );
        let verified = verify_image_file(&path, ImageFormat::Png, 2, 2).unwrap();
        assert_eq!(
            verified.sha256,
            format!("{:x}", Sha256::digest(fs::read(&path).unwrap()))
        );
        assert_eq!(verified.bytes, fs::metadata(&path).unwrap().len());
        assert_eq!((verified.width, verified.height), (2, 2));
        assert_eq!(verified.format, "png");
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn corruption_after_publication_reports_failure_and_never_claims_receipt() {
        let folder = std::env::temp_dir().join(format!(
            "yu-post-verify-failure-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&folder).unwrap();
        let destination = folder.join("output.png");
        let txn =
            OutputTransaction::prepare(&destination, &OutputPolicy::default(), false).unwrap();
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255])));
        let error = txn
            .publish_with_hooks(
                &image,
                ImageFormat::Png,
                || {},
                || {
                    fs::write(&destination, b"corrupted-after-publish").unwrap();
                },
            )
            .unwrap_err();
        assert_eq!(
            error.kind,
            yu_capability_image::ImageErrorKind::Verification
        );
        assert!(error.message.contains("may already be published"));
        assert_eq!(fs::read(&destination).unwrap(), b"corrupted-after-publish");
        drop(txn);
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
        fs::remove_dir_all(folder).unwrap();
    }
}
