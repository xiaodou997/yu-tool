//! Creation-time private Job ownership on stable Rust, using documented Windows APIs.
//! Uses #31 nonblocking parent pipes; process observation never performs an infinite wait.
use super::nonblocking::{self, Pipes};
use super::windows_wire::{MAX_ENV_UNITS, command_line, filtered_environment};
use std::{
    ffi::{OsStr, OsString, c_void},
    fs, io,
    marker::PhantomData,
    mem::size_of,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
        process::ExitStatusExt,
    },
    path::{Component, Path, PathBuf, Prefix},
    process::ExitStatus,
    ptr::{null, null_mut},
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_INSUFFICIENT_BUFFER, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    System::{
        Environment::{FreeEnvironmentStringsW, GetEnvironmentStringsW},
        JobObjects::{
            CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_PROCESS_ID_LIST,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
            JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
        Threading::{
            CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
            EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, InitializeProcThreadAttributeList,
            LPPROC_THREAD_ATTRIBUTE_LIST, OpenProcess, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROC_THREAD_ATTRIBUTE_JOB_LIST, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject,
        },
    },
};
use yu_engine_manager::ManagedEngineCommand;

#[cfg(test)]
#[path = "windows_spawn_tests.rs"]
mod tests;

fn wide_z(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut units: Vec<_> = value.encode_wide().collect();
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows startup value contains NUL",
        ));
    }
    units.push(0);
    Ok(units)
}

// Node's relative main-script lookup cannot use a verbatim current-directory spelling.
// Convert only drive/UNC prefixes and verify the ordinary spelling resolves to the same
// canonical directory. Never blindly strip prefixes from names whose semantics would change.
fn startup_directory(path: &Path) -> io::Result<Vec<u16>> {
    let canonical = fs::canonicalize(path)?;
    let units: Vec<_> = canonical.as_os_str().encode_wide().collect();
    let candidate = match canonical.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::VerbatimDisk(_) => PathBuf::from(OsString::from_wide(&units[4..])),
            Prefix::VerbatimUNC(_, _) => {
                let mut ordinary = vec![92, 92];
                ordinary.extend_from_slice(&units[8..]);
                PathBuf::from(OsString::from_wide(&ordinary))
            }
            Prefix::Disk(_) | Prefix::UNC(_, _) => canonical.clone(),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows engine working directory has no supported DOS/UNC spelling",
                ));
            }
        },
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows engine working directory is not absolute",
            ));
        }
    };
    for component in candidate.components() {
        if let Component::Normal(name) = component {
            let units: Vec<_> = name.encode_wide().collect();
            if matches!(units.last(), Some(32 | 46)) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows engine working-directory component ends with a dot or space",
                ));
            }
        }
    }
    if fs::canonicalize(&candidate)? != canonical {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ordinary Windows working-directory spelling changes the selected path",
        ));
    }
    wide_z(candidate.as_os_str())
}

pub(super) struct Containment {
    pub(super) job: OwnedHandle,
}

const MAX_CLEANUP_MEMBERS: usize = 128;
#[repr(C)]
struct ProcessList {
    assigned: u32,
    listed: u32,
    ids: [usize; MAX_CLEANUP_MEMBERS],
}
const _: () = assert!(
    std::mem::offset_of!(ProcessList, ids)
        == std::mem::offset_of!(JOBOBJECT_BASIC_PROCESS_ID_LIST, ProcessIdList)
);

pub(super) struct MemberSnapshot {
    handles: Vec<OwnedHandle>,
    total_processes: u32,
}
impl MemberSnapshot {
    pub(super) fn retained_count(&self) -> usize {
        self.handles.len()
    }
}

fn unchanged_membership(expected: u32, observed: u32) -> Result<(), String> {
    if expected != observed {
        Err("owned Job membership changed during cleanup; member completion is unconfirmed".into())
    } else {
        Ok(())
    }
}

impl Containment {
    fn create(name: Option<&str>) -> io::Result<Self> {
        let name = name.map(|s| wide_z(OsStr::new(s))).transpose()?;
        // SAFETY: optional name is NUL terminated and live; default security is non-inheritable.
        let raw = unsafe { CreateJobObjectW(null(), name.as_ref().map_or(null(), |v| v.as_ptr())) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: CreateJobObjectW returned a new owned reference, wrapped exactly once.
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: live Job and initialized information of exactly the supplied length.
        if unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { job })
    }

