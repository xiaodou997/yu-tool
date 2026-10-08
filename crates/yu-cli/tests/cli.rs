use image::{Rgba, RgbaImage};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};
use yu_engine_manager::{
    ArchiveKind, Downloader, EngineInstaller, EngineManager, EngineManifest, EnginePackage,
    EngineTarget, MANIFEST_SCHEMA_VERSION, ManagedLayout, ManagerError,
};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yu"))
        .args(args)
        .output()
        .expect("yu should execute")
}

fn run_with_data_home(args: &[&str], data_home: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yu"))
        .args(args)
        .env("YU_DATA_HOME", data_home)
        .output()
        .expect("yu should execute")
}

fn parse_stdout(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON")
}

fn temp_path(label: &str, extension: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "yu-cli-{label}-{}-{nonce}.{extension}",
        std::process::id()
    ))
}

fn create_png(path: &Path, width: u32, height: u32) {
    let image = RgbaImage::from_pixel(width, height, Rgba([32, 64, 128, 255]));
    image.save(path).expect("fixture should save");
}

#[test]
fn doctor_json_is_machine_readable() {
    let root = temp_path("doctor-data", "dir");
    fs::create_dir_all(&root).unwrap();
    let output = run_with_data_home(&["doctor", "--json"], &root);
    let json = parse_stdout(&output);

    assert_eq!(json["schema_version"], "1");
    assert_eq!(json["operation"], "runtime.doctor");
    assert!(json["result"]["engines"]["total"].as_u64().unwrap() >= 2);
    assert!(json["result"]["engines"]["ready"].as_u64().unwrap() >= 2);
    assert_eq!(json["result"]["engines"]["built_in"], 2);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn capabilities_json_contains_runtime_and_image_capabilities() {
    let output = run(&["capabilities", "--json"]);
    let json = parse_stdout(&output);
    let capabilities = json["result"]
        .as_array()
        .expect("result should be an array");

    assert!(
        capabilities
            .iter()
            .any(|item| item["id"] == "runtime.doctor")
    );
    assert!(
        capabilities
            .iter()
            .any(|item| item["id"] == "runtime.capabilities")
    );
    assert!(capabilities.iter().any(|item| item["id"] == "engine.list"));
    assert!(capabilities.iter().any(|item| item["id"] == "engine.info"));
    assert!(capabilities.iter().any(|item| item["id"] == "image.info"));
    assert!(capabilities.iter().any(|item| item["id"] == "image.resize"));
}

#[test]
fn engine_list_json_contains_built_in_runtime_and_raster_engine() {
    let output = run(&["engine", "list", "--json"]);
    let json = parse_stdout(&output);
    let engines = json["result"]
        .as_array()
        .expect("result should be an array");

    assert!(engines.len() >= 2);
    assert!(
        engines
            .iter()
            .any(|engine| engine["id"] == "yu-runtime" && engine["state"] == "ready")
    );
    assert!(
        engines
            .iter()
            .any(|engine| engine["id"] == "raster-rs" && engine["provider"] == "built_in")
    );
}

#[test]
fn engine_info_json_reports_built_in_provider() {
    let output = run(&["engine", "info", "raster-rs", "--json"]);
    let json = parse_stdout(&output);
    let entries = json["result"]
        .as_array()
        .expect("engine.info result should be an array");

    assert_eq!(json["operation"], "engine.info");
    assert!(entries.iter().any(|engine| engine["id"] == "raster-rs"
        && engine["provider"] == "built_in"
        && engine["state"] == "ready"));
}

#[test]
fn engine_info_unknown_returns_engine_unavailable() {
    let output = run(&["engine", "info", "does-not-exist", "--json"]);

    assert_eq!(output.status.code(), Some(3));
    let json: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be structured JSON");
    assert_eq!(json["error"]["code"], "ENGINE_UNAVAILABLE");
}

#[test]
fn image_info_json_reports_fixture_metadata() {
    let input = temp_path("info-input", "png");
    create_png(&input, 4, 2);

    let output = run(&["image", "info", input.to_str().unwrap(), "--json"]);
    let json = parse_stdout(&output);

    assert_eq!(json["operation"], "image.info");
    assert_eq!(json["engine"]["id"], "raster-rs");
    assert_eq!(json["result"]["format"], "png");
    assert_eq!(json["result"]["width"], 4);
    assert_eq!(json["result"]["height"], 2);
    assert_eq!(json["result"]["channels"], 4);
    assert_eq!(json["result"]["has_alpha"], true);

    let _ = fs::remove_file(input);
}

#[test]
fn image_resize_width_only_preserves_aspect_ratio() {
    let input = temp_path("resize-input", "png");
    let output_path = temp_path("resize-output", "png");
    create_png(&input, 4, 2);

    let output = run(&[
        "image",
        "resize",
        input.to_str().unwrap(),
        "--width",
        "2",
        "-o",
        output_path.to_str().unwrap(),
        "--json",
    ]);
    let json = parse_stdout(&output);

    assert_eq!(json["operation"], "image.resize");
    assert_eq!(json["engine"]["id"], "raster-rs");
    assert_eq!(json["result"]["source_width"], 4);
    assert_eq!(json["result"]["source_height"], 2);
    assert_eq!(json["result"]["width"], 2);
    assert_eq!(json["result"]["height"], 1);
    assert_eq!(json["result"]["dry_run"], false);

    let resized = image::open(&output_path).expect("resized image should decode");
    assert_eq!(resized.width(), 2);
    assert_eq!(resized.height(), 1);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output_path);
}

