use serde_json::{json, Value};
use std::process::{Command, Output, Stdio};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cli(args: &[&str], base: Option<&str>, token: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_aigate-cli"));
    command
        .env("AIGATE_TOKEN", token)
        .stdin(Stdio::null())
        .arg("--no-proxy");
    if let Some(base) = base {
        command.args(["--base-url", base]);
    }
    command.args(args).output().unwrap()
}

fn ok(output: &Output) {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn parsed(output: &Output) -> Value {
    ok(output);
    serde_json::from_slice(&output.stdout).unwrap()
}

fn success(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"code": 0, "ok": true, "data": data}))
}

const CREATE: &[&str] = &[
    "create",
    "--sku",
    "4090D-48G",
    "--area",
    "华东一区",
    "--image-id",
    "1096726823955333120",
    "--image-type",
    "3",
];

#[test]
fn help_and_regions_are_available_without_credentials() {
    let help = cli(&["--help"], None, "");
    ok(&help);
    let stdout = String::from_utf8_lossy(&help.stdout);
    for command in [
        "skus", "images", "list", "info", "status", "create", "start", "stop", "release", "login",
        "logout", "init",
    ] {
        assert!(stdout.contains(command));
    }
    assert!(!stdout.contains("--token"));
    assert_eq!(
        parsed(&cli(&["regions", "--json"], None, "")),
        json!(["华东一区", "华东二区"])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn init_emits_shell_code_without_credentials_or_api_requests() {
    let server = MockServer::start().await;
    for shell in [
        "bash",
        "zsh",
        "fish",
        "powershell",
        "clink",
        "cmder",
        "nushell",
        "nu",
    ] {
        let output = cli(&["init", shell, "--json"], Some(&server.uri()), "");
        ok(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("aigate-cli"));
        assert!(output.stderr.is_empty());
        assert!(serde_json::from_slice::<Value>(&output.stdout).is_err());
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[test]
fn init_requires_a_supported_shell() {
    for args in [
        vec!["init"],
        vec!["init", "sh"],
        vec!["init", "cmd"],
        vec!["init", "bash", "extra"],
    ] {
        let output = cli(&args, None, "");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_origin_cannot_read_or_modify_official_keyring_credentials() {
    let server = MockServer::start().await;
    for command in ["list", "status", "login", "logout"] {
        let output = Command::new(env!("CARGO_BIN_EXE_aigate-cli"))
            .env_remove("AIGATE_TOKEN")
            .args(["--no-proxy", "--base-url", &server.uri(), command])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Keyring 凭证仅适用于官方 API origin"));
        assert!(stderr.contains("AIGATE_TOKEN"));
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_and_release_dry_run_are_offline_even_with_invalid_token() {
    let server = MockServer::start().await;
    let mut args = CREATE.to_vec();
    args.push("--dry-run");
    let value = parsed(&cli(&args, Some(&server.uri()), ""));
    assert_eq!(value["body"]["imageId"].as_i64(), Some(1096726823955333120));
    assert_eq!(value["method"], "POST");
    assert_eq!(value["path"], "/api/openapi/instance/start");
    assert_eq!(value["dryRun"], true);
    let release = parsed(&cli(
        &["release", "123", "--dry-run"],
        Some(&server.uri()),
        "",
    ));
    assert_eq!(release["query"]["instanceId"], "123");
    assert_eq!(release["path"], "/api/openapi/instance/release");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn noninteractive_create_and_release_require_yes_before_any_api_request() {
    let server = MockServer::start().await;
    for args in [CREATE.to_vec(), vec!["release", "123"], vec!["rm", "123"]] {
        let output = cli(&args, Some(&server.uri()), "");
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("--yes"));
        assert!(output.stdout.is_empty());
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn yes_creates_exact_resource_and_emits_redacted_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/api/openapi/instance/start"))
        .and(header("authorization", "Bearer cli-token"))
        .and(body_json(json!({"skuName": "4090D-48G", "areaName": "华东一区", "count": 1, "imageId": 1096726823955333120_i64, "imageType": "3"})))
        .respond_with(success(json!({"instanceId": "123", "operationStatus": "9", "instanceUtilList": [{"password": "private-password"}]})))
        .expect(1).mount(&server).await;
    let mut args = CREATE.to_vec();
    args.extend(["--yes", "--json"]);
    let output = cli(&args, Some(&server.uri()), "Bearer cli-token");
    let value = parsed(&output);
    assert_eq!(value["instanceId"], "123");
    assert_eq!(value["instanceUtilList"][0]["password"], "[REDACTED]");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("cli-token"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-password"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gpu_model_alternative_serializes_without_sku_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/api/openapi/instance/start"))
        .and(body_json(json!({"gpuModel": "A100", "areaName": "华东二区", "count": 8, "imageId": 42, "imageType": "2"})))
        .respond_with(success(json!({"instanceId": "123"}))).expect(1).mount(&server).await;
    ok(&cli(
        &[
            "create",
            "--gpu-model",
            "A100",
            "--area",
            "华东二区",
            "--count",
            "8",
            "--image-id",
            "42",
            "--image-type",
            "2",
            "--yes",
        ],
        Some(&server.uri()),
        "cli-token",
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn yes_releases_only_named_id_and_reports_acceptance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/openapi/instance/release"))
        .and(query_param("instanceId", "1096749550260588544"))
        .respond_with(success(Value::Null))
        .expect(1)
        .mount(&server)
        .await;
    let value = parsed(&cli(
        &["rm", "1096749550260588544", "-y", "--json"],
        Some(&server.uri()),
        "cli-token",
    ));
    assert_eq!(
        value,
        json!({"action": "release", "instanceId": "1096749550260588544", "accepted": true})
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lifecycle_commands_use_status_then_the_correct_endpoint() {
    for (command, state, endpoint) in [("start", "22", "open"), ("stop", "2", "close")] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/openapi/instance/status"))
            .and(query_param("instanceId", "123"))
            .respond_with(success(json!(state)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/api/openapi/instance/{endpoint}")))
            .and(query_param("instanceId", "123"))
            .respond_with(success(Value::Null))
            .expect(1)
            .mount(&server)
            .await;
        let value = parsed(&cli(
            &[command, "123", "--json"],
            Some(&server.uri()),
            "cli-token",
        ));
        assert_eq!(value["action"], command);
        assert_eq!(value["accepted"], true);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skus_defaults_to_both_documented_areas_and_renders_table() {
    let server = MockServer::start().await;
    for area in ["华东一区", "华东二区"] {
        Mock::given(method("GET")).and(path("/api/openapi/instance/skuList"))
            .and(query_param("areaName", area))
            .respond_with(success(json!([{"areaName": area, "skuName": "4090D-48G", "price": "199", "maxGpuCount": 3}])))
            .expect(1).mount(&server).await;
    }
    let output = cli(&["stock"], Some(&server.uri()), "cli-token");
    ok(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for text in ["华东一区", "华东二区", "4090D-48G", "199"] {
        assert!(stdout.contains(text));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_json_strips_envelope_and_includes_total() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/openapi/instance/page"))
        .and(body_json(
            json!({"current": 2, "pageSize": 5, "operationStatus": "4"}),
        ))
        .respond_with(success(
            json!({"total": 8, "records": [{"instanceId": "123", "operationStatus": "4"}]}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let value = parsed(&cli(
        &[
            "list",
            "--page",
            "2",
            "--size",
            "5",
            "--operation-status",
            "4",
            "--json",
        ],
        Some(&server.uri()),
        "cli-token",
    ));
    assert_eq!(value["total"], 8);
    assert_eq!(value["records"][0]["instanceId"], "123");
    assert!(value.get("code").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn image_commands_send_documented_personal_and_community_filters() {
    for (args, body) in [
        (
            vec!["images", "--json"],
            json!({"current": 1, "pageSize": 20, "imageType": "3"}),
        ),
        (
            vec![
                "images",
                "--image-type",
                "2",
                "--area",
                "华东一区",
                "--sku",
                "4090D-48G",
                "--name",
                "Comfy",
                "--json",
            ],
            json!({"current": 1, "pageSize": 20, "imageType": "2", "areaName": "华东一区", "skuName": "4090D-48G", "imageName": "Comfy", "imageVersion": ""}),
        ),
        (
            vec![
                "images",
                "--image-type",
                "2",
                "--area",
                "华东一区",
                "--sku",
                "4090D-48G",
                "--name",
                "Comfy",
                "--image-version",
                "v1",
                "--json",
            ],
            json!({"current": 1, "pageSize": 20, "imageType": "2", "areaName": "华东一区", "skuName": "4090D-48G", "imageName": "Comfy", "imageVersion": "v1"}),
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/openapi/image/page"))
            .and(body_json(body))
            .respond_with(success(
                json!({"total": 1, "records": [{"worksId": "42", "name": "Comfy"}]}),
            ))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            parsed(&cli(&args, Some(&server.uri()), "cli-token"))["records"][0]["worksId"],
            "42"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn community_images_require_nonempty_name_before_credentials_or_requests() {
    let server = MockServer::start().await;
    let base_args = [
        "images",
        "--image-type",
        "2",
        "--area",
        "华东一区",
        "--sku",
        "4090D-48G",
    ];
    for name_args in [vec![], vec!["--name", " "]] {
        let mut args = base_args.to_vec();
        args.extend(name_args);
        let output = cli(&args, Some(&server.uri()), "");
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("--name"));
        assert!(output.stdout.is_empty());
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn info_redacts_credentials_unless_explicitly_requested() {
    let server = MockServer::start().await;
    Mock::given(path("/api/openapi/instance/get")).respond_with(success(json!({"instanceId": "123", "instanceUtilList": [{"password": "private-password", "host": "instance.region1.waas.aigate.cc?token=private-token", "config": "private-config"}]})))
        .expect(2).mount(&server).await;
    let output = cli(&["info", "123", "--json"], Some(&server.uri()), "cli-token");
    ok(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for secret in ["private-password", "private-token", "private-config"] {
        assert!(!stdout.contains(secret));
    }
    let value = parsed(&cli(
        &["info", "123", "--show-secrets", "--json"],
        Some(&server.uri()),
        "cli-token",
    ));
    assert_eq!(value["instanceUtilList"][0]["password"], "private-password");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_without_id_emits_complete_redacted_json_overview() {
    let server = MockServer::start().await;
    for (current, records) in [
        (
            1,
            json!([
                {"instanceId": 1096749550260588544_i64, "instanceName": "running", "operationStatus": "2", "password": "private-password"},
                {"instanceId": "1096749550260588545", "operationStatus": 2}
            ]),
        ),
        (
            2,
            json!([
                {"instanceId": "1096749550260588546", "operationStatus": "26", "config": "private-config"},
                {"instanceId": "1096749550260588547", "operationStatus": "999"}
            ]),
        ),
    ] {
        Mock::given(method("POST"))
            .and(path("/api/openapi/instance/page"))
            .and(header("authorization", "Bearer cli-token"))
            .and(body_json(json!({"current": current, "pageSize": 20})))
            .respond_with(success(json!({"total": 4, "records": records})))
            .expect(1)
            .mount(&server)
            .await;
    }
    let output = cli(&["--json", "status"], Some(&server.uri()), "cli-token");
    let value = parsed(&output);
    assert_eq!(value["total"], 4);
    assert_eq!(value["records"].as_array().unwrap().len(), 4);
    assert_eq!(
        value["records"][0]["instanceId"].as_i64(),
        Some(1096749550260588544)
    );
    assert_eq!(value["records"][0]["password"], "[REDACTED]");
    assert_eq!(value["records"][2]["config"], "[REDACTED]");
    assert_eq!(value["records"][3]["label"], "未知状态");
    assert_eq!(
        value["summary"],
        json!([
            {"status": "2", "label": "运行中", "count": 2},
            {"status": "26", "label": "实例关机且已上传", "count": 1},
            {"status": "999", "label": "未知状态", "count": 1}
        ])
    );
    assert!(value.get("code").is_none());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for secret in ["cli-token", "private-password", "private-config"] {
        assert!(!stdout.contains(secret));
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_overview_renders_counts_table_and_safe_unknown_states() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/openapi/instance/page"))
        .and(body_json(json!({"current": 1, "pageSize": 20})))
        .respond_with(success(json!({"total": 3, "records": [
            {"instanceId": "123", "instanceName": "smoke\u{7}", "operationStatus": "2", "areaName": "华东一区", "count": 1},
            {"instanceId": "124", "operationStatus": 22},
            {"instanceId": "125", "operationStatus": "999\u{1b}[2J", "password": "private-password"}
        ]})))
        .expect(1)
        .mount(&server)
        .await;
    let output = cli(&["status"], Some(&server.uri()), "cli-token");
    ok(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for text in [
        "当前计费中实例：3 个",
        "运行中 (2)：1 个",
        "实例已关机 (22)：1 个",
        "未知状态",
        "smoke",
        "华东一区",
        "123",
        "124",
        "125",
        "GPU数",
    ] {
        assert!(stdout.contains(text), "missing {text}: {stdout}");
    }
    assert!(!stdout.chars().any(|c| c.is_control() && c != '\n'));
    assert!(!stdout.contains("private-password"));
    assert!(output.stderr.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_overview_handles_empty_accounts_in_text_and_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/openapi/instance/page"))
        .and(body_json(json!({"current": 1, "pageSize": 20})))
        .respond_with(success(json!({"total": 0, "records": []})))
        .expect(2)
        .mount(&server)
        .await;
    let output = cli(&["status"], Some(&server.uri()), "cli-token");
    ok(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("暂无计费中实例"));
    assert!(output.stderr.is_empty());
    assert_eq!(
        parsed(&cli(
            &["status", "--json"],
            Some(&server.uri()),
            "cli-token"
        )),
        json!({"total": 0, "summary": [], "records": []})
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_overview_failures_do_not_emit_a_success_or_partial_overview() {
    for response in [
        ResponseTemplate::new(503),
        success(json!({"total": 2, "records": []})),
        success(json!({"total": 1, "records": [{"instanceId": "123", "operationStatus": null}]})),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/openapi/instance/page"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let output = cli(&["status", "--json"], Some(&server.uri()), "cli-token");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("cli-token"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_overview_rejects_duplicate_pages_without_partial_output() {
    let server = MockServer::start().await;
    for current in [1, 2] {
        Mock::given(method("POST"))
            .and(path("/api/openapi/instance/page"))
            .and(body_json(json!({"current": current, "pageSize": 20})))
            .respond_with(success(
                json!({"total": 2, "records": [{"instanceId": "123", "operationStatus": "2"}]}),
            ))
            .expect(1)
            .mount(&server)
            .await;
    }
    let output = cli(&["status", "--json"], Some(&server.uri()), "cli-token");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("重复实例 ID"));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn single_instance_status_sanitizes_text_and_preserves_json_codes() {
    let server = MockServer::start().await;
    let code = "999\u{1b}[2J\u{7}";
    Mock::given(method("GET"))
        .and(path("/api/openapi/instance/status"))
        .and(query_param("instanceId", "123"))
        .respond_with(success(json!(code)))
        .expect(2)
        .mount(&server)
        .await;
    let output = cli(&["status", "123"], Some(&server.uri()), "cli-token");
    ok(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("未知状态"));
    assert!(!stdout.chars().any(|c| c.is_control() && c != '\n'));
    let value = parsed(&cli(
        &["status", "123", "--json"],
        Some(&server.uri()),
        "cli-token",
    ));
    assert_eq!(value["status"], code);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_keeps_unknown_code_visible_in_json() {
    let server = MockServer::start().await;
    Mock::given(path("/api/openapi/instance/status"))
        .and(query_param("instanceId", "123"))
        .respond_with(success(json!("999")))
        .expect(1)
        .mount(&server)
        .await;
    let value = parsed(&cli(
        &["status", "123", "--json"],
        Some(&server.uri()),
        "cli-token",
    ));
    assert_eq!(value["status"], "999");
    assert_eq!(value["label"], "未知状态");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_errors_exit_nonzero_without_stdout_or_token() {
    let server = MockServer::start().await;
    Mock::given(path("/api/openapi/instance/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"code": 1, "ok": false, "errCode": "EXPIRED\u{1b}[2J", "msg": "expired\u{7} cli-token", "data": null})))
        .expect(1).mount(&server).await;
    let output = cli(
        &["status", "123", "--json"],
        Some(&server.uri()),
        "cli-token",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("EXPIRED"));
    assert!(!stderr.contains("cli-token"));
    assert!(!stderr.chars().any(|c| c.is_control() && c != '\n'));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_arguments_and_empty_environment_token_make_no_api_requests() {
    let server = MockServer::start().await;
    let cases = [
        vec!["create", "--dry-run"],
        vec![
            "create",
            "--sku",
            "gpu",
            "--gpu-model",
            "A100",
            "--area",
            "华东一区",
            "--image-id",
            "42",
            "--image-type",
            "2",
            "--dry-run",
        ],
        vec!["images", "--image-type", "2"],
        vec!["images", "--image-type", "3", "--area", "华东一区"],
        vec!["list", "--size", "21"],
        vec!["images", "--size", "4"],
        vec!["list", "--page", "0"],
        vec!["list", "--all", "--page", "2"],
        vec!["start"],
        vec!["stop"],
        vec!["release"],
        vec!["release", "0", "--yes"],
        vec!["release", "-1", "--yes"],
        vec!["status", "9223372036854775808"],
        vec!["status", "not-an-id"],
        vec!["status", "0"],
        vec!["status", "123", "456"],
        vec!["release", "123", "--dry-run", "--yes"],
        vec!["--timeout", "0", "list"],
    ];
    for args in cases {
        let output = cli(&args, Some(&server.uri()), "cli-token");
        assert_eq!(output.status.code(), Some(2), "args={args:?}");
        assert!(output.stdout.is_empty());
    }
    for (flag, value) in [
        ("--count", "0"),
        ("--count", "9"),
        ("--image-type", "1"),
        ("--area", " "),
        ("--image-id", "0"),
    ] {
        let mut args = CREATE.to_vec();
        if let Some(index) = args.iter().position(|s| *s == flag) {
            args[index + 1] = value;
        } else {
            args.extend([flag, value]);
        }
        args.push("--dry-run");
        assert_eq!(
            cli(&args, Some(&server.uri()), "cli-token").status.code(),
            Some(2)
        );
    }
    for args in [vec!["status"], vec!["status", "123"]] {
        let empty_token = cli(&args, Some(&server.uri()), "");
        assert_eq!(empty_token.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&empty_token.stderr).contains("Token"));
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[test]
fn invalid_origins_are_usage_errors_without_echoing_url_credentials() {
    for base in [
        "not-a-url",
        "http://example.invalid",
        "https://user:private-password@example.invalid",
        "https://example.invalid?token=private-query-token",
        "https://example.invalid/path",
    ] {
        let output = cli(&["release", "123", "--dry-run", "--json"], Some(base), "");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("private-password"));
        assert!(!stderr.contains("private-query-token"));
    }
}
