//! Keep quarantine atomic while tolerating briefly held Windows sharing handles.
#[cfg(any(windows, test))]
use std::io;
use std::{fs, path::Path};

#[cfg(windows)]
const WINDOWS_QUARANTINE_RETRY_BUDGET: std::time::Duration =
    std::time::Duration::from_millis(2500);

/// The caller holds the per-engine mutation lock and has checked ownership,
/// activation, metadata and paths. Retry only this same rename; never copy or
/// delete the original version when quarantine cannot be completed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct QuarantineObservation {
    pub attempts: u32,
    pub elapsed_ms: u64,
}

pub(crate) fn rename(source: &Path, destination: &Path) -> Result<QuarantineObservation, String> {
    let started = std::time::Instant::now();
    let mut attempts = 0;
    let mut operation = || {
        attempts += 1;
        fs::rename(source, destination)
    };
    #[cfg(windows)]
    // Issue #27 RC evidence observed an identity-confirmed external regular-file user
    // on runtime/node.exe through ~2125 ms and gone shortly after the old 2 s bound.
    // Keep the operation atomic and add only 500 ms of sharing-class settle grace.
    let result = retry_windows_rename(&mut operation, WINDOWS_QUARANTINE_RETRY_BUDGET);
    #[cfg(not(windows))]
    let result = operation();
    let observation = QuarantineObservation {
        attempts,
        elapsed_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
    };
    result.map(|()| observation).map_err(|error| {
        let metadata = fs::symlink_metadata(source).ok();
        let cwd_inside_source = std::env::current_dir().ok().and_then(|cwd| {
            let source = fs::canonicalize(source).ok()?;
            Some(cwd.starts_with(source))
        });
        format!("{error}; quarantine diagnostic: attempts={attempts}, elapsed_ms={}, os_error={:?}, source={:?}, destination={:?}, source_present={}, source_readonly={:?}, cwd_inside_source={cwd_inside_source:?}; original version was not deleted; lock owner is unknown",
            started.elapsed().as_millis(), error.raw_os_error(), source, destination,
            metadata.is_some(), metadata.map(|m| m.permissions().readonly()))
    })
}

#[cfg(any(windows, test))]
fn retry_windows_rename(
    mut operation: impl FnMut() -> io::Result<()>,
    budget: std::time::Duration,
) -> io::Result<()> {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let started = Instant::now();
    loop {
        match operation() {
            Ok(()) => return Ok(()),
            Err(error) => {
                // WinError.h: ACCESS_DENIED, SHARING_VIOLATION, LOCK_VIOLATION.
                // Access denied may also be permanent; the deadline preserves
                // that failure instead of treating it as successful cleanup.
                if !matches!(error.raw_os_error(), Some(5 | 32 | 33)) || started.elapsed() >= budget
                {
                    return Err(error);
                }
                thread::sleep(
                    Duration::from_millis(25).min(budget.saturating_sub(started.elapsed())),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn successful_quarantine_is_not_repeated() {
        let mut calls = 0;
        retry_windows_rename(
            || {
                calls += 1;
                Ok(())
            },
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(calls, 1);
    }

    #[test]
    fn windows_sharing_failures_retry_without_changing_the_operation() {
        let mut calls = 0;
        let errors = [5, 32, 33];
        retry_windows_rename(
            || {
                calls += 1;
                if calls <= errors.len() {
                    Err(io::Error::from_raw_os_error(errors[calls - 1]))
                } else {
                    Ok(())
                }
            },
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(calls, 4);
    }

    #[test]
    fn unrelated_errors_are_not_retried() {
        let mut calls = 0;
        let error = retry_windows_rename(
            || {
                calls += 1;
                Err(io::Error::from_raw_os_error(2))
            },
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(error.raw_os_error(), Some(2));
    }

    #[test]
    fn exhausted_retry_budget_preserves_the_error() {
        let mut calls = 0;
        let error = retry_windows_rename(
            || {
                calls += 1;
                Err(io::Error::from_raw_os_error(5))
            },
            Duration::ZERO,
        )
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(error.raw_os_error(), Some(5));
    }

    #[cfg(windows)]
    #[test]
    fn windows_nested_file_holder_released_after_two_seconds_uses_settle_grace() {
        use std::{
            os::windows::fs::OpenOptionsExt,
            thread,
            time::{Duration, Instant, SystemTime, UNIX_EPOCH},
        };
        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

        let root = std::env::temp_dir().join(format!(
            "yu-quarantine-late-file-sharing-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("version");
        let target = root.join("quarantined");
        let runtime = source.join("runtime");
        let node = runtime.join("node.exe");
        fs::create_dir_all(&runtime).unwrap();
        fs::write(&node, b"node fixture").unwrap();

        // Model the RC Freeze reproduction: a nested executable is observed by an
        // external process beyond the historical two-second bound, then released.
        // No FILE_SHARE_DELETE means the directory rename must wait for this handle.
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&node)
            .unwrap();
        let releaser = thread::spawn(move || {
            thread::sleep(Duration::from_millis(2100));
            drop(held);
        });

        let started = Instant::now();
        let observation = rename(&source, &target).unwrap();
        releaser.join().unwrap();

        assert!(
            observation.elapsed_ms >= 2000,
            "control must cross the historical two-second bound: {observation:?}"
        );
        assert!(
            started.elapsed() < Duration::from_millis(2600),
            "sharing settle grace must remain bounded: {observation:?}"
        );
        assert!(!source.exists());
        assert_eq!(fs::read(target.join("runtime/node.exe")).unwrap(), b"node fixture");

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_directory_handle_blocks_quarantine_until_released() {
        use std::{
            os::windows::fs::OpenOptionsExt,
            thread,
            time::{SystemTime, UNIX_EPOCH},
        };
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        let root = std::env::temp_dir().join(format!(
            "yu-quarantine-sharing-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("version");
        let target = root.join("quarantined");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("sentinel"), b"keep intact").unwrap();
        // Without FILE_SHARE_DELETE, a live directory handle denies rename.
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(&source)
            .unwrap();
        let error =
            retry_windows_rename(|| fs::rename(&source, &target), Duration::from_millis(50))
                .expect_err("a held directory must not be treated as removed");
        assert!(matches!(error.raw_os_error(), Some(5 | 32 | 33)));
        assert!(source.is_dir());
        assert!(!target.exists());
        assert_eq!(fs::read(source.join("sentinel")).unwrap(), b"keep intact");
        let releaser = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            drop(held);
        });
        let outcome = rename(&source, &target);
        releaser.join().unwrap();
        outcome.unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(target.join("sentinel")).unwrap(), b"keep intact");
        fs::remove_dir_all(root).unwrap();
    }
}
