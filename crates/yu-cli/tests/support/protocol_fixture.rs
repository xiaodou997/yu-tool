//! Native test engine, compiled by integration tests; never shipped with YuTool.
use std::{io::{Read, Write}, process::{Command, Stdio}, thread, time::Duration};

#[path = "lifecycle_fixture.rs"]
mod lifecycle;

fn string_field<'a>(request: &'a str, key: &str) -> &'a str {
    request.split(&format!("\"{key}\":\"")).nth(1).unwrap_or("").split('"').next().unwrap_or("")
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "ok".into());
    if lifecycle::before_input(&mode) { return; }
    if mode == "sleep" || mode == "descendant" {
        thread::sleep(Duration::from_secs(20));
        return;
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    if mode == "inherited-pipes" {
        let _child = Command::new(std::env::current_exe().unwrap()).arg("descendant")
            .stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn().unwrap();
        return; // The runtime must not wait indefinitely for inherited pipe handles.
    }
    if mode == "stdout-limit" {
        let _ = std::io::stdout().write_all(&vec![b'x'; 17 * 1024 * 1024]);
        return;
    }
    if mode == "stderr-limit" {
        let _ = std::io::stderr().write_all(&vec![b'x'; 65 * 1024]);
        return;
    }
    if mode == "nonzero" { eprintln!("fixture process failure"); std::process::exit(9); }
    if mode == "garbage" { print!("not json"); return; }
    if mode == "invalid-utf8" { std::io::stdout().write_all(&[255]).unwrap(); return; }
    if mode == "environment" {
        assert!(std::env::var_os("NODE_OPTIONS").is_none());
        assert!(std::env::var_os("NODE_PATH").is_none());
        assert!(std::path::Path::new(".yu-install.json").is_file());
    }
    let request_id = if mode == "wrong-id" { "unrelated" } else { string_field(&input, "request_id") };
    let protocol = if mode == "protocol-v2" { "2" } else { "1" };
    if mode == "invalid-input" || mode == "unsupported" || mode == "invalid-argument" {
        let code = match mode.as_str() {
            "invalid-input" => "INVALID_INPUT",
            "unsupported" => "UNSUPPORTED_CAPABILITY",
            _ => "INVALID_ARGUMENT",
        };
        print!("{{\"protocol_version\":\"{protocol}\",\"request_id\":\"{request_id}\",\"status\":\"error\",\"error\":{{\"code\":\"{code}\",\"message\":\"fixture business error\"}}}}");
        return;
    }
    if string_field(&input, "capability") == "psd.layer.export" && mode.starts_with("export-") {
        // Paths here are generated ASCII test staging names; retain JSON escaping in the response.
        let encoded_path = string_field(&input, "output_path");
        let path = encoded_path.replace("\\\\", "\\");
        if mode == "export-partial" {
            std::fs::write(&path, b"partial PNG").unwrap();
            print!("{{\"protocol_version\":\"1\",\"request_id\":\"{request_id}\",\"status\":\"error\",\"error\":{{\"code\":\"EXECUTION_FAILED\",\"message\":\"partial artifact\"}}}}");
            return;
        }
        if mode == "export-garbage" { std::fs::write(&path, b"not PNG").unwrap(); }
        else { std::fs::copy(std::env::args().nth(2).unwrap(), &path).unwrap(); }
        let id = if mode == "export-wrong-layer" { "L0002" } else { string_field(&input, "layer_id") };
        let output_path = if mode == "export-wrong-path" { "other.png" } else { encoded_path };
        let width = if mode == "export-wrong-size" { 3 } else { 2 };
        print!("{{\"protocol_version\":\"1\",\"request_id\":\"{request_id}\",\"status\":\"ok\",\"result\":{{\"contract_version\":\"1\",\"layer_id\":\"{id}\",\"output_path\":\"{output_path}\",\"width\":{width},\"height\":1,\"pixel_format\":\"rgba8\",\"container\":\"png\"}}}}");
        return;
    }
    let document = r#"{"format":"psd","width":2,"height":3,"channels":3,"bits_per_channel":8,"color_mode":"rgb","layer_count":2,"maximum_tree_depth":1}"#;
    let layer = |id: &str| format!(r#"{{"id":"{id}","depth":1,"name":"same","kind":"pixel","visible":true,"has_pixel_mask":false,"has_vector_mask":false,"child_count":0}}"#);
    let a = layer("L0001");
    let b = layer("L0002");
    let contract = if mode == "contract-v2" { "2" } else { "1" };
    let mut result = format!("\"contract_version\":\"{contract}\",\"document\":{document}");
    match string_field(&input, "capability") {
        "psd.tree" => {
            let node = |item: String| format!("{},\"children\":[]}}", item.trim_end_matches('}'));
            result.push_str(&format!(",\"layers\":[{},{}]", node(a), node(b)));
        }
        "psd.layer.list" => result.push_str(&format!(",\"layers\":[{a},{b}]")),
        "psd.layer.info" => {
            let id = if mode == "wrong-layer" { "L0002" } else { string_field(&input, "layer_id") };
            result.push_str(&format!(",\"layer\":{}", layer(id)));
        }
        _ => {}
    }
    if mode == "bad-schema" { result = "\"contract_version\":\"1\"".into(); }
    let response = format!("{{\"protocol_version\":\"{protocol}\",\"request_id\":\"{request_id}\",\"status\":\"ok\",\"result\":{{{result}}},\"warnings\":[\"fixture warning\"]}}");
    if lifecycle::respond(&mode, &response) { return; }
    print!("{response}");
    if mode == "multiple-json" { print!("{response}"); }
}
