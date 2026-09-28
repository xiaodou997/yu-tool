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
