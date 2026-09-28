//! Controlled failures exercise the real transport/cleanup path, not environment fault knobs.
use super::*;
use std::path::Path;

// The subprocess runs only this test, reads the actual request to EOF, then exits.
// A normal workspace test invocation returns immediately without touching stdin.
#[test]
fn fixture_child_waits_for_eof() {
    if !std::env::args().any(|argument| argument == "--exact") {
        return;
    }
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    println!("cleanup-fixture-response:{}", input.len());
}

fn fixture_command(directory: &Path) -> ManagedEngineCommand {
    ManagedEngineCommand {
        engine_id: "ag-psd".into(),
        version: "cleanup-fixture".into(),
        capabilities: vec!["psd.layer.export".into()],
        working_dir: directory.to_owned(),
        entrypoint: std::env::current_exe().unwrap(),
        args: vec![
            "--exact".into(),
            "process::cleanup_tests::fixture_child_waits_for_eof".into(),
            "--nocapture".into(),
        ],
    }
}

fn panic_worker(running: &mut Running) {
    running.workers.push(thread::spawn(|| {
        panic!("private-fixture-panic-payload");
    }));
}

pub(crate) fn failed_cleanup_after_success(directory: &Path) -> String {
    match execute_with_setup(
        &fixture_command(directory),
        b"{}".to_vec(),
        Duration::from_secs(30),
        panic_worker,
    ) {
        Ok(output) => panic!(
            "cleanup failure was swallowed after real child completion: {}",
            String::from_utf8_lossy(&output.stdout)
        ),
        Err(error) => error,
    }
}

#[test]
fn cleanup_failure_after_success_reaches_caller() {
    let root = tempfile::tempdir().unwrap();
    let error = failed_cleanup_after_success(root.path());
    assert!(error.contains("engine cleanup failed:"), "{error}");
    assert!(error.contains("I/O worker 0 panicked"), "{error}");
    assert!(!error.contains("private-fixture-panic-payload"), "{error}");
}

#[test]
fn cleanup_failure_is_appended_to_early_setup_error() {
    let root = tempfile::tempdir().unwrap();
    let result = execute_with_setup(
        &fixture_command(root.path()),
        b"{}".to_vec(),
        Duration::from_secs(30),
        |running| {
            panic_worker(running);
            drop(running.child.stdout.take());
        },
    );
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("missing output pipe unexpectedly succeeded"),
    };
    assert!(error.starts_with("engine stdout is missing"), "{error}");
    assert!(error.contains("engine cleanup failed:"), "{error}");
    assert!(error.contains("I/O worker 0 panicked"), "{error}");
}

#[test]
fn real_child_success_keeps_transport_output() {
    let root = tempfile::tempdir().unwrap();
    let output = execute(
        &fixture_command(root.path()),
        b"{}".to_vec(),
        Duration::from_secs(30),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("cleanup-fixture-response:2"));
}

#[test]
fn execution_and_cleanup_result_matrix() {
    assert_eq!(combine_execution_and_cleanup(Ok(42), Ok(())), Ok(42));
    assert_eq!(
        combine_execution_and_cleanup::<()>(Err("execution".into()), Ok(())),
        Err("execution".into())
    );
    assert_eq!(
        combine_execution_and_cleanup(Ok(42), Err("cleanup".into())),
        Err("engine cleanup failed: cleanup".into())
    );
    assert_eq!(
        combine_execution_and_cleanup::<()>(Err("execution".into()), Err("cleanup".into())),
        Err("execution; engine cleanup failed: cleanup".into())
    );
}

fn running_before_io(directory: &Path) -> Running {
    let fixture = fixture_command(directory);
    let (child, containment) = spawn_engine(&fixture).unwrap();
    Running {
        child,
        containment,
        workers: Vec::new(),
        cleanup_result: None,
        #[cfg(windows)]
        trace: None,
    }
}

#[test]
fn completed_cleanup_error_is_cached_and_workers_are_joined() {
    let root = tempfile::tempdir().unwrap();
    let mut running = running_before_io(root.path());
    panic_worker(&mut running);
    let first = running.cleanup();
    assert!(
        first
            .as_ref()
            .unwrap_err()
            .contains("I/O worker 0 panicked")
    );
    assert!(running.workers.is_empty());
    assert!(running.child.try_wait().unwrap().is_some());
    assert_eq!(running.cleanup_result, Some(first.clone()));
    assert_eq!(running.cleanup(), first);
}

#[test]
fn drop_fallback_joins_owned_workers_during_unwind() {
    use std::sync::atomic::AtomicBool;
    let root = tempfile::tempdir().unwrap();
    let joined = Arc::new(AtomicBool::new(false));
    let marker = Arc::clone(&joined);
    let mut running = running_before_io(root.path());
    running
        .workers
        .push(thread::spawn(move || marker.store(true, Ordering::SeqCst)));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owned = running;
        panic!("controlled unwind");
    }));
    assert!(result.is_err());
    assert!(joined.load(Ordering::SeqCst));
}
