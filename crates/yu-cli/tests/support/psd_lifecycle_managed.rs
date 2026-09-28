//! Repeated actual-CLI lifecycle acceptance: fail on the first unsuccessful cycle.
use super::*;

fn phase(cycle: usize, action: &str) {
    println!(
        "{}",
        serde_json::json!({"test":"repeated_lifecycle", "cycle":cycle, "action":action, "event":"begin"})
    );
}

#[test]
#[ignore = "requires YU_TEST_MANIFEST and YU_TEST_PACKAGE"]
fn managed_package_repeated_lifecycle() {
    let manifest: EngineManifest =
        serde_json::from_slice(&fs::read(std::env::var("YU_TEST_MANIFEST").unwrap()).unwrap())
            .unwrap();
    let package = PathBuf::from(std::env::var_os("YU_TEST_PACKAGE").unwrap());
    let file = fixture("upstream/psd-tools/2layers.psd");
    let source_hash = sha256_file(&file).unwrap();
    let root = TempRoot::new("repeated-lifecycle");
    let mut observations = Vec::new();
    for cycle in 1..=10 {
        phase(cycle, "install");
        install(&root, &manifest, &package);
        phase(cycle, "activate");
        activate(&root.0, &manifest.version);
        phase(cycle, "inspect");
        assert_eq!(
            command(&root.0, &file, &["inspect"])["result"]["document"]["width"],
            101
        );
        phase(cycle, "engine.info");
        success(run(&root.0, &["engine", "info", "ag-psd", "--json"]));
        phase(cycle, "deactivate");
        success(run(&root.0, &["engine", "deactivate", "ag-psd", "--json"]));
        phase(cycle, "capabilities");
        success(run(&root.0, &["capabilities", "--json"]));
        phase(cycle, "remove");
        let output = run(
            &root.0,
            &["engine", "remove", "ag-psd", &manifest.version, "--json"],
        );
        assert!(
            output.status.success(),
            "cycle={cycle}, data_root={:?}, stderr={}",
            root.0,
            String::from_utf8_lossy(&output.stderr)
        );
        let removal = success(output);
        assert_eq!(
            removal["result"]["cleanup_complete"], true,
            "cycle={cycle}: {removal}"
        );
        assert!(
            !root
                .0
                .join("engines/ag-psd")
                .join(&manifest.version)
                .exists()
        );
        observations.push(removal["result"]["quarantine"].clone());
        assert_eq!(sha256_file(&file).unwrap(), source_hash);
        println!(
            "{}",
            serde_json::json!({"test":"repeated_lifecycle", "cycle":cycle, "event":"completed", "quarantine":removal["result"]["quarantine"]})
        );
    }
    println!(
        "{}",
        serde_json::json!({"schema_version":"1", "test":"repeated_lifecycle", "cycles":10, "failed_cycles":0, "os":std::env::consts::OS, "arch":std::env::consts::ARCH, "quarantine":observations})
    );
}
