use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Output, Stdio};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cli(args: &[&str], base: Option<&str>, compute: Option<&str>, workflow: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_aigate-cli"));
    command
        .env_remove("AIGATE_TOKEN")
        .env_remove("AIGATE_WORKFLOW_TOKEN")
        .arg("--no-proxy")
        .stdin(Stdio::null());
    if let Some(base) = base {
        command.args(["--base-url", base]);
    }
    if let Some(token) = compute {
        command.env("AIGATE_TOKEN", token);
    }
    if let Some(token) = workflow {
        command.env("AIGATE_WORKFLOW_TOKEN", token);
    }
    command.args(args).output().unwrap()
}
fn json_file(value: Value) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, "{value}").unwrap();
    file
}
fn parsed(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn success(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"code":0,"ok":true,"data":data}))
}

#[test]
fn generation_help_is_offline_and_exposes_nested_commands() {
    for group in ["workflow", "comfy"] {
        let output = cli(&[group, "--help"], None, Some(""), Some(""));
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        for command in ["run", "status", "upload"] {
            assert!(stdout.contains(command));
        }
        assert!(!stdout.contains("--token"));
    }
}

#[test]
fn offline_previews_select_service_origin_and_redact_parameters() {
    let file = json_file(
        json!({"prompt":"example","api_key":"input-secret","image":"HTTPS://example.invalid/file?signature=upper-secret"}),
    );
    let input = file.path().to_str().unwrap();
    let workflow = parsed(&cli(
        &[
            "workflow",
            "run",
            "--app-id",
            "my-app",
            "--input",
            input,
            "--dry-run",
        ],
        None,
        Some("invalid token"),
        Some("invalid token"),
    ));
    assert_eq!(workflow["origin"], "https://studio.aigate.cc");
    assert_eq!(workflow["body"]["appId"], "my-app");
    assert_eq!(workflow["body"]["inputParams"]["api_key"], "[REDACTED]");
    assert_eq!(
        workflow["body"]["inputParams"]["image"],
        "HTTPS://example.invalid/file?[REDACTED]"
    );
    let comfy = parsed(&cli(
        &[
            "comfy",
            "run",
            "--app-id",
            "9223372036854775807",
            "--instance-id",
            "456",
            "--input",
            input,
            "--dry-run",
        ],
        None,
        Some(""),
        Some(""),
    ));
    assert_eq!(comfy["origin"], "https://waas.aigate.cc");
    assert_eq!(comfy["body"]["appId"], "9223372036854775807");
    assert_eq!(comfy["body"]["allowInput"]["prompt"], "example");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generation_and_upload_previews_and_unconfirmed_tasks_send_no_requests() {
    let server = MockServer::start().await;
    let file = json_file(
        json!({"prompt":"example","token":"input-secret","image":"HTTPS://example.invalid/file?signature=upper-secret"}),
    );
    let input = file.path().to_str().unwrap();
    for group in ["workflow", "comfy"] {
        let mut args = vec![group, "run", "--app-id", "123", "--input", input];
        if group == "comfy" {
            args.extend(["--instance-id", "456"]);
        }
        let output = cli(
            &args,
            Some(&server.uri()),
            Some("invalid token"),
            Some("invalid token"),
        );
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("非交互模式拒绝执行"));
        assert!(!error.contains("input-secret") && !error.contains("invalid token"));
        assert!(!error.contains("upper-secret"));
        args.push("--dry-run");
        assert_eq!(
            parsed(&cli(&args, Some(&server.uri()), None, None))["dryRun"],
            true
        );
    }
    for args in [
        vec![
            "workflow",
            "upload",
            input,
            "--media-type",
            "image",
            "--dry-run",
        ],
        vec![
            "comfy",
            "upload",
            input,
            "--app-id",
            "123",
            "--instance-id",
            "456",
            "--dry-run",
        ],
    ] {
        assert_eq!(
            parsed(&cli(&args, Some(&server.uri()), Some(""), Some("")))["dryRun"],
            true
        );
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workflow_status_redacts_uppercase_scheme_file_strings() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(success(json!({"taskId":"123","status":2,"files":["HTTPS://example.invalid/file?signature=upper-secret"]})))
        .expect(1).mount(&server).await;
    let output = cli(
        &["workflow", "status", "123"],
        Some(&server.uri()),
        None,
        Some("workflow-test"),
    );
    assert_eq!(
        parsed(&output)["files"][0],
        "HTTPS://example.invalid/file?[REDACTED]"
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("upper-secret"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_task_url_is_rejected_without_echoing_signature_or_sending_requests() {
    let server = MockServer::start().await;
    for group in ["workflow", "comfy"] {
        let output = cli(
            &[
                group,
                "status",
                "https://example.invalid/?signature=private-signature",
            ],
            Some(&server.uri()),
            Some("compute-test"),
            Some("workflow-test"),
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("taskId"));
        assert!(!error.contains("private-signature") && !error.contains("example.invalid"));
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[test]
fn invalid_json_parameters_fail_before_authentication_and_never_echo_content() {
    for value in [json!([]), json!("private-secret"), json!(null)] {
        let file = json_file(value);
        let output = cli(
            &[
                "workflow",
                "run",
                "--app-id",
                "123",
                "--input",
                file.path().to_str().unwrap(),
                "--yes",
            ],
            None,
            Some(""),
            Some(""),
        );
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("JSON object"));
        assert!(!error.contains("private-secret"));
    }
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, "invalid-json-private-secret").unwrap();
    let output = cli(
        &[
            "comfy",
            "run",
            "--app-id",
            "123",
            "--instance-id",
            "456",
            "--input",
            file.path().to_str().unwrap(),
            "--dry-run",
        ],
        None,
        None,
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-secret"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn credentials_never_fall_back_between_profiles_or_cross_custom_origins() {
    let server = MockServer::start().await;
    for (args, compute, workflow, expected) in [
        (
            vec!["workflow", "status", "123"],
            Some("compute-secret"),
            None,
            "AIGATE_WORKFLOW_TOKEN",
        ),
        (
            vec!["comfy", "apps"],
            None,
            Some("workflow-secret"),
            "AIGATE_TOKEN",
        ),
        (
            vec!["workflow", "login"],
            Some("compute-secret"),
            Some("workflow-secret"),
            "Keyring",
        ),
        (vec!["workflow", "logout"], None, None, "Keyring"),
        (
            vec!["workflow", "status", "123"],
            Some("compute-secret"),
            Some(""),
            "Token 必须",
        ),
    ] {
        let output = cli(&args, Some(&server.uri()), compute, workflow);
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected), "{error}");
        assert!(!error.contains("compute-secret") && !error.contains("workflow-secret"));
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_task_confirmation_uses_matching_profile_and_returns_task_id() {
    let server = MockServer::start().await;
    let file = json_file(json!({"prompt":"example"}));
    for (group, path_suffix, body, token) in [
        (
            "workflow",
            "/api/comfyui/open/v1/comfyui_workflow/create",
            json!({"appId":"123","inputParams":{"prompt":"example"}}),
            "Bearer workflow-test",
        ),
        (
            "comfy",
            "/api/openapi/comfyui/comfyuiTask/create",
            json!({"appId":"123","instanceId":"456","allowInput":{"prompt":"example"}}),
            "Bearer compute-test",
        ),
    ] {
        Mock::given(method("POST"))
            .and(path(path_suffix))
            .and(header("authorization", token))
            .and(body_json(body))
            .respond_with(success(json!({"taskId":"1154715881075376128"})))
            .expect(1)
            .mount(&server)
            .await;
        let mut args = vec![
            group,
            "run",
            "--app-id",
            "123",
            "--input",
            file.path().to_str().unwrap(),
            "--yes",
            "--json",
        ];
        if group == "comfy" {
            args.extend(["--instance-id", "456"]);
        }
        let output = cli(
            &args,
            Some(&server.uri()),
            Some("compute-test"),
            Some("workflow-test"),
        );
        assert_eq!(parsed(&output)["taskId"], "1154715881075376128");
        assert!(String::from_utf8_lossy(&output.stderr).contains("查询最终状态"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_status_redacts_signed_results_and_bare_comfy_hosts_by_default() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/api/openapi/comfyui/comfyuiTask/status")).and(query_param("taskId","123"))
        .respond_with(success(json!({"taskId":123,"status":2,"comfyuiHost":"example.invalid?token=host-secret","files":[{"downloadUrl":"https://example.invalid/file?signature=file-secret"}]})))
        .expect(2).mount(&server).await;
    let output = cli(
        &["comfy", "status", "123"],
        Some(&server.uri()),
        Some("compute-test"),
        None,
    );
    assert_eq!(parsed(&output)["label"], "成功");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("host-secret") && !stdout.contains("file-secret"));
    let output = cli(
        &["comfy", "status", "123", "--show-secrets"],
        Some(&server.uri()),
        Some("compute-test"),
        None,
    );
    assert!(parsed(&output)["files"][0]["downloadUrl"]
        .as_str()
        .unwrap()
        .contains("file-secret"));
}

#[test]
fn bad_generation_flags_are_argument_errors() {
    let file = json_file(json!({}));
    for args in [
        vec![
            "workflow",
            "run",
            "--app-id",
            "123",
            "--input",
            file.path().to_str().unwrap(),
            "--dry-run",
            "--yes",
        ],
        vec![
            "workflow",
            "upload",
            file.path().to_str().unwrap(),
            "--media-type",
            "document",
        ],
        vec![
            "comfy",
            "run",
            "--app-id",
            "0",
            "--instance-id",
            "1",
            "--input",
            file.path().to_str().unwrap(),
        ],
        vec!["comfy", "status", "0"],
        vec!["workflow", "status", "invalid task"],
    ] {
        assert_eq!(cli(&args, None, None, None).status.code(), Some(2));
    }
}
