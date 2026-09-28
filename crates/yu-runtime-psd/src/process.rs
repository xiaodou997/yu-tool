//! Bounded one-shot transport. Engines are trusted installed programs, not sandboxed code.
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    io::{Read, Write},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
use yu_engine_manager::ManagedEngineCommand;

#[cfg(test)]
mod bounded_tests;
mod nonblocking;
use nonblocking::{Pipe, Pipes};
const CLEANUP_BUDGET: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(5);

#[cfg(windows)]
mod windows_trace;

#[cfg(test)]
#[path = "process_cleanup_tests.rs"]
pub(super) mod cleanup_tests;

pub(crate) const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub(crate) const MAX_STDOUT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_STDERR_BYTES: usize = 64 * 1024;

pub(crate) struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

struct Running {
    child: Child,
    containment: Containment,
    pipes: Pipes,
    cleanup_result: Option<Result<(), String>>,
    #[cfg(test)]
    cleanup_fault: Option<String>,
    #[cfg(windows)]
    trace: Option<windows_trace::Trace>,
}

impl Running {
    fn finish<T>(&mut self, execution: Result<T, String>) -> Result<T, String> {
        combine_execution_and_cleanup(execution, self.cleanup())
    }

    fn cleanup(&mut self) -> Result<(), String> {
        if let Some(result) = &self.cleanup_result {
            return result.clone();
        }
        let started = Instant::now();
        let deadline = started + CLEANUP_BUDGET;
        #[cfg(windows)]
        if let Some(trace) = &self.trace {
            trace.record(
                &self.child,
                &self.containment.job,
                "cleanup_begin",
                serde_json::json!({"workers":0, "io_transport":"nonblocking_poll", "cleanup_budget_ms":CLEANUP_BUDGET.as_millis()}),
            );
        }
        // Cancel the pump before waiting for process exit. No outstanding read/write,
        // OVERLAPPED buffer or detached worker can retain these local endpoints.
        self.pipes.close();
        let mut errors = Vec::new();
        if let Err(error) = self.containment.terminate() {
            errors.push(error);
        }
        let wait_result = stop_child_until(&mut self.child, deadline);
        if let Err(error) = &wait_result {
            errors.push(error.clone());
        }
        #[cfg(windows)]
        let job_result = poll_until(deadline, "owned Job exit", || self.containment.is_empty());
        #[cfg(windows)]
        if let Err(error) = &job_result {
            errors.push(error.clone());
        }
        #[cfg(test)]
        if let Some(error) = &self.cleanup_fault {
            errors.push(error.clone());
        }
        #[cfg(windows)]
        if let Some(trace) = &self.trace {
            trace.record(
                &self.child,
                &self.containment.job,
                "cleanup_end",
                serde_json::json!({
                    "direct_wait_succeeded":wait_result.is_ok(), "workers_joined":0,
                    "io_transport":"nonblocking_poll", "io_endpoints_closed":true,
                    "job_empty_confirmed":job_result.is_ok(),
                    "cleanup_budget_ms":CLEANUP_BUDGET.as_millis(), "cleanup_elapsed_ms":started.elapsed().as_millis(),
                    "cleanup_succeeded":errors.is_empty(), "cleanup_errors":&errors,
                }),
            );
        }
        let result = if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        };
        // Both outcomes are final for this attempt; Drop must not retry or hide a failure.
        self.cleanup_result = Some(result.clone());
        result
    }
}

fn poll_until(
    deadline: Instant,
    label: &str,
    mut complete: impl FnMut() -> Result<bool, String>,
) -> Result<(), String> {
    loop {
        if complete()? {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!("cleanup deadline exceeded waiting for {label}"));
        }
        thread::sleep(POLL_INTERVAL.min(remaining));
    }
}

fn stop_child_until(child: &mut Child, deadline: Instant) -> Result<(), String> {
    let exited = child
        .try_wait()
        .map_err(|e| format!("cannot observe direct child: {e}"))?;
    if exited.is_some() {
        return Ok(());
    }
    let kill_error = child.kill().err();
    let result = poll_until(deadline, "direct child exit", || {
        child
            .try_wait()
            .map(|s| s.is_some())
            .map_err(|e| format!("cannot reap direct child: {e}"))
    });
    // A redundant TerminateProcess can race Job termination. Only a confirmed exit
    // within this same deadline resolves that error; the error is never blindly ignored.
    match (result, kill_error) {
        (Ok(()), _) => Ok(()),
        (Err(error), Some(kill)) => Err(format!("cannot terminate direct child: {kill}; {error}")),
        (Err(error), None) => Err(error),
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if self.cleanup_result.is_none() {
            // Fallback for unwinding only. Ordinary returns go through finish().
            let _ = self.cleanup();
        }
    }
}

