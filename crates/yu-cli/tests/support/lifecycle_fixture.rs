//! Finite-lived native fixtures for process-exit, EOF and directory-ownership tests.
//! All files are inside the isolated test engine's working directory.
use std::{fs, io::Write, process::{Command, Stdio}, thread, time::{Duration, Instant}};
const READY: &str = ".yu-lifecycle-child-ready";

pub fn before_input(mode: &str) -> bool {
    if mode == "lifecycle-ready" { print!("fixture-ready"); return true; }
    if mode != "lifecycle-child" { return false; }
    fs::write(READY, std::process::id().to_string()).unwrap();
    thread::sleep(Duration::from_secs(20)); // Finite even if lifecycle cleanup regresses.
    true
}

pub fn respond(mode: &str, response: &str) -> bool {
    match mode {
        "lifecycle-live-parent" => {
            print!("{response}");
            std::io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(20));
        }
        "lifecycle-pipe-descendant" | "lifecycle-quiet-descendant" => {
            let inherited = mode == "lifecycle-pipe-descendant";
            let _child = Command::new(std::env::current_exe().unwrap())
                .arg("lifecycle-child")
                .stdin(Stdio::null())
                .stdout(if inherited { Stdio::inherit() } else { Stdio::null() })
                .stderr(if inherited { Stdio::inherit() } else { Stdio::null() })
                .spawn().unwrap();
            let start = Instant::now();
            // Inherited handles exist once spawn returns. Do not wait for child
            // user-code startup in the one-second EOF case; that would test startup,
            // not parent exit. The normal-deadline quiet-child case verifies readiness.
            while !inherited && !std::path::Path::new(READY).is_file() {
                assert!(start.elapsed() < Duration::from_secs(5), "child fixture failed to signal readiness");
                thread::sleep(Duration::from_millis(5));
            }
            print!("{response}");
            std::io::stdout().flush().unwrap();
        }
        "lifecycle-chunks" => {
            let middle = response.len() / 2; // Generated fixture response is ASCII.
            std::io::stdout().write_all(&response.as_bytes()[..middle]).unwrap();
            std::io::stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(50));
            std::io::stdout().write_all(&response.as_bytes()[middle..]).unwrap();
            std::io::stdout().flush().unwrap();
        }
        _ => return false,
    }
    true
}
