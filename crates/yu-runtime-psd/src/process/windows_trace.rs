//! Opt-in investigation snapshots of this invocation's handles; no process-control changes.
use super::{Child, windows_spawn::StartupObservation};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::windows::io::{AsRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::{
        JobObjects::{
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation,
            QueryInformationJobObject,
        },
        Threading::GetProcessTimes,
    },
};

pub(super) struct Trace {
    directory: PathBuf,
    identity: String,
    version_directory: PathBuf,
    child_created_filetime: Option<u64>,
}

fn unix_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn creation_time(child: &Child) -> Option<u64> {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: the Child owns a live process handle; all four outputs are initialized.
    let ok = unsafe {
        GetProcessTimes(
            child.as_raw_handle(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    (ok != 0).then_some(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
}

fn accounting(job: &OwnedHandle) -> Value {
    let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: query only this invocation's owned Job into a correctly sized POD buffer.
    let ok = unsafe {
        QueryInformationJobObject(
            job.as_raw_handle(),
            JobObjectBasicAccountingInformation,
            &mut info as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of_val(&info) as u32,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return json!({"status":"query_failed", "os_error":std::io::Error::last_os_error().raw_os_error()});
    }
    json!({"status":"observed", "active_processes":info.ActiveProcesses, "total_processes":info.TotalProcesses})
}

impl Trace {
    pub(super) fn start(
        child: &Child,
        job: &OwnedHandle,
        version_directory: &Path,
        startup: &StartupObservation,
    ) -> Option<Self> {
        let directory = PathBuf::from(std::env::var_os("YU_WINDOWS_LIFECYCLE_TRACE_DIR")?);
        let metadata = fs::symlink_metadata(&directory).ok()?;
        if !directory.is_absolute() || !metadata.is_dir() || metadata.file_type().is_symlink() {
            return None;
        }
        let trace = Self {
            directory,
            identity: format!("{}-{}-{}", std::process::id(), child.id(), unix_ns()),
            version_directory: version_directory.to_owned(),
            child_created_filetime: creation_time(child),
        };
        trace.record(child, job, "started", json!({"startup": startup.json()}));
        Some(trace)
    }

    pub(super) fn record(
        &self,
        child: &Child,
        job: &OwnedHandle,
        stage: &str,
        observations: Value,
    ) {
        let record = json!({
            "schema_version":"1", "kind":"owned_windows_job_snapshot", "invocation":self.identity,
            "stage":stage, "observed_unix_ns":unix_ns().to_string(),
            "yu_pid":std::process::id(), "child_pid":child.id(),
            "child_created_filetime":self.child_created_filetime.map(|v| v.to_string()),
            "version_directory":self.version_directory,
            "job":accounting(job), "observations":observations,
            "root_cause_fixed":false,
            "limitations":"Job accounting is an asynchronous snapshot with process handles still owned; it is not proof that all references are closed or an external lock-owner diagnosis."
        });
        // Each stage has a new file. Never overwrite an earlier invocation or mix JSON stderr.
        // Diagnostics are best effort and never change the underlying command result.
        if let Ok(bytes) = serde_json::to_vec(&record)
            && let Ok(mut file) = OpenOptions::new().write(true).create_new(true).open(
                self.directory
                    .join(format!("{}-{stage}.json", self.identity)),
            )
        {
            let _ = file.write_all(&bytes);
        }
    }
}
