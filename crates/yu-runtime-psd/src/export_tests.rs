use super::*;
fn png_file(path: &Path, color: png::ColorType) {
    let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), 1, 1);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(if color == png::ColorType::Rgba {
            &[1, 2, 3, 4]
        } else {
            &[1, 2, 3]
        })
        .unwrap();
    writer.finish().unwrap();
}
#[test]
fn validates_rgba8_dimensions_crc_and_complete_data() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("layer.png");
    png_file(&file, png::ColorType::Rgba);
    verify_png(&file, 1, 1).unwrap();
    assert!(verify_png(&file, 2, 1).is_err());
    assert!(verify_png(&file, u32::MAX, u32::MAX).is_err());
    assert!(pixel_bytes(0, 1).is_err());
    let valid = fs::read(&file).unwrap();
    fs::write(&file, &valid[..valid.len() - 8]).unwrap();
    assert!(verify_png(&file, 1, 1).is_err());
    let mut bad_crc = valid;
    bad_crc[29] ^= 1;
    fs::write(&file, bad_crc).unwrap();
    assert!(verify_png(&file, 1, 1).is_err());
    png_file(&file, png::ColorType::Rgb);
    assert!(verify_png(&file, 1, 1).is_err());
    fs::write(&file, b"bad png").unwrap();
    assert!(verify_png(&file, 1, 1).is_err());
}
#[test]
fn publish_never_clobbers_a_racing_writer() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("stage.png");
    let target = root.path().join("out.png");
    png_file(&file, png::ColorType::Rgba);
    assert_eq!(
        destination(&target).unwrap(),
        fs::canonicalize(root.path()).unwrap().join("out.png")
    );
    fs::write(&target, b"competitor").unwrap();
    assert_eq!(
        publish(&file, &target).unwrap_err().code,
        ErrorCode::OutputConflict
    );
    assert_eq!(fs::read(&target).unwrap(), b"competitor");
    fs::remove_file(&target).unwrap();
    publish(&file, &target).unwrap();
    verify_png(&target, 1, 1).unwrap();
}
#[test]
fn destination_rejects_conflicts_and_invalid_extensions() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("layer.png");
    fs::create_dir(&target).unwrap();
    assert_eq!(
        destination(&target).unwrap_err().code,
        ErrorCode::OutputConflict
    );
    assert!(destination(&root.path().join("layer.jpg")).is_err());
    assert!(destination(&root.path().join("missing/layer.png")).is_err());
}
#[cfg(unix)]
#[test]
fn symlink_artifacts_and_dangling_destinations_are_rejected() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("out.png");
    symlink(root.path().join("absent"), &target).unwrap();
    assert_eq!(
        destination(&target).unwrap_err().code,
        ErrorCode::OutputConflict
    );
    assert!(verify_png(&target, 1, 1).is_err());
}

fn selected_fixture(root: &Path) -> (ManagedEngineCommand, EngineDescriptor) {
    let command = ManagedEngineCommand {
        engine_id: ENGINE_ID.into(),
        version: "cleanup-fixture".into(),
        capabilities: vec![PSD_LAYER_EXPORT.into()],
        working_dir: root.to_owned(),
        entrypoint: std::env::current_exe().unwrap(),
        args: Vec::new(),
    };
    let selected = EngineDescriptor {
        state: EngineState::Ready,
        version: Some(command.version.clone()),
        capabilities: command.capabilities.clone(),
        ..descriptor()
    };
    (command, selected)
}

#[test]
fn cleanup_failure_prevents_export_publication_and_preserves_racing_output() {
    for racing_writer in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.psd");
        fs::write(&source, b"unchanged input").unwrap();
        let target = root.path().join("out.png");
        let destination = destination(&target).unwrap();
        let (mut command, selected) = selected_fixture(root.path());
        let result = export_selected_with(
            &mut command,
            &selected,
            canonical_input(&source).unwrap(),
            PsdLayerId::from_index(1).unwrap(),
            &destination,
            Duration::from_secs(30),
            |_, request, _| {
                let staged = Path::new(&request.payload.output_path);
                png_file(staged, png::ColorType::Rgba);
                verify_png(staged, 1, 1).unwrap();
                if racing_writer {
                    fs::write(&target, b"competitor").unwrap();
                }
                // A real test child exits normally; a real owned worker panics in cleanup.
                let error =
                    crate::process::cleanup_tests::failed_cleanup_after_success(root.path());
                Err(execution_error(error))
            },
        );
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::ExecutionFailed);
        assert!(error.message.contains("engine cleanup failed:"));
        assert!(error.engine.is_some());
        assert_eq!(fs::read(&source).unwrap(), b"unchanged input");
        if racing_writer {
            assert_eq!(fs::read(&target).unwrap(), b"competitor");
        } else {
            assert!(!target.exists());
        }
        assert!(!fs::read_dir(root.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".yu-psd-export-")
        }));
    }
}

#[test]
fn selected_export_still_publishes_after_successful_transport_settlement() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.psd");
    fs::write(&source, b"unchanged input").unwrap();
    let target = root.path().join("out.png");
    let (mut command, selected) = selected_fixture(root.path());
    let result = export_selected_with(
        &mut command,
        &selected,
        canonical_input(&source).unwrap(),
        PsdLayerId::from_index(1).unwrap(),
        &destination(&target).unwrap(),
        Duration::from_secs(30),
        |_, request, _| {
            png_file(
                Path::new(&request.payload.output_path),
                png::ColorType::Rgba,
            );
            Ok((
                serde_json::json!({"contract_version":"1", "layer_id":"L0001",
                "output_path":request.payload.output_path, "width":1, "height":1,
                "pixel_format":"rgba8", "container":"png"}),
                Vec::new(),
            ))
        },
    )
    .unwrap();
    assert_eq!(
        PathBuf::from(&result.result.output_path),
        fs::canonicalize(&target).unwrap()
    );
    verify_png(&target, 1, 1).unwrap();
    assert_eq!(fs::read(&source).unwrap(), b"unchanged input");
}