    fn accounting(&self) -> Result<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, String> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: live owned Job and correctly sized initialized accounting structure.
        if unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut c_void,
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                null_mut(),
            )
        } == 0
        {
            return Err(format!(
                "cannot confirm owned Job exit: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(info)
    }

    pub(super) fn is_empty(&self) -> Result<bool, String> {
        Ok(self.accounting()?.ActiveProcesses == 0)
    }

    fn retain_member(&self, pid: u32) -> Result<OwnedHandle, String> {
        // SAFETY: PID came from this private Job's bounded list. Query/synchronize only;
        // membership is checked on the SAME retained handle before it is ever waited on.
        let raw = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if raw.is_null() {
            return Err(format!(
                "cannot retain owned Job member {pid}: {}",
                io::Error::last_os_error()
            ));
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut belongs = 0;
        if unsafe {
            IsProcessInJob(
                handle.as_raw_handle(),
                self.job.as_raw_handle(),
                &mut belongs,
            )
        } == 0
        {
            return Err(format!(
                "cannot verify retained Job member {pid}: {}",
                io::Error::last_os_error()
            ));
        }
        if belongs == 0 {
            return Err(format!(
                "listed process {pid} no longer verifies in owned Job; completion unconfirmed"
            ));
        }
        Ok(handle)
    }

    pub(super) fn snapshot_members(
        &self,
        direct: &Child,
        deadline: Instant,
    ) -> Result<MemberSnapshot, String> {
        let total_processes = self.accounting()?.TotalProcesses;
        let mut list = ProcessList {
            assigned: 0,
            listed: 0,
            ids: [0; MAX_CLEANUP_MEMBERS],
        };
        // SAFETY: repr(C) header/array has the native layout, with fixed bounded capacity.
        // A truncated/error result is rejected rather than used as a complete snapshot.
        if unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
                JobObjectBasicProcessIdList,
                &mut list as *mut _ as *mut c_void,
                size_of::<ProcessList>() as u32,
                null_mut(),
            )
        } == 0
        {
            return Err(format!(
                "cannot snapshot owned Job members: {}",
                io::Error::last_os_error()
            ));
        }
        if list.listed != list.assigned || list.listed as usize > MAX_CLEANUP_MEMBERS {
            return Err("owned Job member snapshot exceeds fixed128-process bound".into());
        }
        let ids = &list.ids[..list.listed as usize];
        let mut handles = Vec::new();
        for (index, &raw_pid) in ids.iter().enumerate() {
            if Instant::now() >= deadline {
                return Err("cleanup deadline exceeded retaining owned Job members".into());
            }
            let pid = u32::try_from(raw_pid)
                .ok()
                .filter(|pid| *pid != 0)
                .ok_or("invalid process identifier in owned Job snapshot")?;
            if ids[..index].contains(&raw_pid) {
                return Err("duplicate process identifier in owned Job snapshot".into());
            }
            if pid != direct.id() {
                // The direct process already has an owned stable handle.
                handles.push(self.retain_member(pid)?);
            }
        }
        unchanged_membership(total_processes, self.accounting()?.TotalProcesses)?;
        Ok(MemberSnapshot {
            handles,
            total_processes,
        })
    }

    pub(super) fn members_complete(&self, snapshot: &MemberSnapshot) -> Result<bool, String> {
        let info = self.accounting()?;
        unchanged_membership(snapshot.total_processes, info.TotalProcesses)?;
        let mut complete = info.ActiveProcesses == 0;
        for handle in &snapshot.handles {
            // SAFETY: retained and previously membership-verified handle; zero-time wait.
            match unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => complete = false,
                _ => {
                    return Err(format!(
                        "cannot confirm retained Job member exit: {}",
                        io::Error::last_os_error()
                    ));
                }
            }
        }
        Ok(complete)
    }

    pub(super) fn terminate(&self) -> Result<(), String> {
        // SAFETY: this invocation owns the private Job; no unrelated process is targeted.
        if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
            Err(format!(
                "cannot terminate owned engine job: {}",
                io::Error::last_os_error()
            ))
        } else {
            Ok(())
        }
    }
}

