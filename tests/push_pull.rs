mod common;

use std::fs;
use std::thread;

use common::{fixture_path, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

#[test]
fn push_json_publishes_frozen_profile_payload() {
    let (base_url, server) = spawn_one_shot_server(|mut request| {
        assert_eq!(request.method(), &Method::Post);
        assert_eq!(request.url(), "/execute");

        let mut body = String::new();
        request
            .as_reader()
            .read_to_string(&mut body)
            .expect("request body should be readable");
        let payload: Value = serde_json::from_str(&body).expect("push payload should be JSON");

        assert_eq!(
            payload.get("command").and_then(|v| v.as_str()),
            Some("AddProfileArtifact")
        );
        assert_eq!(
            payload
                .get("payload")
                .and_then(|p| p.get("profile_id"))
                .and_then(|v| v.as_str()),
            Some("csv.loan_tape.core.v0")
        );
        assert_eq!(
            payload
                .get("payload")
                .and_then(|p| p.get("profile_sha256"))
                .and_then(|v| v.as_str()),
            Some("sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );

        request
            .respond(json_response(202, r#"{"errors":null}"#))
            .expect("response should be sent");
    });

    let assert = profile_cmd()
        .env("EPISTEMIC_FABRIC_URL", &base_url)
        .arg("--json")
        .arg("--no-witness")
        .arg("push")
        .arg(fixture_path("profiles/valid/frozen_complete.yaml"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("published"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("profile_id"))
            .and_then(|v| v.as_str()),
        Some("csv.loan_tape.core.v0")
    );

    server.join().expect("server thread should complete");
}

#[test]
fn push_json_refuses_non_frozen_profile_with_e_invalid_schema() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("push")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_INVALID_SCHEMA")
    );
}

#[test]
fn pull_json_fetches_profile_and_writes_output_file() {
    let fixture_content = fs::read_to_string(fixture_path("profiles/valid/frozen_complete.yaml"))
        .expect("fixture should be readable");
    let response_body = json!({ "content": fixture_content }).to_string();

    let (base_url, server) = spawn_one_shot_server(move |request| {
        assert_eq!(request.method(), &Method::Get);
        assert_eq!(request.url(), "/query/profile/csv.loan_tape.core.v0");
        request
            .respond(json_response(200, &response_body))
            .expect("response should be sent");
    });

    let workspace = temp_workspace();
    let out_dir = workspace.path().join("profiles");

    let assert = profile_cmd()
        .env("EPISTEMIC_FABRIC_URL", &base_url)
        .arg("--json")
        .arg("--no-witness")
        .arg("pull")
        .arg("csv.loan_tape.core.v0")
        .arg("--out")
        .arg(&out_dir)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let written_path = out_dir.join("csv.loan_tape.core.v0.yaml");
    let written = fs::read_to_string(&written_path).expect("pulled file should be written");
    let expected = fs::read_to_string(fixture_path("profiles/valid/frozen_complete.yaml"))
        .expect("fixture should be readable");
    assert_eq!(written, expected);
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("path"))
            .and_then(|v| v.as_str()),
        Some(written_path.to_string_lossy().as_ref())
    );

    server.join().expect("server thread should complete");
}

#[test]
fn pull_json_maps_server_errors_to_e_io() {
    let (base_url, server) = spawn_one_shot_server(|request| {
        assert_eq!(request.method(), &Method::Get);
        request
            .respond(json_response(500, r#"{"error":"boom"}"#))
            .expect("response should be sent");
    });

    let workspace = temp_workspace();

    let assert = profile_cmd()
        .env("EPISTEMIC_FABRIC_URL", &base_url)
        .arg("--json")
        .arg("--no-witness")
        .arg("pull")
        .arg("csv.loan_tape.core.v0")
        .arg("--out")
        .arg(workspace.path())
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_IO")
    );

    server.join().expect("server thread should complete");
}

fn spawn_one_shot_server<F>(handler: F) -> (String, thread::JoinHandle<()>)
where
    F: FnOnce(Request) + Send + 'static,
{
    let server = Server::http("127.0.0.1:0").expect("server should bind");
    let base_url = format!("http://{}", server.server_addr());

    let handle = thread::spawn(move || {
        let request = server.recv().expect("server should receive request");
        handler(request);
    });

    (base_url, handle)
}

fn json_response(status: u16, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let header =
        Header::from_bytes("Content-Type", "application/json").expect("header should parse");
    Response::from_string(body.to_string())
        .with_header(header)
        .with_status_code(StatusCode(status))
}
