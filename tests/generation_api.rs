use aigate_cli::auth::{Profile, Token, WORKFLOW_BASE};
use aigate_cli::client::{normalize_origin, DEFAULT_BASE};
use aigate_cli::generation::{valid_task_id, ComfyClient, MediaType, UploadFile, WorkflowClient};
use serde_json::{json, Value};
use std::io::Write;
use std::time::Duration;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const WORKFLOW: &str = "/api/comfyui/open/v1/comfyui_workflow";
const COMFY: &str = "/api/openapi/comfyui";
const ID: &str = "1154715881075376128";

fn workflow(server: &MockServer) -> WorkflowClient {
    WorkflowClient::new(
        Token::parse("workflow-test").unwrap(),
        &server.uri(),
        Duration::from_secs(2),
        true,
    )
    .unwrap()
}
fn comfy(server: &MockServer) -> ComfyClient {
    ComfyClient::new(
        Token::parse("compute-test").unwrap(),
        &server.uri(),
        Duration::from_secs(2),
        true,
    )
    .unwrap()
}
fn success(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"code":0,"ok":true,"data":data}))
}
fn file() -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new()
        .prefix("图片 example ")
        .suffix(".png")
        .tempfile()
        .unwrap();
    file.write_all(b"image-bytes").unwrap();
    file
}
async fn mount_instances(server: &MockServer, rows: Value) {
    Mock::given(method("GET"))
        .and(path(format!("{COMFY}/comfyuiInstance/list")))
        .and(query_param("appId", "123"))
        .and(header("authorization", "Bearer compute-test"))
        .respond_with(success(rows))
        .expect(1)
        .mount(server)
        .await;
}
fn instance(host: &str) -> Value {
    json!({"appId":123,"instanceId":"456","instanceStatus":"2","comfyuiHost":host})
}

#[test]
fn credential_profiles_bind_to_distinct_official_origins() {
    let compute = normalize_origin(DEFAULT_BASE).unwrap();
    let workflow = normalize_origin("https://STUDIO.AIGATE.CC:443/").unwrap();
    assert!(Profile::Compute.require_keyring_origin(&compute).is_ok());
    assert!(Profile::Workflow.require_keyring_origin(&workflow).is_ok());
    assert!(Profile::Compute.require_keyring_origin(&workflow).is_err());
    assert!(Profile::Workflow.require_keyring_origin(&compute).is_err());
    assert_eq!(Profile::Workflow.origin(), WORKFLOW_BASE);
    assert_eq!(Profile::Workflow.environment(), "AIGATE_WORKFLOW_TOKEN");
}

#[test]
fn task_ids_are_reusable_identifiers_without_url_components_or_placeholders() {
    for id in [ID, "task-A_1", "550e8400-e29b-41d4-a716-446655440000"] {
        assert!(valid_task_id(id));
    }
    for id in [
        "https://example.invalid/?signature=secret",
        "file/path",
        "id?query",
        "id#fragment",
        "null",
        "NULL",
        "undefined",
        "OK",
        "false",
        "0",
        "-1",
        "9223372036854775808",
        "invalid task",
    ] {
        assert!(!valid_task_id(id), "{id}");
    }
}

#[tokio::test]
async fn workflow_submission_preserves_application_parameters_and_task_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path(format!("{WORKFLOW}/create")))
        .and(header("authorization","Bearer workflow-test"))
        .and(body_json(json!({"appId":"app/slash","inputParams":{"seed":9223372036854775807_i64,"prompt":"中文"}})))
        .respond_with(success(json!({"taskId":1154715881075376128_i64})))
        .expect(1).mount(&server).await;
    let data = workflow(&server)
        .run("app/slash", &json!({"seed":i64::MAX,"prompt":"中文"}))
        .await
        .unwrap();
    assert_eq!(data["taskId"], ID);
}

#[tokio::test]
async fn comfy_submission_sends_string_ids_and_allow_input() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("{COMFY}/comfyuiTask/create")))
        .and(header("authorization", "Bearer compute-test"))
        .and(body_json(
            json!({"appId":ID,"instanceId":"9223372036854775807","allowInput":{"text":"example"}}),
        ))
        .respond_with(success(json!(ID)))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        comfy(&server)
            .run(1154715881075376128, i64::MAX, &json!({"text":"example"}))
            .await
            .unwrap()["taskId"],
        ID
    );
}

