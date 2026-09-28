//! Creation-time private Job ownership on stable Rust, using documented Windows APIs.
//! Synchronous pipes/waits intentionally retain the existing cleanup policy.
use super::windows_wire::{MAX_ENV_UNITS, command_line, filtered_environment};
use std::{
    ffi::{OsStr, OsString, c_void},
    fs::{self, File},
    io::{self, Read},
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
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_BROKEN_PIPE, ERROR_INSUFFICIENT_BUFFER, HANDLE, HANDLE_FLAG_INHERIT,
        SetHandleInformation, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Security::SECURITY_ATTRIBUTES,
    System::{
        Environment::{FreeEnvironmentStringsW, GetEnvironmentStringsW},
        JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Pipes::CreatePipe,
        Threading::{
            CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
            EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE,
            InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST, PROCESS_INFORMATION,
            STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
            WaitForSingleObject,
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

pub(super) struct PipeReader(File);
impl Read for PipeReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self.0.read(bytes) {
            Err(error) if error.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) => Ok(0),
            result => result,
        }
    }
}

/// Minimal owned-process interface needed by the existing transport and cleanup code.
pub(super) struct Child {
    handle: OwnedHandle,
    pid: u32,
    status: Option<ExitStatus>,
    pub(super) stdin: Option<File>,
    pub(super) stdout: Option<PipeReader>,
    pub(super) stderr: Option<PipeReader>,
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
    fn observe(&mut self, milliseconds: u32) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        // SAFETY: live owned process handle. Waiting does not infer identity from a reused PID.
        match unsafe { WaitForSingleObject(self.as_raw_handle(), milliseconds) } {
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
    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.observe(0)
    }
    pub(super) fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        // Existing blocking wait policy; a bounded waiter is a separate follow-up.
        self.observe(INFINITE)?
            .ok_or_else(|| io::Error::other("unexpected Windows wait timeout"))
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

fn pipe(parent_reads: bool) -> io::Result<(File, OwnedHandle)> {
    let security = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let (mut read, mut write) = (null_mut(), null_mut());
    // SAFETY: outputs and security attributes are valid for this call.
    if unsafe { CreatePipe(&mut read, &mut write, &security, 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful call returns two distinct owned handles.
    let (read, write) = unsafe {
        (
            OwnedHandle::from_raw_handle(read),
            OwnedHandle::from_raw_handle(write),
        )
    };
    let (parent, child) = if parent_reads {
        (read, write)
    } else {
        (write, read)
    };
    // SAFETY: parent is live. Only the engine-side endpoint may be inherited.
    if unsafe { SetHandleInformation(parent.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((File::from(parent), child))
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

pub(super) fn spawn(installed: &ManagedEngineCommand) -> Result<(Child, Containment), String> {
    let containment =
        Containment::create(None).map_err(|e| format!("cannot prepare private engine Job: {e}"))?;
    let child = spawn_in_job(installed, &containment.job)
        .map_err(|e| format!("cannot create engine in private Job (no fallback): {e}"))?;
    Ok((child, containment))
}

fn spawn_in_job(installed: &ManagedEngineCommand, job: &OwnedHandle) -> io::Result<Child> {
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
    let (stdin, input) = pipe(false)?;
    let (stdout, output) = pipe(true)?;
    let (stderr, error) = pipe(true)?;
    let jobs = [job.as_raw_handle()];
    let inherited = [
        input.as_raw_handle(),
        output.as_raw_handle(),
        error.as_raw_handle(),
    ];
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
    Ok(Child {
        handle,
        pid: info.dwProcessId,
        status: None,
        stdin: Some(stdin),
        stdout: Some(PipeReader(stdout)),
        stderr: Some(PipeReader(stderr)),
    })
}