#[test]
fn image_resize_rejects_existing_output() {
    let input = temp_path("conflict-input", "png");
    let output_path = temp_path("conflict-output", "png");
    create_png(&input, 4, 2);
    create_png(&output_path, 1, 1);

    let output = run(&[
        "image",
        "resize",
        input.to_str().unwrap(),
        "--width",
        "2",
        "-o",
        output_path.to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(2));
    let json: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be structured JSON");
    assert_eq!(json["error"]["code"], "OUTPUT_CONFLICT");

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output_path);
}

#[test]
fn image_crop_selects_expected_pixels_and_reports_geometry() {
    let input = temp_path("crop-input", "png");
    let output_path = temp_path("crop-output", "png");
    let mut source = RgbaImage::new(3, 2);
    for y in 0..2 {
        for x in 0..3 {
            source.put_pixel(x, y, Rgba([x as u8 * 60, y as u8 * 90, 5, 255]));
        }
    }
    source.save(&input).unwrap();
    let original = fs::read(&input).unwrap();

    let result = run(&[
        "image",
        "crop",
        input.to_str().unwrap(),
        "--x",
        "1",
        "--y",
        "1",
        "--width",
        "2",
        "--height",
        "1",
        "-o",
        output_path.to_str().unwrap(),
        "--json",
    ]);
    let json = parse_stdout(&result);
    assert_eq!(json["operation"], "image.crop");
    assert_eq!(json["engine"]["id"], "raster-rs");
    assert_eq!(json["result"]["source_width"], 3);
    assert_eq!(json["result"]["width"], 2);
    assert_eq!(json["result"]["height"], 1);
    let cropped = image::open(&output_path).unwrap().to_rgba8();
    assert_eq!(*cropped.get_pixel(0, 0), *source.get_pixel(1, 1));
    assert_eq!(*cropped.get_pixel(1, 0), *source.get_pixel(2, 1));
    assert_eq!(fs::read(&input).unwrap(), original);
    fs::remove_file(input).unwrap();
    fs::remove_file(output_path).unwrap();
}