fn combine_execution_and_cleanup<T>(
    execution: Result<T, String>,
    cleanup: Result<(), String>,
) -> Result<T, String> {
    match (execution, cleanup) {
        (result, Ok(())) => result,
        (Ok(_), Err(cleanup)) => Err(format!("engine cleanup failed: {cleanup}")),
        (Err(execution), Err(cleanup)) => {
            Err(format!("{execution}; engine cleanup failed: {cleanup}"))
        }
    }
}

pub(crate) fn execute(
    installed: &ManagedEngineCommand,
    request: Vec<u8>,
    timeout: Duration,
) -> Result<Output, String> {
    execute_with_setup(installed, request, timeout, |_| {})
}

// Private seam for controlled resource failures; no public option or environment switch.
fn execute_with_setup(
    installed: &ManagedEngineCommand,
    request: Vec<u8>,
    timeout: Duration,
    setup: impl FnOnce(&mut Running),
) -> Result<Output, String> {
    if request.len() > MAX_REQUEST_BYTES {
        return Err("external engine request exceeds 64 KiB".to_owned());
    }
    if timeout.is_zero() {
        return Err("external engine timeout must be greater than zero".to_owned());
    }
    let started = Instant::now();
    let mut running = spawn_running(installed)?;
    let spawn_elapsed_ms = started.elapsed().as_millis();
    setup(&mut running);
    let execution = exchange(&mut running, request, timeout, started, spawn_elapsed_ms);
    running.finish(execution)
}

fn spawn_running(installed: &ManagedEngineCommand) -> Result<Running, String> {
    let mut command = Command::new(&installed.entrypoint);
    command
        .args(&installed.args)
        .current_dir(&installed.working_dir)
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH");
    let mut pipes = Pipes::configure(&mut command)
        .map_err(|e| format!("cannot prepare cancellable engine I/O: {e}"))?;
    configure_containment(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start engine: {e}"))?;
    drop(command); // Release parent-side copies of the engine endpoints before waiting for EOF.
    let containment = match Containment::attach(&child) {
        Ok(value) => value,
        Err(error) => {
            pipes.close();
            return combine_execution_and_cleanup(
                Err(error),
                stop_child_until(&mut child, Instant::now() + CLEANUP_BUDGET),
            );
        }
    };
    #[cfg(windows)]
    let trace = windows_trace::Trace::start(&child, &containment.job, &installed.working_dir);
    Ok(Running {
        child,
        containment,
        pipes,
        cleanup_result: None,
        #[cfg(test)]
        cleanup_fault: None,
        #[cfg(windows)]
        trace,
    })
}

fn exchange(
    running: &mut Running,
    request: Vec<u8>,
    timeout: Duration,
    started: Instant,
    spawn_elapsed_ms: u128,
) -> Result<Output, String> {
    if running.pipes.stdin.is_none() {
        return Err("engine stdin is missing".into());
    }
    if running.pipes.stdout.is_none() {
        return Err("engine stdout is missing".into());
    }
    if running.pipes.stderr.is_none() {
        return Err("engine stderr is missing".into());
    }
    let mut written = 0;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    loop {
        let status = running
            .child
            .try_wait()
            .map_err(|e| format!("cannot wait for engine: {e}"))?;
        let input_done = running.pipes.stdin.is_none();
        let stdout_done = running.pipes.stdout.is_none();
        let stderr_done = running.pipes.stderr.is_none();
        if started.elapsed() >= timeout {
            // Snapshot before owned-process cleanup. Completion means EOF/event delivery,
            // not absence of partial bytes. Do not dump request or document contents.
            return Err(format!(
                "engine timed out after {} ms; transport diagnostic: pid={}, elapsed_ms={}, spawn_ms={}, child_exited={}, stdin_complete={}, stdout_complete={}, stderr_complete={}, phase={}, stdout_bytes={}, stderr_bytes={}",
                timeout.as_millis(),
                running.child.id(),
                started.elapsed().as_millis(),
                spawn_elapsed_ms,
                status.is_some(),
                input_done,
                stdout_done,
                stderr_done,
                wait_phase(status.is_some(), input_done, stdout_done, stderr_done),
                stdout.len(),
                stderr.len()
            ));
        }
        if let Some(status) = status
            && input_done
            && stdout_done
            && stderr_done
        {
            if !status.success() {
                return Err(format!(
                    "engine process failed ({status}): {}",
                    String::from_utf8_lossy(&stderr).trim()
                ));
            }
            return Ok(Output { stdout, stderr });
        }
        let mut progress = false;
        if let Some(stdin) = running.pipes.stdin.as_mut() {
            if written < request.len() {
                match stdin.write(&request[written..request.len().min(written + 8192)]) {
                    Ok(0) => return Err("engine stdin wrote zero bytes".into()),
                    Ok(count) => {
                        written += count;
                        progress = true;
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(format!("engine stdin failed: {e}")),
                }
            }
            if written == request.len() {
                drop(running.pipes.stdin.take());
                progress = true;
            }
        }
        // Exactly one bounded read per stream per turn prevents stderr/deadline starvation.
        progress |= read_step(
            &mut running.pipes.stdout,
            &mut stdout,
            MAX_STDOUT_BYTES,
            "stdout",
        )?;
        progress |= read_step(
            &mut running.pipes.stderr,
            &mut stderr,
            MAX_STDERR_BYTES,
            "stderr",
        )?;
        if !progress {
            thread::sleep(POLL_INTERVAL.min(timeout.saturating_sub(started.elapsed())));
        }
    }
}

fn read_step(
    pipe: &mut Option<Pipe>,
    output: &mut Vec<u8>,
    limit: usize,
    name: &str,
) -> Result<bool, String> {
    let Some(reader) = pipe.as_mut() else {
        return Ok(false);
    };
    let mut buffer = [0; 8192];
    match reader.read(&mut buffer) {
        Ok(0) => {
            drop(pipe.take());
            Ok(true)
        }
        Ok(count) => {
            if count > limit.saturating_sub(output.len()) {
                return Err(format!("engine {name} exceeds {limit} bytes"));
            }
            output.extend_from_slice(&buffer[..count]);
            Ok(true)
        }
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok(false)
        }
        Err(e) => Err(format!("cannot read engine {name}: {e}")),
    }
}

fn wait_phase(exited: bool, input: bool, stdout: bool, stderr: bool) -> &'static str {
    if !exited {
        "process_exit"
    } else if !input {
        "input_completion"
    } else if !stdout || !stderr {
        "pipe_eof"
    } else {
        "complete"
    }
}

