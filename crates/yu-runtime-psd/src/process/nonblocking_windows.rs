//! Synchronous byte pipes in PIPE_NOWAIT mode, deliberately polled, not overlapped I/O.
//! A completed call never retains a pointer to caller storage. Cancellation closes endpoints.
use super::Pipe;
use std::{
    fs::File,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED,
        GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
    },
    Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
    Storage::FileSystem::{
        CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING, PIPE_ACCESS_INBOUND,
        PIPE_ACCESS_OUTBOUND, ReadFile, WriteFile,
    },
    System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_NOWAIT, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    },
};

pub(super) fn pair(parent_reads: bool) -> io::Result<(Pipe, File)> {
    let mut nonce = [0u8; 16];
    // SAFETY: system RNG fills this bounded local buffer; no key or external resource.
    if unsafe {
        BCryptGenRandom(
            null_mut(),
            nonce.as_mut_ptr(),
            nonce.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } != 0
    {
        return Err(io::Error::other("cannot create private pipe name"));
    }
    let random: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let name: Vec<u16> = format!(r"\\.\pipe\yu-psd-{}-{random}", std::process::id())
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: unique local name, non-inheritable server, one instance, byte-oriented NOWAIT.
    // FIRST_PIPE_INSTANCE rejects collisions; REMOTE_CLIENTS are rejected, with no fallback.
    let raw = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            (if parent_reads {
                PIPE_ACCESS_INBOUND
            } else {
                PIPE_ACCESS_OUTBOUND
            }) | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            4096,
            4096,
            0,
            null(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful server handle is owned exactly once.
    let parent = unsafe { File::from_raw_handle(raw) };
    // SAFETY: open only our fresh pipe. Client is synchronous/blocking and non-inheritable;
    // std Command duplicates just the explicit stdio handle during process creation.
    let raw = unsafe {
        CreateFileW(
            name.as_ptr(),
            if parent_reads {
                GENERIC_WRITE
            } else {
                GENERIC_READ
            },
            0,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let child = unsafe { File::from_raw_handle(raw) };
    // SAFETY: live server handle. Client already connected; never wait for another client.
    if unsafe { ConnectNamedPipe(parent.as_raw_handle(), null_mut()) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32) {
            return Err(error);
        }
    }
    Ok((Pipe(parent), child))
}

pub(super) fn read(file: &File, bytes: &mut [u8]) -> io::Result<usize> {
    if bytes.is_empty() {
        return Ok(0);
    }
    let mut count = 0;
    // SAFETY: live NOWAIT server and bounded buffer, no OVERLAPPED/pending pointer lifetime.
    let ok = unsafe {
        ReadFile(
            file.as_raw_handle(),
            bytes.as_mut_ptr(),
            bytes.len().min(u32::MAX as usize) as u32,
            &mut count,
            null_mut(),
        )
    };
    if ok != 0 {
        // A successful zero-byte pipe transfer is not proof that the peer closed.
        return if count == 0 {
            Err(io::ErrorKind::WouldBlock.into())
        } else {
            Ok(count as usize)
        };
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error().map(|e| e as u32) {
        Some(ERROR_NO_DATA) => Err(io::ErrorKind::WouldBlock.into()),
        Some(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) => Ok(0),
        _ => Err(error),
    }
}

pub(super) fn write(file: &File, bytes: &[u8]) -> io::Result<usize> {
    if bytes.is_empty() {
        return Ok(0);
    }
    let mut count = 0;
    // SAFETY: synchronous NOWAIT byte pipe; actual partial counts are preserved.
    let ok = unsafe {
        WriteFile(
            file.as_raw_handle(),
            bytes.as_ptr(),
            bytes.len().min(u32::MAX as usize) as u32,
            &mut count,
            null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    if count == 0 {
        return Err(io::ErrorKind::WouldBlock.into());
    }
    Ok(count as usize)
}