#[test]
fn image_crop_rejects_overflow_out_of_bounds_and_zero_geometry() {
    let input = temp_path("crop-bounds-input", "png");
    let output_path = temp_path("crop-bounds-output", "png");
    create_png(&input, 3, 2);
    for (x, y, width, height) in [
        ("2", "1", "2", "1"),
        ("4294967295", "0", "2", "1"),
        ("0", "0", "0", "1"),
        ("0", "0", "1", "0"),
    ] {
        let result = run(&[
            "image",
            "crop",
            input.to_str().unwrap(),
            "--x",
            x,
            "--y",
            y,
            "--width",
            width,
            "--height",
            height,
            "-o",
            output_path.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(result.status.code(), Some(2));
        let json: Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(json["error"]["code"], "INVALID_INPUT");
        assert!(!output_path.exists());
    }
    fs::remove_file(input).unwrap();
}

#[test]
fn image_rotate_90_is_clockwise_and_preserves_source() {
    let input = temp_path("rotate-input", "png");
    let output_path = temp_path("rotate-output", "png");
    let mut source = RgbaImage::new(3, 2);
    for y in 0..2 {
        for x in 0..3 {
            source.put_pixel(x, y, Rgba([(10 + x + y * 3) as u8, 0, 0, 255]));
        }
    }
    source.save(&input).unwrap();
    let original = fs::read(&input).unwrap();
    let result = run(&[
        "image",
        "rotate",
        input.to_str().unwrap(),
        "--degrees",
        "90",
        "-o",
        output_path.to_str().unwrap(),
        "--json",
    ]);
    let json = parse_stdout(&result);
    assert_eq!(json["operation"], "image.rotate");
    assert_eq!(json["result"]["degrees"], 90);
    assert_eq!(json["result"]["width"], 2);
    assert_eq!(json["result"]["height"], 3);
    let rotated = image::open(&output_path).unwrap().to_rgba8();
    assert_eq!(*rotated.get_pixel(0, 0), *source.get_pixel(0, 1));
    assert_eq!(*rotated.get_pixel(1, 0), *source.get_pixel(0, 0));
    assert_eq!(fs::read(&input).unwrap(), original);
    fs::remove_file(input).unwrap();
    fs::remove_file(output_path).unwrap();
}

#[test]
fn image_rotate_rejects_non_right_angle() {
    let input = temp_path("rotate-invalid-input", "png");
    let output_path = temp_path("rotate-invalid-output", "png");
    create_png(&input, 3, 2);
    let result = run(&[
        "image",
        "rotate",
        input.to_str().unwrap(),
        "--degrees",
        "45",
        "-o",
        output_path.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(json["error"]["code"], "INVALID_INPUT");
    assert!(!output_path.exists());
    fs::remove_file(input).unwrap();
}

#[test]
fn image_rotate_180_and_270_preserve_pixel_positions() {
    let input = temp_path("rotate-other-input", "png");
    let mut source = RgbaImage::new(3, 2);
    for y in 0..2 {
        for x in 0..3 {
            source.put_pixel(x, y, Rgba([(x + 1 + y * 3) as u8, 0, 0, 255]));
        }
    }
    source.save(&input).unwrap();
    for (degrees, expected_width, expected_height, first_pixel) in [
        ("180", 3, 2, *source.get_pixel(2, 1)),
        ("270", 2, 3, *source.get_pixel(2, 0)),
    ] {
        let output_path = temp_path("rotate-other-output", "png");
        let result = run(&[
            "image",
            "rotate",
            input.to_str().unwrap(),
            "--degrees",
            degrees,
            "-o",
            output_path.to_str().unwrap(),
            "--json",
        ]);
        let json = parse_stdout(&result);
        assert_eq!(json["result"]["width"], expected_width);
        assert_eq!(json["result"]["height"], expected_height);
        let rotated = image::open(&output_path).unwrap().to_rgba8();
        assert_eq!(*rotated.get_pixel(0, 0), first_pixel);
        fs::remove_file(output_path).unwrap();
    }
    fs::remove_file(input).unwrap();
}

#[test]
fn image_convert_supports_png_to_jpeg_and_webp() {
    let input = temp_path("convert-input", "png");
    create_png(&input, 3, 2);
    let original = fs::read(&input).unwrap();
    for (extension, format) in [("jpg", "jpeg"), ("webp", "webp")] {
        let output_path = temp_path("convert-output", extension);
        let result = run(&[
            "image",
            "convert",
            input.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--json",
        ]);
        let json = parse_stdout(&result);
        assert_eq!(json["operation"], "image.convert");
        assert_eq!(json["result"]["source_format"], "png");
        assert_eq!(json["result"]["format"], format);
        let converted = image::open(&output_path).unwrap();
        assert_eq!((converted.width(), converted.height()), (3, 2));
        assert_eq!(fs::read(&input).unwrap(), original);
        fs::remove_file(output_path).unwrap();
    }
    fs::remove_file(input).unwrap();
}

#[test]
fn image_convert_refuses_existing_destination_and_unknown_format() {
    let input = temp_path("convert-guard-input", "png");
    let output_path = temp_path("convert-guard-output", "webp");
    create_png(&input, 3, 2);
    create_png(&output_path.with_extension("png"), 1, 1);
    fs::write(&output_path, b"do not replace").unwrap();
    let result = run(&[
        "image",
        "convert",
        input.to_str().unwrap(),
        "-o",
        output_path.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(json["error"]["code"], "OUTPUT_CONFLICT");
    assert_eq!(fs::read(&output_path).unwrap(), b"do not replace");
    let unsupported = temp_path("convert-unknown", "gif");
    let result = run(&[
        "image",
        "convert",
        input.to_str().unwrap(),
        "-o",
        unsupported.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(3));
    assert!(!unsupported.exists());
    fs::remove_file(input).unwrap();
    fs::remove_file(&output_path).unwrap();
    fs::remove_file(output_path.with_extension("png")).unwrap();
}

#[test]
fn image_dry_run_previews_all_mutations_without_writing() {
    let input = temp_path("dry-run-input", "png");
    let destination = temp_path("dry-run-output", "webp");
    create_png(&input, 6, 4);
    let original = fs::read(&input).unwrap();
    let source = input.to_str().unwrap();
    let output = destination.to_str().unwrap();
    let cases = [
        (
            vec![
                "image",
                "resize",
                source,
                "--width",
                "3",
                "-o",
                output,
                "--dry-run",
                "--json",
            ],
            "image.resize",
            (3, 2),
        ),
        (
            vec![
                "image",
                "crop",
                source,
                "--x",
                "1",
                "--y",
                "0",
                "--width",
                "3",
                "--height",
                "2",
                "-o",
                output,
                "--dry-run",
                "--json",
            ],
            "image.crop",
            (3, 2),
        ),
        (
            vec![
                "image",
                "rotate",
                source,
                "--degrees",
                "270",
                "-o",
                output,
                "--dry-run",
                "--json",
            ],
            "image.rotate",
            (4, 6),
        ),
        (
            vec![
                "image",
                "convert",
                source,
                "-o",
                output,
                "--dry-run",
                "--json",
            ],
            "image.convert",
            (6, 4),
        ),
    ];
    for (args, operation, (width, height)) in cases {
        let reply = parse_stdout(&run(&args));
        assert_eq!(reply["operation"], operation);
        assert_eq!(reply["engine"]["id"], "raster-rs");
        assert_eq!(reply["result"]["dry_run"], true);
        assert_eq!(reply["result"]["width"], width);
        assert_eq!(reply["result"]["height"], height);
        assert_eq!(reply["result"]["format"], "webp");
        assert_eq!(reply["result"]["output_receipt"]["status"], "planned");
        assert!(
            reply["result"]["output_receipt"]
                .get("verified_output")
                .is_none()
        );
        assert!(
            reply["result"]["output_receipt"]
                .get("previous_sha256")
                .is_none()
        );
        assert!(
            !destination.exists(),
            "dry-run created output for {operation}"
        );
        assert_eq!(fs::read(&input).unwrap(), original);
    }
    fs::remove_file(input).unwrap();
}

#[test]
fn image_output_receipts_are_derived_from_real_published_files() {
    let input = temp_path("verify-output-input", "png");
    create_png(&input, 6, 4);
    let original = fs::read(&input).unwrap();
    let cases = [
        ("resize", vec!["--width", "3"], "png", (3, 2)),
        (
            "crop",
            vec!["--x", "1", "--y", "1", "--width", "3", "--height", "2"],
            "jpg",
            (3, 2),
        ),
        ("rotate", vec!["--degrees", "90"], "webp", (4, 6)),
        ("convert", vec![], "jpg", (6, 4)),
    ];
    for (name, arguments, extension, dimensions) in cases {
        let output_path = temp_path("verify-output", extension);
        let mut args = vec!["image", name, input.to_str().unwrap()];
        args.extend_from_slice(&arguments);
        args.extend_from_slice(&["-o", output_path.to_str().unwrap(), "--json"]);
        let json = parse_stdout(&run(&args));
        assert_eq!(json["result"]["dry_run"], false);
        assert_eq!(json["result"]["replaced"], false);
        let receipt = &json["result"]["output_receipt"];
        assert_eq!(receipt["status"], "verified");
        assert!(receipt.get("previous_sha256").is_none());
        let file = fs::read(&output_path).unwrap();
        let verified = &receipt["verified_output"];
        assert_eq!(verified["sha256"], fixture_sha256(&file));
        assert_eq!(verified["bytes"], file.len() as u64);
        assert_eq!(
            verified["format"],
            if extension == "jpg" {
                "jpeg"
            } else {
                extension
            }
        );
        assert_eq!(verified["width"], dimensions.0);
        assert_eq!(verified["height"], dimensions.1);
        let reopened = image::open(&output_path).unwrap();
        assert_eq!((reopened.width(), reopened.height()), dimensions);
        assert_eq!(fs::read(&input).unwrap(), original);
        fs::remove_file(output_path).unwrap();
    }
    fs::remove_file(input).unwrap();
}

#[test]
fn image_dry_run_checks_existing_destination_and_invalid_geometry() {
    let input = temp_path("dry-guard-input", "png");
    let destination = temp_path("dry-guard-output", "png");
    create_png(&input, 6, 4);
    fs::write(&destination, b"unchanged").unwrap();

    let reply = run(&[
        "image",
        "resize",
        input.to_str().unwrap(),
        "--width",
        "3",
        "-o",
        destination.to_str().unwrap(),
        "--dry-run",
        "--json",
    ]);
    assert_eq!(reply.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&reply.stderr).unwrap();
    assert_eq!(error["error"]["code"], "OUTPUT_CONFLICT");
    assert_eq!(fs::read(&destination).unwrap(), b"unchanged");
    fs::remove_file(&destination).unwrap();

    let reply = run(&[
        "image",
        "crop",
        input.to_str().unwrap(),
        "--x",
        "5",
        "--y",
        "0",
        "--width",
        "3",
        "--height",
        "1",
        "-o",
        destination.to_str().unwrap(),
        "--dry-run",
        "--json",
    ]);
    assert_eq!(reply.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&reply.stderr).unwrap();
    assert_eq!(error["error"]["code"], "INVALID_INPUT");
    assert!(!destination.exists());
    fs::remove_file(input).unwrap();
}

#[test]
fn image_dry_run_rejects_missing_parent_before_claiming_success() {
    let input = temp_path("dry-parent-input", "png");
    let destination = temp_path("dry-missing-dir", "dir").join("out.webp");
    create_png(&input, 2, 2);
    let reply = run(&[
        "image",
        "convert",
        input.to_str().unwrap(),
        "-o",
        destination.to_str().unwrap(),
        "--dry-run",
        "--json",
    ]);
    assert_eq!(reply.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&reply.stderr).unwrap();
    assert_eq!(error["error"]["code"], "INVALID_INPUT");
    assert!(!destination.exists());
    fs::remove_file(input).unwrap();
}

#[cfg(unix)]
#[test]
fn image_dry_run_treats_dangling_output_symlink_as_conflict() {
    use std::os::unix::fs::symlink;

    let input = temp_path("dry-symlink-input", "png");
    let destination = temp_path("dry-symlink-output", "webp");
    let nonexistent = temp_path("dry-symlink-target", "webp");
    create_png(&input, 2, 2);
    symlink(&nonexistent, &destination).unwrap();

    let reply = run(&[
        "image",
        "convert",
        input.to_str().unwrap(),
        "-o",
        destination.to_str().unwrap(),
        "--dry-run",
        "--json",
    ]);
    assert_eq!(reply.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&reply.stderr).unwrap();
    assert_eq!(error["error"]["code"], "OUTPUT_CONFLICT");
    assert_eq!(fs::read_link(&destination).unwrap(), nonexistent);

    fs::remove_file(input).unwrap();
    fs::remove_file(destination).unwrap();
}

#[test]
fn image_replace_requires_explicit_flag_and_sha256_argument() {
    let input = temp_path("replace-args-input", "png");
    let output_path = temp_path("replace-args-output", "png");
    create_png(&input, 4, 2);
    let hash = fixture_sha256(b"old");
    for extra in [
        vec!["--replace"],
        vec!["--expected-output-sha256", hash.as_str()],
    ] {
        let mut args = vec![
            "image",
            "convert",
            input.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
        ];
        args.extend(extra);
        args.push("--json");
        let result = run(&args);
        assert_eq!(result.status.code(), Some(2));
        let error: Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(error["error"]["code"], "INVALID_ARGUMENT");
        assert!(!output_path.exists());
    }
    fs::remove_file(input).unwrap();
}

#[test]
fn image_replace_valid_sha256_can_replace_with_all_four_operations() {
    let input = temp_path("replace-valid-input", "png");
    create_png(&input, 4, 2);
    let source = fs::read(&input).unwrap();
    for (name, extra, width, height) in [
        ("resize", vec!["--width", "2"], 2, 1),
        (
            "crop",
            vec!["--x", "1", "--y", "0", "--width", "2", "--height", "1"],
            2,
            1,
        ),
        ("rotate", vec!["--degrees", "90"], 2, 4),
        ("convert", vec![], 4, 2),
    ] {
        let destination = temp_path("replace-valid-output", "png");
        create_png(&destination, 1, 1);
        let before = fs::read(&destination).unwrap();
        let hash = fixture_sha256(&before);
        let mut args = vec!["image", name, input.to_str().unwrap()];
        args.extend_from_slice(&extra);
        args.extend_from_slice(&[
            "-o",
            destination.to_str().unwrap(),
            "--replace",
            "--expected-output-sha256",
            hash.as_str(),
            "--json",
        ]);
        let output = run(&args);
        let json = parse_stdout(&output);
        assert_eq!(json["result"]["replaced"], true);
        assert_eq!(json["result"]["would_replace"], false);
        assert_eq!(json["result"]["dry_run"], false);
        assert_eq!(json["result"]["width"], width);
        assert_eq!(json["result"]["height"], height);
        assert_eq!(json["result"]["output_receipt"]["status"], "verified");
        assert_eq!(json["result"]["output_receipt"]["previous_sha256"], hash);
        let decoded = image::open(&destination).unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            (width as u32, height as u32)
        );
        assert_eq!(fs::read(&input).unwrap(), source);
        let verified = &json["result"]["output_receipt"]["verified_output"];
        let bytes = fs::read(&destination).unwrap();
        assert_eq!(verified["sha256"], fixture_sha256(&bytes));
        assert_eq!(verified["bytes"], bytes.len() as u64);
        assert_eq!(verified["format"], "png");
        assert_eq!(verified["width"], width);
        assert_eq!(verified["height"], height);
        let sidecar = destination.with_file_name(format!(
            ".{}.yu-replace.lock",
            destination.file_name().unwrap().to_string_lossy()
        ));
        assert!(!sidecar.exists());
        fs::remove_file(destination).unwrap();
    }
    fs::remove_file(input).unwrap();
}

#[test]
fn image_replace_dry_run_only_plans_and_preserves_existing_bytes() {
    let input = temp_path("replace-plan-input", "png");
    let output_path = temp_path("replace-plan-output", "webp");
    create_png(&input, 4, 2);
    fs::write(&output_path, b"old-preview-file").unwrap();
    let original = fs::read(&output_path).unwrap();
    let hash = fixture_sha256(&original).to_uppercase();
    let output = run(&[
        "image",
        "convert",
        input.to_str().unwrap(),
        "-o",
        output_path.to_str().unwrap(),
        "--replace",
        "--expected-output-sha256",
        &hash,
        "--dry-run",
        "--json",
    ]);
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "image.convert");
    assert_eq!(json["result"]["replaced"], false);
    assert_eq!(json["result"]["would_replace"], true);
    assert_eq!(json["result"]["dry_run"], true);
    assert_eq!(json["result"]["output_receipt"]["status"], "planned");
    assert_eq!(
        json["result"]["output_receipt"]["previous_sha256"],
        hash.to_ascii_lowercase()
    );
    assert!(
        json["result"]["output_receipt"]
            .get("verified_output")
            .is_none()
    );
    assert_eq!(fs::read(&output_path).unwrap(), original);
    fs::remove_file(input).unwrap();
    fs::remove_file(output_path).unwrap();
}

#[test]
fn image_replace_rejects_stale_hash_missing_destination_and_bad_hash() {
    let input = temp_path("replace-reject-input", "png");
    let output_path = temp_path("replace-reject-output", "png");
    create_png(&input, 4, 2);
    fs::write(&output_path, b"newer-version").unwrap();
    let original = fs::read(&output_path).unwrap();
    let stale_hash = fixture_sha256(b"older-version");
    let valid_hash = fixture_sha256(&original);
    let bad = "not-a-sha256";
    for (hash, expected_error) in [
        (stale_hash.as_str(), "OUTPUT_CONFLICT"),
        (bad, "INVALID_INPUT"),
    ] {
        let reply = run(&[
            "image",
            "convert",
            input.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--replace",
            "--expected-output-sha256",
            hash,
            "--json",
        ]);
        assert_eq!(reply.status.code(), Some(2));
        let error: Value = serde_json::from_slice(&reply.stderr).unwrap();
        assert_eq!(error["error"]["code"], expected_error);
        assert_eq!(fs::read(&output_path).unwrap(), original);
    }
    fs::remove_file(&output_path).unwrap();
    let reply = run(&[
        "image",
        "convert",
        input.to_str().unwrap(),
        "-o",
        output_path.to_str().unwrap(),
        "--replace",
        "--expected-output-sha256",
        &valid_hash,
        "--json",
    ]);
    assert_eq!(reply.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&reply.stderr).unwrap();
    assert_eq!(error["error"]["code"], "OUTPUT_CONFLICT");
    assert!(!output_path.exists());
    fs::remove_file(input).unwrap();
}

#[test]
fn image_replace_refuses_directory_even_when_named_as_png() {
    let input = temp_path("replace-directory-input", "png");
    let target = temp_path("replace-directory-output", "png");
    create_png(&input, 4, 2);
    fs::create_dir(&target).unwrap();
    let hash = fixture_sha256(b"does-not-matter");
    let reply = run(&[
        "image",
        "convert",
        input.to_str().unwrap(),
        "-o",
        target.to_str().unwrap(),
        "--replace",
        "--expected-output-sha256",
        &hash,
        "--json",
    ]);
    assert_eq!(reply.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&reply.stderr).unwrap();
    assert_eq!(error["error"]["code"], "INVALID_INPUT");
    assert!(target.is_dir());
    fs::remove_file(input).unwrap();
    fs::remove_dir(target).unwrap();
}

#[test]
fn image_replace_allows_in_place_only_with_explicit_version_guard() {
    let input = temp_path("replace-in-place", "png");
    create_png(&input, 6, 4);
    let original = fs::read(&input).unwrap();
    let hash = fixture_sha256(&original);
    let reply = run(&[
        "image",
        "resize",
        input.to_str().unwrap(),
        "--width",
        "3",
        "-o",
        input.to_str().unwrap(),
        "--replace",
        "--expected-output-sha256",
        &hash,
        "--json",
    ]);
    let json = parse_stdout(&reply);
    assert_eq!(json["result"]["replaced"], true);
    assert_eq!(json["result"]["output_receipt"]["previous_sha256"], hash);
    assert_eq!(json["result"]["output_receipt"]["status"], "verified");
    let decoded = image::open(&input).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (3, 2));
    assert_ne!(fs::read(&input).unwrap(), original);
    assert_eq!(
        json["result"]["output_receipt"]["verified_output"]["sha256"],
        fixture_sha256(&fs::read(&input).unwrap())
    );
    fs::remove_file(input).unwrap();
}

#[cfg(unix)]
#[test]
fn image_replace_rejects_symlink_destination_without_changing_target() {
    use std::os::unix::fs::symlink;
    let input = temp_path("replace-link-input", "png");
    let target = temp_path("replace-link-target", "png");
    let destination = temp_path("replace-link-output", "png");
    create_png(&input, 4, 2);
    create_png(&target, 2, 2);
    let original = fs::read(&target).unwrap();
    symlink(&target, &destination).unwrap();
    let hash = fixture_sha256(&original);
    let reply = run(&[
        "image",
        "convert",
        input.to_str().unwrap(),
        "-o",
        destination.to_str().unwrap(),
        "--replace",
        "--expected-output-sha256",
        &hash,
        "--json",
    ]);
    assert_eq!(reply.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&reply.stderr).unwrap();
    assert_eq!(error["error"]["code"], "INVALID_INPUT");
    assert_eq!(fs::read(&target).unwrap(), original);
    assert_eq!(fs::read_link(&destination).unwrap(), target);
    fs::remove_file(input).unwrap();
    fs::remove_file(target).unwrap();
    fs::remove_file(destination).unwrap();
}

#[test]
fn explicit_incompatible_engine_returns_structured_error() {
    let input = temp_path("engine-input", "png");
    create_png(&input, 4, 2);

    let output = run(&[
        "image",
        "info",
        input.to_str().unwrap(),
        "--engine",
        "yu-runtime",
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(3));
    let json: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be structured JSON");
    assert_eq!(json["error"]["code"], "ENGINE_INCOMPATIBLE");

    let _ = fs::remove_file(input);
}

#[derive(Clone)]
struct FixtureDownloader {
    bytes: Vec<u8>,
}

impl Downloader for FixtureDownloader {
    fn download(&self, _url: &str, destination: &Path) -> Result<u64, ManagerError> {
        fs::write(destination, &self.bytes)
            .map_err(|error| ManagerError::Io(format!("fixture download failed: {error}")))?;
        Ok(self.bytes.len() as u64)
    }
}

fn fixture_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn install_fixture_engine(data_home: &Path, version: &str) {
    let bytes = format!("fixture-{version}").into_bytes();
    let target = EngineTarget::current();
    let manifest = EngineManifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_owned(),
        id: "fixture-engine".to_owned(),
        display_name: "Fixture Engine".to_owned(),
        version: version.to_owned(),
        capabilities: vec!["fixture.run".to_owned()],
        packages: vec![EnginePackage {
            target: target.clone(),
            url: "https://example.invalid/fixture".to_owned(),
            sha256: fixture_sha256(&bytes),
            archive: ArchiveKind::Raw,
            entrypoint: "bin/fixture".to_owned(),
            args: Vec::new(),
        }],
    };
    let manager = EngineManager::new(ManagedLayout::new(data_home), target);
    let installer = EngineInstaller::new(manager, FixtureDownloader { bytes });
    installer
        .install(&manifest)
        .expect("fixture install should succeed");
}

#[test]
fn engine_versions_activate_deactivate_remove_round_trip() {
    let root = temp_path("engine-lifecycle", "dir");
    fs::create_dir_all(&root).unwrap();
    install_fixture_engine(&root, "1.0.0");
    install_fixture_engine(&root, "2.0.0");

    let output = run_with_data_home(&["engine", "list", "--json"], &root);
    let json = parse_stdout(&output);
    let managed = json["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|engine| engine["id"] == "fixture-engine" && engine["provider"] == "managed")
        .expect("managed fixture should be discovered");
    assert_eq!(managed["state"], "disabled");
    assert_eq!(managed["installed_versions"].as_array().unwrap().len(), 2);

    let output = run_with_data_home(&["engine", "versions", "fixture-engine", "--json"], &root);
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "engine.versions");
    assert_eq!(json["result"].as_array().unwrap().len(), 2);

    let output = run_with_data_home(
        &["engine", "activate", "fixture-engine", "2.0.0", "--json"],
        &root,
    );
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "engine.activate");
    assert_eq!(json["result"]["active_version"], "2.0.0");

    let output = run_with_data_home(&["engine", "info", "fixture-engine", "--json"], &root);
    let json = parse_stdout(&output);
    let managed = json["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|engine| engine["provider"] == "managed")
        .expect("managed fixture info should be discovered");
    assert_eq!(managed["state"], "ready");
    assert_eq!(managed["active_version"], "2.0.0");
    assert_eq!(managed["display_name"], "Fixture Engine");
    assert_eq!(managed["capabilities"][0], "fixture.run");

    let output = run_with_data_home(
        &["engine", "remove", "fixture-engine", "2.0.0", "--json"],
        &root,
    );
    assert_eq!(output.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "OUTPUT_CONFLICT");

    let output = run_with_data_home(&["engine", "deactivate", "fixture-engine", "--json"], &root);
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "engine.deactivate");
    assert_eq!(json["result"]["previous_version"], "2.0.0");

    let output = run_with_data_home(
        &["engine", "remove", "fixture-engine", "2.0.0", "--json"],
        &root,
    );
    let json = parse_stdout(&output);
    assert_eq!(json["operation"], "engine.remove");
    assert_eq!(json["result"]["version"], "2.0.0");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn engine_install_rejects_invalid_manifest_before_network() {
    let root = temp_path("engine-invalid", "dir");
    fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
          "schema_version":"1",
          "id":"fixture-engine",
          "display_name":"Fixture",
          "version":"1.0.0",
          "capabilities":[],
          "packages":[{
            "target":{"os":"linux","arch":"x86_64"},
            "url":"http://example.invalid/fixture",
            "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "archive":"raw",
            "entrypoint":"bin/fixture"
          }]
        }"#,
    )
    .unwrap();

    let output = run_with_data_home(
        &[
            "engine",
            "install",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--json",
        ],
        &root,
    );

    assert_eq!(output.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(json["error"]["code"], "INVALID_INPUT");

    let _ = fs::remove_dir_all(root);
}