#[tokio::test]
async fn status_retains_unknown_codes_and_reports_execution_failure_with_successful_query() {
    let server = MockServer::start().await;
    for (suffix, status, expected) in [
        ("/getTask", json!(6), "上传中"),
        ("/getTask", json!(4), "失败"),
        ("/getTask", json!(999), "未知"),
    ] {
        server.reset().await;
        Mock::given(method("GET"))
            .and(path(format!("{WORKFLOW}{suffix}")))
            .and(query_param("taskId", ID))
            .respond_with(success(json!({"taskId":ID,"status":status,"files":null})))
            .expect(1)
            .mount(&server)
            .await;
        let result = workflow(&server).status(ID).await.unwrap();
        assert_eq!(result["status"], status);
        assert_eq!(result["label"], expected);
        server.verify().await;
    }
    Mock::given(method("GET")).and(path(format!("{COMFY}/comfyuiTask/status"))).and(query_param("taskId",ID))
        .respond_with(success(json!({"taskId":1154715881075376128_i64,"status":2,"files":[{"downloadUrl":"https://example.invalid/file?signature=secret"}]})))
        .expect(1).mount(&server).await;
    let result = comfy(&server).status(ID).await.unwrap();
    assert_eq!(result["label"], "成功");
    assert_eq!(result["taskId"], ID);
}

#[tokio::test]
async fn malformed_status_and_mismatched_task_are_rejected() {
    for data in [
        json!({"taskId":"other","status":2}),
        json!({"taskId":ID}),
        json!({"taskId":ID,"status":2.5}),
        json!({"taskId":ID,"status":2,"files":{}}),
        json!({"taskId":ID,"status":2,"files":[null]}),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(success(data))
            .expect(1)
            .mount(&server)
            .await;
        assert!(workflow(&server).status(ID).await.is_err());
    }
}

#[tokio::test]
async fn workflow_file_results_preserve_strings_and_objects() {
    let server = MockServer::start().await;
    let files = json!(["https://example.invalid/image?signature=secret",{"url":"https://example.invalid/video"}]);
    Mock::given(method("GET"))
        .respond_with(success(json!({"taskId":ID,"status":2,"files":files})))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(workflow(&server).status(ID).await.unwrap()["files"], files);
}

#[tokio::test]
async fn comfy_documentation_placeholder_is_not_a_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"code":0,"ok":false,"data":{"":{}}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let error = comfy(&server)
        .run(123, 456, &json!({}))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("API 错误") && error.contains("可能已提交"));
}

#[tokio::test]
async fn placeholder_or_missing_submission_id_is_uncertain_and_never_retried() {
    for data in [
        json!({"":{}}),
        json!(null),
        json!({"taskId":0}),
        json!({"taskId":""}),
        json!({"taskId":1.5}),
        json!("OK"),
        json!("null"),
        json!("opaque-untyped"),
        json!({"taskId":"null"}),
        json!({"taskId":"https://example.invalid/?signature=secret"}),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(success(data))
            .expect(1)
            .mount(&server)
            .await;
        let error = comfy(&server)
            .run(123, 456, &json!({}))
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("可能已提交"));
    }
}

#[tokio::test]
async fn named_opaque_task_id_is_preserved_for_status_queries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(success(json!({"taskId":"task-A_1"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(query_param("taskId", "task-A_1"))
        .respond_with(success(
            json!({"taskId":"task-A_1","status":0,"files":null}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let client = workflow(&server);
    let created = client.run("app", &json!({})).await.unwrap();
    let id = created["taskId"].as_str().unwrap();
    assert_eq!(id, "task-A_1");
    assert_eq!(client.status(id).await.unwrap()["taskId"], id);
}

#[tokio::test]
async fn generation_errors_redact_token_and_control_characters() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"code":1,"ok":false,"msg":"workflow-test\u{1b}[31m失败","data":null}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let error = workflow(&server)
        .run("app", &json!({}))
        .await
        .unwrap_err()
        .to_string();
    assert!(!error.contains("workflow-test"));
    assert!(!error.contains('\u{1b}'));
    assert!(error.contains("可能已提交"));
}

#[tokio::test]
async fn generation_does_not_redirect_or_retry_submission() {
    for response in [
        ResponseTemplate::new(503),
        ResponseTemplate::new(307).insert_header("location", "/redirected"),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!("{WORKFLOW}/create")))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        assert!(workflow(&server).run("app", &json!({})).await.is_err());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn timeout_reports_uncertain_submission_without_replay() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(success(json!({"taskId":ID})).set_delay(Duration::from_millis(200)))
        .expect(1)
        .mount(&server)
        .await;
    let client = WorkflowClient::new(
        Token::parse("workflow-test").unwrap(),
        &server.uri(),
        Duration::from_millis(40),
        true,
    )
    .unwrap();
    let error = client.run("app", &json!({})).await.unwrap_err().to_string();
    assert!(error.contains("超时") && error.contains("可能已提交"));
}

#[tokio::test]
async fn comfy_lists_require_arrays_of_objects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("{COMFY}/comfyuiApp/list")))
        .and(header("authorization", "Bearer compute-test"))
        .respond_with(success(
            json!([{"appId":1154715881075376128_i64,"allowInput":{"prompt":"text"}}]),
        ))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        comfy(&server).apps().await.unwrap()[0]["appId"],
        json!(1154715881075376128_i64)
    );
    mount_instances(&server, json!([instance("example.invalid")])).await;
    assert_eq!(
        comfy(&server).instances(123).await.unwrap()[0]["instanceStatus"],
        "2"
    );
    for data in [json!({}), json!([null])] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(success(data))
            .mount(&server)
            .await;
        assert!(comfy(&server).apps().await.is_err());
    }
}

