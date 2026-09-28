// Included by psd_managed; real export gates run only with a verified package.
use super::*;
use sha2::{Digest, Sha256};

fn export(root: &TempRoot, file: &Path, id: &str, out: &Path) -> std::process::Output {
    run(
        &root.0,
        &[
            "psd",
            "layer",
            "export",
            file.to_str().unwrap(),
            "--id",
            id,
            "-o",
            out.to_str().unwrap(),
            "--engine",
            "ag-psd",
            "--json",
        ],
    )
}
fn fingerprint(root: &TempRoot, file: &Path, label: &str) -> (usize, u64, String) {
    let hash_before = sha256_file(file).unwrap();
    let layers = command(&root.0, file, &["layer", "list"]);
    let mut digest = Sha256::new();
    let mut bytes = 0;
    let mut count = 0;
    for layer in layers["result"]["layers"].as_array().unwrap() {
        let id = layer["id"].as_str().unwrap();
        let out = root.0.join(format!("{label}-{id}.png"));
        if layer["kind"] == "group" {
            error(export(root, file, id, &out), 3, "UNSUPPORTED_CAPABILITY");
            assert!(!out.exists());
            continue;
        }
        let result = success(export(root, file, id, &out));
        assert_eq!(result["operation"], "psd.layer.export");
        assert_eq!(result["engine"]["id"], "ag-psd");
        assert_eq!(result["engine"]["provider"], "managed");
        assert_eq!(result["result"]["layer_id"], id);
        assert_eq!(result["result"]["pixel_format"], "rgba8");
        assert_eq!(result["result"]["container"], "png");
        assert_eq!(
            PathBuf::from(result["result"]["output_path"].as_str().unwrap()),
            fs::canonicalize(&out).unwrap()
        );
        assert!(!result["warnings"].as_array().unwrap().is_empty());
        let image = image::open(&out).unwrap();
        assert_eq!(image.color(), image::ColorType::Rgba8);
        assert_eq!(result["result"]["width"], image.width());
        assert_eq!(result["result"]["height"], image.height());
        let raw = image.into_rgba8().into_raw();
        digest.update(&raw);
        bytes += raw.len() as u64;
        count += 1;
        let png_hash = sha256_file(&out).unwrap();
        error(export(root, file, id, &out), 2, "OUTPUT_CONFLICT");
        assert_eq!(sha256_file(&out).unwrap(), png_hash);
    }
    assert_eq!(sha256_file(file).unwrap(), hash_before);
    (count, bytes, format!("{:x}", digest.finalize()))
}

#[test]
#[ignore = "requires YU_TEST_MANIFEST and YU_TEST_PACKAGE"]
fn managed_package_layer_export() {
    let manifest: EngineManifest =
        serde_json::from_slice(&fs::read(std::env::var("YU_TEST_MANIFEST").unwrap()).unwrap())
            .unwrap();
    let package = PathBuf::from(std::env::var("YU_TEST_PACKAGE").unwrap());
    assert_eq!(manifest.version, "31.0.2+node22.23.3.yu2");
    assert!(
        manifest
            .capabilities
            .iter()
            .any(|id| id == "psd.layer.export")
    );
    let root = TempRoot::new("export-managed");
    install(&root, &manifest, &package);
    activate(&root.0, &manifest.version);
    for (label, file) in [
        ("psd", fixture("upstream/psd-tools/2layers.psd")),
        ("psb", fixture("upstream/psd-tools/2layers.psb")),
        ("duplicates", fixture("derived/duplicate-layer-names.psd")),
    ] {
        assert_eq!(
            fingerprint(&root, &file, label),
            (
                2,
                37860,
                "bea0c17a1c85d0dfcaa95c7bd5f0df6184e3bc48c81e56078130d39af4e62062".into()
            )
        );
    }
    // Use the independently recorded psd-tools fingerprints, not hashes freshly produced by this adapter.
    let report: Value = serde_json::from_slice(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/data/psd-benchmark-report-v2.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut rgb8_checked = 0;
    let mut high_bit_rejected = 0;
    for workload in report["workloads"].as_array().unwrap() {
        let file = fixture("benchmark").join(workload["fixture_path"].as_str().unwrap());
        let header = fs::read(&file).unwrap();
        if u16::from_be_bytes([header[22], header[23]]) != 8 {
            let out = root.0.join("high-bit.png");
            error(
                export(&root, &file, "L0001", &out),
                3,
                "UNSUPPORTED_CAPABILITY",
            );
            assert!(!out.exists());
            high_bit_rejected += 1;
            continue;
        }
        let reference = workload["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["candidate_id"] == "psd-tools")
            .unwrap();
        let observed = fingerprint(&root, &file, workload["fixture_id"].as_str().unwrap());
        assert_eq!(
            observed.0 as u64,
            reference["exported_layer_count"].as_u64().unwrap()
        );
        assert_eq!(observed.1, reference["total_rgba_bytes"].as_u64().unwrap());
        assert_eq!(
            observed.2,
            reference["export_checksum_sha256"].as_str().unwrap()
        );
        rgb8_checked += 1;
    }
    assert_eq!(rgb8_checked, 5);
    assert_eq!(high_bit_rejected, 2);
    let group_file = fixture("upstream/psd-tools/group.psd");
    let groups = command(&root.0, &group_file, &["layer", "list"]);
    let group = groups["result"]["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "group")
        .unwrap();
    let out = root.0.join("unsupported.png");
    error(
        export(&root, &group_file, group["id"].as_str().unwrap(), &out),
        3,
        "UNSUPPORTED_CAPABILITY",
    );
    let simple = fixture("upstream/psd-tools/2layers.psd");
    error(export(&root, &simple, "L9999", &out), 2, "INVALID_ARGUMENT");
    error(
        export(
            &root,
            &fixture("malformed/truncated-header.psd"),
            "L0001",
            &out,
        ),
        2,
        "INVALID_INPUT",
    );
    assert!(!out.exists());
    assert!(!fs::read_dir(&root.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".yu-psd-export-")
    }));
    success(run(&root.0, &["engine", "deactivate", "ag-psd", "--json"]));
    success(run(
        &root.0,
        &["engine", "remove", "ag-psd", &manifest.version, "--json"],
    ));
    println!(
        "{}",
        serde_json::json!({"export":"rgba8/png", "simple_psd_psb_and_duplicates":3,
        "reference_fingerprint_workloads":rgb8_checked, "high_bit_rejections":high_bit_rejected,
        "target":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH}, "input_preserved":true})
    );
}