#[cfg(test)]
fn read_capped(reader: impl Read, limit: usize, stream: &str) -> Result<Vec<u8>, String> {
    read_capped_observed(reader, limit, stream, &AtomicUsize::new(0))
}

#[cfg(test)]
fn read_capped_observed(
    mut reader: impl Read,
    limit: usize,
    stream: &str,
    progress: &AtomicUsize,
) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(result),
            Ok(count) => {
                // Counts only; never include document or response bytes in diagnostics.
                progress.fetch_add(count, Ordering::Relaxed);
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
    fn terminate(&self) -> Result<(), String> {
        // SAFETY: spawn created a separate group with this positive child PID.
        let result = unsafe { libc::kill(-self.group, libc::SIGKILL) };
        if result == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            // Normal completion can leave no remaining process group to terminate.
            Ok(())
        } else {
            Err(format!(
                "cannot terminate owned engine process group: {error}"
            ))
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
    fn is_empty(&self) -> Result<bool, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::{
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation,
            QueryInformationJobObject,
        };
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: live owned Job and correctly sized initialized accounting structure.
        if unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of_val(&info) as u32,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(format!(
                "cannot confirm owned Job exit: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(info.ActiveProcesses == 0)
    }
    fn terminate(&self) -> Result<(), String> {
        use std::os::windows::io::AsRawHandle;
        // SAFETY: the job handle is live and belongs only to this invocation.
        let result = unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job.as_raw_handle(), 1)
        };
        if result == 0 {
            Err(format!(
                "cannot terminate owned engine job: {}",
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(not(any(unix, windows)))]
compile_error!("PSD managed process execution currently supports Unix and Windows only");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_wait_phase_distinguishes_exit_from_eof() {
        assert_eq!(wait_phase(false, true, true, true), "process_exit");
        assert_eq!(wait_phase(true, false, true, true), "input_completion");
        assert_eq!(wait_phase(true, true, false, true), "pipe_eof");
        assert_eq!(wait_phase(true, true, true, false), "pipe_eof");
        assert_eq!(wait_phase(true, true, true, true), "complete");
    }

    #[test]
    fn lifecycle_pipe_progress_is_observed_without_retaining_extra_output() {
        let count = AtomicUsize::new(0);
        assert_eq!(
            read_capped_observed(&b"1234"[..], 4, "stdout", &count).unwrap(),
            b"1234"
        );
        assert_eq!(count.load(Ordering::Relaxed), 4);
        let count = AtomicUsize::new(0);
        assert!(read_capped_observed(&b"12345"[..], 4, "stdout", &count).is_err());
        assert_eq!(count.load(Ordering::Relaxed), 5);
    }
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
