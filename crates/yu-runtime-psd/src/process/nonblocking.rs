//! Nonblocking parent endpoints, ordinary blocking engine endpoints. No I/O workers.
#[cfg(unix)]
use std::process::Command;
use std::{
    fs::File,
    io::{self, Read, Write},
};

#[cfg(windows)]
#[path = "nonblocking_windows.rs"]
mod platform;

pub(super) struct Pipe(File);

#[cfg(unix)]
pub(super) fn pair(parent_reads: bool) -> io::Result<(Pipe, File)> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let mut fds = [-1; 2];
    // SAFETY: valid two-element output. Each successful descriptor is wrapped once.
    #[cfg(target_os = "linux")]
    let status = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    #[cfg(not(target_os = "linux"))]
    let status = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    let (read, write) = unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) };
    for file in [&read, &write] {
        // SAFETY: descriptor belongs to this pair. Child gets only explicitly passed stdio.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let (parent, child) = if parent_reads {
        (read, write)
    } else {
        (write, read)
    };
    // SAFETY: distinct pipe ends have independent file status flags; engine remains blocking.
    let flags = unsafe { libc::fcntl(parent.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(parent.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok((Pipe(parent), child))
}

#[cfg(windows)]
pub(super) fn pair(parent_reads: bool) -> io::Result<(Pipe, File)> {
    platform::pair(parent_reads)
}

impl Read for Pipe {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        #[cfg(unix)]
        {
            self.0.read(bytes)
        }
        #[cfg(windows)]
        {
            platform::read(&self.0, bytes)
        }
    }
}
impl Write for Pipe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        #[cfg(unix)]
        {
            self.0.write(bytes)
        }
        #[cfg(windows)]
        {
            platform::write(&self.0, bytes)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    } // No user-space buffer; never FlushFileBuffers.
}

pub(super) struct Pipes {
    pub(super) stdin: Option<Pipe>,
    pub(super) stdout: Option<Pipe>,
    pub(super) stderr: Option<Pipe>,
}
impl Pipes {
    #[cfg(unix)]
    pub(super) fn configure(command: &mut Command) -> io::Result<Self> {
        let (stdin, input) = pair(false)?;
        let (stdout, output) = pair(true)?;
        let (stderr, error) = pair(true)?;
        command.stdin(input).stdout(output).stderr(error);
        Ok(Self {
            stdin: Some(stdin),
            stdout: Some(stdout),
            stderr: Some(stderr),
        })
    }
    pub(super) fn close(&mut self) {
        // No outstanding overlapped operation or blocked thread references these endpoints.
        drop(self.stdin.take());
        drop(self.stdout.take());
        drop(self.stderr.take());
    }
    #[cfg(test)]
    pub(super) fn is_closed(&self) -> bool {
        self.stdin.is_none() && self.stdout.is_none() && self.stderr.is_none()
    }
}
