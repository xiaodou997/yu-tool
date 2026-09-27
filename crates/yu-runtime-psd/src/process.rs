//! Bounded one-shot transport. Engines are trusted installed programs, not sandboxed code.
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use yu_engine_manager::ManagedEngineCommand;

pub(crate) const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub(crate) const MAX_STDOUT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_STDERR_BYTES: usize = 64 * 1024;

pub(crate) struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

enum Event {
    Input(Result<(), String>),
    Stdout(Result<Vec<u8>, String>),
    Stderr(Result<Vec<u8>, String>),
}

struct Running {
    child: Child,
    containment: Containment,
    workers: Vec<JoinHandle<()>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        // Close inherited pipes as well as the direct child before joining I/O workers.
        self.containment.terminate();
        let _ = self.child.kill();
        let _ = self.child.wait();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

pub(crate) fn execute(
    installed: &ManagedEngineCommand,
    request: Vec<u8>,
    timeout: Duration,
) -> Result<Output, String> {
    if request.len() > MAX_REQUEST_BYTES {
        return Err("external engine request exceeds 64 KiB".to_owned());
    }
    if timeout.is_zero() {
        return Err("external engine timeout must be greater than zero".to_owned());
    }
    let started = Instant::now();
    let mut command = Command::new(&installed.entrypoint);
    command
        .args(&installed.args)
        .current_dir(&installed.working_dir)
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_containment(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start engine: {e}"))?;
    let containment = match Containment::attach(&child) {
        Ok(value) => value,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let mut running = Running {
        child,
        containment,
        workers: Vec::new(),
    };
    let mut stdin = running
        .child
        .stdin
        .take()
        .ok_or("engine stdin is missing")?;
    let stdout = running
        .child
        .stdout
        .take()
        .ok_or("engine stdout is missing")?;
    let stderr = running
        .child
        .stderr
        .take()
        .ok_or("engine stderr is missing")?;
    let (sender, receiver) = mpsc::channel();
    let input_sender = sender.clone();
    running.workers.push(
        thread::Builder::new()
            .name("yu-psd-stdin".into())
            .spawn(move || {
                let result = stdin
                    .write_all(&request)
                    .map_err(|e| format!("engine stdin failed: {e}"));
                drop(stdin); // The protocol request is terminated by EOF, not a newline.
                let _ = input_sender.send(Event::Input(result));
            })
            .map_err(|e| format!("cannot start engine input worker: {e}"))?,
    );
    let stdout_sender = sender.clone();
    running.workers.push(
        thread::Builder::new()
            .name("yu-psd-stdout".into())
            .spawn(move || {
                let _ = stdout_sender.send(Event::Stdout(read_capped(
                    stdout,
                    MAX_STDOUT_BYTES,
                    "stdout",
                )));
            })
            .map_err(|e| format!("cannot start engine stdout worker: {e}"))?,
    );
    running.workers.push(
        thread::Builder::new()
            .name("yu-psd-stderr".into())
            .spawn(move || {
                let _ = sender.send(Event::Stderr(read_capped(
                    stderr,
                    MAX_STDERR_BYTES,
                    "stderr",
                )));
            })
            .map_err(|e| format!("cannot start engine stderr worker: {e}"))?,
    );

    let mut input_done = false;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        for event in receiver.try_iter() {
            match event {
                Event::Input(result) => {
                    result?;
                    input_done = true;
                }
                Event::Stdout(result) => stdout = Some(result?),
                Event::Stderr(result) => stderr = Some(result?),
            }
        }
        let status = running
            .child
            .try_wait()
            .map_err(|e| format!("cannot wait for engine: {e}"))?;
        if let Some(status) = status
            && input_done
            && stdout.is_some()
            && stderr.is_some()
        {
            let output = Output {
                stdout: stdout.take().unwrap(),
                stderr: stderr.take().unwrap(),
            };
            if !status.success() {
                return Err(format!(
                    "engine process failed ({status}): {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            return Ok(output);
        }
        if started.elapsed() >= timeout {
            return Err(format!("engine timed out after {} ms", timeout.as_millis()));
        }
        thread::sleep(Duration::from_millis(5).min(timeout.saturating_sub(started.elapsed())));
    }
}

fn read_capped(mut reader: impl Read, limit: usize, stream: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(result),
            Ok(count) => {
                if count > limit.saturating_sub(result.len()) {
                    return Err(format!("engine {stream} exceeds {limit} bytes"));
                }
                result.extend_from_slice(&buffer[..count]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("cannot read engine {stream}: {e}")),
        }
    }
}

#[cfg(unix)]
fn configure_containment(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn configure_containment(_: &mut Command) {}

#[cfg(unix)]
struct Containment {
    group: libc::pid_t,
}

#[cfg(unix)]
impl Containment {
    fn attach(child: &Child) -> Result<Self, String> {
        let group = libc::pid_t::try_from(child.id()).map_err(|_| "engine PID is out of range")?;
        if group <= 0 {
            return Err("engine PID is invalid".to_owned());
        }
        Ok(Self { group })
    }
    fn terminate(&self) {
        // SAFETY: spawn created a separate group with this positive child PID.
        unsafe {
            libc::kill(-self.group, libc::SIGKILL);
        }
    }
}

#[cfg(windows)]
struct Containment {
    job: std::os::windows::io::OwnedHandle,
}

#[cfg(windows)]
impl Containment {
    fn attach(child: &Child) -> Result<Self, String> {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };
        // SAFETY: null optional security/name pointers create an unnamed owned job.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            return Err(format!(
                "cannot create engine job: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: the successful CreateJobObjectW result is owned exactly once.
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        // SAFETY: this Windows POD structure admits zero initialization.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: pointers refer to valid handles/structures for the duration of each call.
        let configured = unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&limits) as u32,
            )
        };
        if configured == 0 {
            return Err(format!(
                "cannot configure engine job: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: both handles are live and owned by this invocation.
        if unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) } == 0 {
            return Err(format!(
                "cannot assign engine job: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self { job })
    }
    fn terminate(&self) {
        use std::os::windows::io::AsRawHandle;
        // SAFETY: the job handle is live and belongs only to this invocation.
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job.as_raw_handle(), 1);
        }
    }
}

#[cfg(not(any(unix, windows)))]
compile_error!("PSD managed process execution currently supports Unix and Windows only");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capped_reader_accepts_exact_limit_and_rejects_excess() {
        assert_eq!(read_capped(&b"1234"[..], 4, "stdout").unwrap(), b"1234");
        assert!(
            read_capped(&b"12345"[..], 4, "stdout")
                .unwrap_err()
                .contains("exceeds")
        );
    }
}
