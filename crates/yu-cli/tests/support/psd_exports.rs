use super::*;
fn setup(root: &TempRoot, mode: &str) -> PathBuf {
    let bitmap = root.0.join("fixture.png");
    image::RgbaImage::from_raw(2, 1, vec![255, 0, 127, 0, 13, 27, 89, 128])
        .unwrap()
        .save(&bitmap)
        .unwrap();
    provision_with_args(
        root,
        mode,
        "export-test",
        &["psd.layer.export"],
        &[bitmap.to_str().unwrap().into()],
    );
    activate(&root.0, "export-test");
    input(root)
}
fn export(root: &TempRoot, input: &Path, out: &Path) -> std::process::Output {
    export_with_timeout(root, input, out, "30")
}
fn export_with_timeout(
    root: &TempRoot,
    input: &Path,
    out: &Path,
    timeout: &str,
) -> std::process::Output {
    run(
        &root.0,
        &[
            "psd",
            "layer",
            "export",
            input.to_str().unwrap(),
            "--id",
            "L0001",
            "-o",
            out.to_str().unwrap(),
            "--timeout-secs",
            timeout,
            "--json",
        ],
    )
}
fn no_staging(root: &TempRoot) {
    assert!(!fs::read_dir(&root.0).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".yu-psd-export-")
    }));
}
#[test]
fn export_round_trip_preserves_pixels_and_never_overwrites() {
    let root = TempRoot::new("export-ok");
    let input = setup(&root, "export-ok");
    let input_hash = sha256_file(&input).unwrap();
    let out = root.0.join("selected.png");
    let value = success(export(&root, &input, &out));
    assert_eq!(value["operation"], "psd.layer.export");
    assert_eq!(value["engine"]["version"], "export-test");
    assert_eq!(value["result"]["layer_id"], "L0001");
    assert_eq!(value["result"]["pixel_format"], "rgba8");
    assert_eq!(
        image::open(&out).unwrap().into_rgba8().into_raw(),
        vec![255, 0, 127, 0, 13, 27, 89, 128]
    );
    let hash = sha256_file(&out).unwrap();
    error(export(&root, &input, &out), 2, "OUTPUT_CONFLICT");
    error(export(&root, &input, &input), 2, "OUTPUT_CONFLICT");
    assert_eq!(sha256_file(&input).unwrap(), input_hash);
    assert_eq!(sha256_file(&out).unwrap(), hash);
    no_staging(&root);
}
#[test]
fn failed_or_invalid_exports_never_publish_and_clean_staging() {
    for mode in [
        "export-partial",
        "export-garbage",
        "export-wrong-layer",
        "export-wrong-path",
        "export-wrong-size",
        "sleep",
    ] {
        let root = TempRoot::new(mode);
        let input = setup(&root, mode);
        let out = root.0.join("out.png");
        let timeout = if mode == "sleep" { "1" } else { "30" };
        let value = error(
            export_with_timeout(&root, &input, &out, timeout),
            1,
            "EXECUTION_FAILED",
        );
        if mode == "sleep" {
            assert!(
                value["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("timed out")
            );
        }
        assert_eq!(value["engine"]["version"], "export-test");
        assert!(!out.exists());
        no_staging(&root);
    }
}
#[test]
fn export_requires_an_explicitly_active_capable_version() {
    let root = TempRoot::new("export-unavailable");
    let input = input(&root);
    let out = root.0.join("out.png");
    error(export(&root, &input, &out), 3, "ENGINE_UNAVAILABLE");
    provision(&root, "ok", "old", &CAPS);
    activate(&root.0, "old");
    error(export(&root, &input, &out), 3, "ENGINE_INCOMPATIBLE");
    assert!(!psd_caps(&root).contains(&"psd.layer.export".into()));
    assert!(!out.exists());
    no_staging(&root);
}