/// Only nonblocking observation and owned termination; pipes belong to Running separately.
pub(super) struct Child {
    handle: OwnedHandle,
    pid: u32,
    status: Option<ExitStatus>,
}
impl AsRawHandle for Child {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle.as_raw_handle()
    }
}
impl Child {
    pub(super) fn id(&self) -> u32 {
        self.pid
    }
    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        // SAFETY: live owned process handle. Waiting does not infer identity from a reused PID.
        match unsafe { WaitForSingleObject(self.as_raw_handle(), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                // SAFETY: the process is signaled; output points to a valid u32.
                if unsafe { GetExitCodeProcess(self.as_raw_handle(), &mut code) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                let status = ExitStatus::from_raw(code);
                self.status = Some(status);
                Ok(Some(status)) // Even exit code 259 is an exit once the handle is signaled.
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
    pub(super) fn kill(&mut self) -> io::Result<()> {
        // SAFETY: only this invocation's owned process handle is terminated.
        if unsafe { TerminateProcess(self.as_raw_handle(), 1) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

struct AttributeList<'a> {
    storage: Vec<usize>, // Native pointer alignment; never resized after initialization.
    _values: PhantomData<&'a [HANDLE]>,
}
impl<'a> AttributeList<'a> {
    fn new(jobs: &'a [HANDLE], inherited: &'a [HANDLE]) -> io::Result<Self> {
        let mut bytes = 0;
        // SAFETY: documented sizing call; no list is accessed when pointer is null.
        let result = unsafe { InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes) };
        let error = io::Error::last_os_error();
        if result != 0
            || error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32)
            || bytes == 0
            || bytes > 64 * 1024
        {
            return Err(io::Error::other("cannot size Windows startup attributes"));
        }
        let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        // SAFETY: allocated buffer is aligned and contains at least the requested bytes.
        if unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 2, 0, &mut bytes)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut list = Self {
            storage,
            _values: PhantomData,
        };
        for (key, values) in [
            (PROC_THREAD_ATTRIBUTE_JOB_LIST, jobs),
            (PROC_THREAD_ATTRIBUTE_HANDLE_LIST, inherited),
        ] {
            // SAFETY: list is initialized; borrowed value arrays outlive list (including Drop).
            // The caller retains each referenced handle through the CreateProcessW call.
            if unsafe {
                UpdateProcThreadAttribute(
                    list.raw(),
                    0,
                    key as usize,
                    values.as_ptr().cast(),
                    std::mem::size_of_val(values),
                    null_mut(),
                    null(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(list)
    }
    fn raw(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for AttributeList<'_> {
    fn drop(&mut self) {
        // SAFETY: exactly one deletion of the initialized list before its storage is freed.
        unsafe {
            DeleteProcThreadAttributeList(self.raw());
        }
    }
}

fn inherited_environment() -> io::Result<Vec<u16>> {
    struct Environment(*mut u16);
    impl Drop for Environment {
        fn drop(&mut self) {
            // SAFETY: pointer came from GetEnvironmentStringsW and is freed once.
            unsafe {
                FreeEnvironmentStringsW(self.0);
            }
        }
    }
    // SAFETY: takes an OS-owned snapshot; does not mutate the calling process environment.
    let raw = unsafe { GetEnvironmentStringsW() };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    let block = Environment(raw);
    let mut units = Vec::new();
    for offset in 0..MAX_ENV_UNITS {
        // SAFETY: Windows guarantees a double-NUL-terminated block. Stop at that terminator.
        let unit = unsafe { *block.0.add(offset) };
        let last_zero = units.last() == Some(&0);
        units.push(unit);
        if unit == 0 && last_zero {
            return filtered_environment(&units);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Windows environment exceeds startup bound",
    ))
}

pub(super) fn spawn(
    installed: &ManagedEngineCommand,
) -> Result<(Child, Containment, Pipes), String> {
    let containment =
        Containment::create(None).map_err(|e| format!("cannot prepare private engine Job: {e}"))?;
    let (child, pipes) = spawn_in_job(installed, &containment.job)
        .map_err(|e| format!("cannot create engine in private Job (no fallback): {e}"))?;
    Ok((child, containment, pipes))
}

fn spawn_in_job(installed: &ManagedEngineCommand, job: &OwnedHandle) -> io::Result<(Child, Pipes)> {
    if !installed.entrypoint.is_absolute()
        || !installed.working_dir.is_absolute()
        || !installed
            .entrypoint
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows engine requires absolute native .exe and working directory paths",
        ));
    }
    let application = wide_z(installed.entrypoint.as_os_str())?;
    let cwd = startup_directory(&installed.working_dir)?;
    let args: Vec<Vec<u16>> = installed
        .args
        .iter()
        .map(|arg| arg.encode_utf16().collect())
        .collect();
    let mut command = command_line(&application[..application.len() - 1], &args)?;
    let environment = inherited_environment()?;
    let (stdin, input) = nonblocking::pair(false)?;
    let (stdout, output) = nonblocking::pair(true)?;
    let (stderr, error) = nonblocking::pair(true)?;
    let jobs = [job.as_raw_handle()];
    let inherited = [
        input.as_raw_handle(),
        output.as_raw_handle(),
        error.as_raw_handle(),
    ];
    for handle in inherited {
        // SAFETY: these are this invocation's live blocking client endpoints. The parent
        // NOWAIT endpoints and Job remain non-inheritable; HANDLE_LIST permits only these.
        if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) } == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let mut attributes = AttributeList::new(&jobs, &inherited)?;
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = inherited[0];
    startup.StartupInfo.hStdOutput = inherited[1];
    startup.StartupInfo.hStdError = inherited[2];
    startup.lpAttributeList = attributes.raw();
    let mut info = PROCESS_INFORMATION::default();
    // SAFETY: explicit executable and cwd, mutable terminated command, Unicode environment,
    // live attribute arrays/Job/std handles, correctly sized STARTUPINFOEXW and outputs.
    // Job assignment is part of creation. Never retry without attributes or assign after spawn.
    if unsafe {
        CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut info,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful creation returns exactly these owned process/thread references.
    // No fallible setup remains after creation; the private Job already owns the process.
    let handle = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
    let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
    drop(thread);
    drop(attributes);
    // Local child-side copies close before returning, so EOF cannot be held by this parent.
    Ok((
        Child {
            handle,
            pid: info.dwProcessId,
            status: None,
        },
        Pipes {
            stdin: Some(stdin),
            stdout: Some(stdout),
            stderr: Some(stderr),
        },
    ))
}