#[tokio::test]
async fn workflow_upload_streams_file_with_media_type_and_platform_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("{WORKFLOW}/uploadFile")))
        .and(query_param("media_type", "image"))
        .and(header("authorization", "Bearer workflow-test"))
        .respond_with(success(json!({"url":"https://example.invalid/image"})))
        .expect(1)
        .mount(&server)
        .await;
    let file = file();
    let upload = UploadFile::inspect(file.path(), Some(MediaType::Image)).unwrap();
    workflow(&server)
        .upload(&upload, MediaType::Image)
        .await
        .unwrap();
    let requests = server.received_requests().await.unwrap();
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(body.contains("name=\"file\"") && body.contains("image-bytes"));
    assert!(!body.contains(&file.path().parent().unwrap().to_string_lossy().to_string()));
}

#[test]
fn workflow_upload_checks_documented_size_limits_before_network() {
    let file = file();
    file.as_file().set_len(5_000_001).unwrap();
    assert!(UploadFile::inspect(file.path(), Some(MediaType::Image)).is_err());
    assert!(UploadFile::inspect(file.path(), Some(MediaType::Video)).is_ok());
    file.as_file().set_len(20_000_001).unwrap();
    assert!(UploadFile::inspect(file.path(), Some(MediaType::Audio)).is_err());
    assert!(UploadFile::inspect(file.path(), None).is_ok());
    assert!(UploadFile::inspect(file.path().parent().unwrap(), None).is_err());
}

#[tokio::test]
async fn instance_upload_preserves_host_path_and_query_but_never_sends_bearer() {
    let platform = MockServer::start().await;
    let upload_server = MockServer::start().await;
    mount_instances(
        &platform,
        json!([instance(&format!(
            "{}/prefix/?token=instance-secret",
            upload_server.uri()
        ))]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/prefix/api/upload/image"))
        .and(query_param("token", "instance-secret"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"name":"image.png","subfolder":"","type":"input"})),
        )
        .expect(1)
        .mount(&upload_server)
        .await;
    let file = file();
    comfy(&platform)
        .upload(123, 456, &UploadFile::inspect(file.path(), None).unwrap())
        .await
        .unwrap();
    let requests = upload_server.received_requests().await.unwrap();
    assert!(!requests[0].headers.contains_key("authorization"));
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(
        body.contains("name=\"image\"") && body.contains("name=\"type\"") && body.contains("input")
    );
    assert!(!body.contains("compute-test"));
}

#[tokio::test]
async fn instance_lookup_rejects_missing_duplicate_paused_or_wrong_application() {
    let mut paused = instance("https://example.invalid");
    paused["instanceStatus"] = json!(6);
    let mut other_app = instance("https://example.invalid");
    other_app["appId"] = json!(999);
    for rows in [
        json!([]),
        json!([
            instance("https://example.invalid"),
            instance("https://example.invalid")
        ]),
        json!([paused]),
        json!([other_app]),
    ] {
        let server = MockServer::start().await;
        mount_instances(&server, rows).await;
        let file = file();
        assert!(comfy(&server)
            .upload(123, 456, &UploadFile::inspect(file.path(), None).unwrap())
            .await
            .is_err());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn unsafe_instance_hosts_fail_without_exposing_embedded_credentials() {
    for host in [
        "http://example.invalid?token=host-secret",
        "https://user:host-secret@example.invalid",
        "https://example.invalid#host-secret",
        "file:///host-secret",
        "bad host-secret",
    ] {
        let server = MockServer::start().await;
        mount_instances(&server, json!([instance(host)])).await;
        let file = file();
        let error = comfy(&server)
            .upload(123, 456, &UploadFile::inspect(file.path(), None).unwrap())
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains("host-secret"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn upload_rejects_placeholder_results_and_redirects_without_replay() {
    for response in [
        ResponseTemplate::new(200).set_body_json(json!({"code":0,"data":null})),
        ResponseTemplate::new(307).insert_header("location", "/other"),
        ResponseTemplate::new(503),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let file = file();
        let error = workflow(&server)
            .upload(
                &UploadFile::inspect(file.path(), None).unwrap(),
                MediaType::Image,
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("可能已上传"));
    }
    for response in [
        ResponseTemplate::new(200)
            .set_body_json(json!({"name":"image.png","subfolder":"","type":"output"})),
        ResponseTemplate::new(302).insert_header("location", "/other?token=host-secret"),
        ResponseTemplate::new(500),
    ] {
        let platform = MockServer::start().await;
        let upload_server = MockServer::start().await;
        mount_instances(&platform, json!([instance(&upload_server.uri())])).await;
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&upload_server)
            .await;
        let file = file();
        let error = comfy(&platform)
            .upload(123, 456, &UploadFile::inspect(file.path(), None).unwrap())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("可能已上传") && !error.contains("host-secret"));
        assert_eq!(upload_server.received_requests().await.unwrap().len(), 1);
    }
}
