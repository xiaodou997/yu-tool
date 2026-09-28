//! Live pipe/child controls: no external process is killed and no worker is detached.
use super::*;
use std::io::{self, Read, Write};

#[test]
fn empty_nonblocking_pipe_is_not_eof_until_peer_closes() {
    let (mut parent, mut peer) = nonblocking::pair(true).unwrap();
    let started = Instant::now();
    assert_eq!(
        parent.read(&mut [0; 16]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    peer.write_all(b"partial").unwrap();
    let mut bytes = [0; 16];
    assert_eq!(parent.read(&mut bytes).unwrap(), 7);
    assert_eq!(&bytes[..7], b"partial");
    assert_eq!(
        parent.read(&mut bytes).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    drop(peer);
    assert_eq!(parent.read(&mut bytes).unwrap(), 0);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn full_nonblocking_stdin_returns_backpressure_without_reader_progress() {
    let (mut writer, mut reader) = nonblocking::pair(false).unwrap();
    let started = Instant::now();
    let mut count = 0;
    loop {
        match writer.write(&[9; 4096]) {
            Ok(n) => {
                assert!(n > 0);
                count += n;
                assert!(count < 4 * 1024 * 1024);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            other => panic!("unexpected write: {other:?}"),
        }
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(count > 0);
    let mut bytes = vec![0; count];
    reader.read_exact(&mut bytes).unwrap();
    assert_eq!(bytes, vec![9; count]);
    assert!(writer.write(&[7]).unwrap() > 0);
}

fn result_error(result: Result<Output, String>) -> String {
    match result {
        Ok(_) => panic!("stalled I/O unexpectedly succeeded"),
        Err(error) => error,
    }
}

#[test]
fn held_stdout_writer_outside_job_cannot_keep_cleanup_waiting_for_eof() {
    let root = tempfile::tempdir().unwrap();
    let (reader, mut peer) = nonblocking::pair(true).unwrap();
    peer.write_all(b"partial-response").unwrap();
    let started = Instant::now();
    let error = result_error(execute_with_setup(
        &cleanup_tests::fixture_command(root.path()),
        b"{}".to_vec(),
        Duration::from_secs(1),
        move |running| {
            running.pipes.stdout = Some(reader);
        },
    ));
    assert!(error.contains("timed out after 1000 ms"), "{error}");
    assert!(error.contains("stdout_complete=false"), "{error}");
    // The test, not a terminable engine descendant, STILL holds the writer at return.
    assert!(started.elapsed() < Duration::from_secs(4), "{error}");
    let write = peer.write(b"x");
    assert!(
        write.is_err(),
        "local reader must have closed before returning"
    );
}

#[test]
fn held_stdin_reader_outside_job_cannot_keep_cleanup_waiting_for_a_write() {
    let root = tempfile::tempdir().unwrap();
    let (mut writer, _held_reader) = nonblocking::pair(false).unwrap();
    // Pipe capacity varies and may grow (macOS can accept the entire 64KiB request).
    // Establish actual backpressure before starting the bounded invocation.
    let mut filled = 0;
    loop {
        match writer.write(&[0; 4096]) {
            Ok(n) => {
                assert!(n > 0);
                filled += n;
                assert!(filled < 4 * 1024 * 1024);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            other => panic!("unexpected prefill: {other:?}"),
        }
    }
    let started = Instant::now();
    let error = result_error(execute_with_setup(
        &cleanup_tests::fixture_command(root.path()),
        vec![8; MAX_REQUEST_BYTES],
        Duration::from_secs(1),
        move |running| {
            running.pipes.stdin = Some(writer);
        },
    ));
    assert!(error.contains("timed out after 1000 ms"), "{error}");
    assert!(error.contains("stdin_complete=false"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(4), "{error}");
}

#[test]
fn live_child_wait_obeys_deadline_then_is_explicitly_cleaned() {
    let root = tempfile::tempdir().unwrap();
    let mut owned = spawn_running(&cleanup_tests::fixture_command(root.path())).unwrap();
    let started = Instant::now();
    // Child waits for stdin EOF. Observe only; intentionally do NOT request termination yet.
    let result = poll_until(
        started + Duration::from_millis(80),
        "controlled live child",
        || {
            owned
                .child
                .try_wait()
                .map(|v| v.is_some())
                .map_err(|e| e.to_string())
        },
    );
    assert!(result.unwrap_err().contains("cleanup deadline exceeded"));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(owned.child.try_wait().unwrap().is_none());
    owned.cleanup().unwrap();
    assert!(owned.pipes.is_closed());
    assert!(owned.child.try_wait().unwrap().is_some());
}

#[test]
fn exhausted_shared_deadline_is_not_restarted_for_a_later_resource() {
    let deadline = Instant::now() + Duration::from_millis(30);
    assert!(poll_until(deadline, "first", || Ok(false)).is_err());
    let started = Instant::now();
    assert!(poll_until(deadline, "second", || Ok(false)).is_err());
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(poll_until(deadline, "already complete", || Ok(true)).is_ok());
    assert_eq!(
        poll_until(deadline, "query", || Err("native query failure".into())).unwrap_err(),
        "native query failure"
    );
}

#[test]
fn capped_nonblocking_read_rejects_excess_without_appending_it() {
    let (reader, mut peer) = nonblocking::pair(true).unwrap();
    peer.write_all(b"12345").unwrap();
    let mut pipe = Some(reader);
    let mut bytes = Vec::new();
    assert!(
        read_step(&mut pipe, &mut bytes, 4, "stdout")
            .unwrap_err()
            .contains("exceeds 4")
    );
    assert!(bytes.is_empty());
}

#[test]
fn maximum_request_is_fully_written_before_input_eof() {
    let root = tempfile::tempdir().unwrap();
    let output = execute(
        &cleanup_tests::fixture_command(root.path()),
        vec![1; MAX_REQUEST_BYTES],
        Duration::from_secs(30),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("cleanup-fixture-response:65536"));
}
