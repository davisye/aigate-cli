use aigate_cli::{auth::Token, client::Client, models::CreateRequest};
use serde_json::{json, Value};
use std::time::Duration;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> Client {
    Client::new(
        Token::parse("Bearer test-token").unwrap(),
        &server.uri(),
        Duration::from_secs(2),
        true,
    )
    .unwrap()
}

fn success(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(json!({"code": 0, "ok": true, "data": data, "msg": null, "errCode": null}))
}

fn create_request() -> CreateRequest {
    CreateRequest {
        sku_name: Some("4090D-48G".into()),
        gpu_model: None,
        area_name: "华东一区".into(),
        count: 2,
        image_id: 1096726823955333120,
        image_type: "3".into(),
    }
}

async fn status_mock(server: &MockServer, code: Value) {
    Mock::given(method("GET"))
        .and(path("/api/openapi/instance/status"))
        .and(query_param("instanceId", "123"))
        .respond_with(success(code))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn skus_sends_normalized_bearer_and_encodes_area() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/openapi/instance/skuList"))
        .and(header("authorization", "Bearer test-token"))
        .and(query_param("areaName", "华东一区 & extra"))
        .respond_with(success(
            json!([{"areaName": "华东一区", "skuName": "4090D-48G", "price": "199"}]),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let rows = client(&server).skus("华东一区 & extra").await.unwrap();
    assert_eq!(rows[0]["price"], "199");
}

#[tokio::test]
async fn instance_list_posts_paging_and_filter_without_losing_int64_ids() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/api/openapi/instance/page"))
        .and(header("authorization", "Bearer test-token"))
        .and(body_json(json!({"current": 2, "pageSize": 10, "operationStatus": "4"})))
        .respond_with(success(json!({"total": 2, "records": [{"instanceId": 1096749550260588544_i64}, {"instanceId": "1096749550260588545"}]})))
        .expect(1).mount(&server).await;
    let page = client(&server)
        .instances(2, 10, Some("4"), false)
        .await
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(
        page.records[0]["instanceId"].as_i64(),
        Some(1096749550260588544)
    );
    assert_eq!(page.records[1]["instanceId"], "1096749550260588545");
}

#[tokio::test]
async fn all_instances_reads_every_page_and_retains_filter() {
    let server = MockServer::start().await;
    for (current, records) in [
        (
            1,
            json!([{"instanceId": "1"}, {"instanceId": "2"}, {"instanceId": "3"}, {"instanceId": "4"}, {"instanceId": "5"}]),
        ),
        (2, json!([{"instanceId": "6"}])),
    ] {
        Mock::given(method("POST"))
            .and(path("/api/openapi/instance/page"))
            .and(body_json(
                json!({"current": current, "pageSize": 5, "operationStatus": "2"}),
            ))
            .respond_with(success(json!({"total": 6, "records": records})))
            .expect(1)
            .mount(&server)
            .await;
    }
    let page = client(&server)
        .instances(1, 5, Some("2"), true)
        .await
        .unwrap();
    assert_eq!(page.records.len(), 6);
    assert_eq!(page.records[5]["instanceId"], "6");
}

#[tokio::test]
async fn all_images_reads_pages_with_the_same_filters() {
    let server = MockServer::start().await;
    for (current, records) in [
        (1, json!([{"worksId": "42"}])),
        (2, json!([{"worksId": "43"}])),
    ] {
        Mock::given(method("POST")).and(path("/api/openapi/image/page"))
            .and(body_json(json!({"current": current, "pageSize": 5, "imageType": "2", "areaName": "华东一区", "skuName": "4090D-48G", "imageName": "Comfy", "imageVersion": "v1"})))
            .respond_with(success(json!({"total": 2, "records": records})))
            .expect(1).mount(&server).await;
    }
    let page = client(&server).images(json!({"current": 1, "pageSize": 5, "imageType": "2", "areaName": "华东一区", "skuName": "4090D-48G", "imageName": "Comfy", "imageVersion": "v1"}), true).await.unwrap();
    assert_eq!(page.records.len(), 2);
}

#[tokio::test]
async fn empty_page_before_total_is_reported_as_incomplete() {
    let server = MockServer::start().await;
    Mock::given(path("/api/openapi/instance/page"))
        .respond_with(success(json!({"total": 2, "records": []})))
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server)
        .instances(1, 20, None, true)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("不完整"));
}

#[tokio::test]
async fn info_uses_get_and_exact_instance_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/openapi/instance/get"))
        .and(query_param("instanceId", "1096749550260588544"))
        .respond_with(success(
            json!({"instanceId": "1096749550260588544", "instanceUtilList": null}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        client(&server).info(1096749550260588544).await.unwrap()["instanceId"],
        "1096749550260588544"
    );
}

#[tokio::test]
async fn status_preserves_known_unknown_and_numeric_codes() {
    for (code, expected) in [(json!("2"), "2"), (json!("999"), "999"), (json!(22), "22")] {
        let server = MockServer::start().await;
        status_mock(&server, code).await;
        assert_eq!(client(&server).status(123).await.unwrap(), expected);
    }
}

#[tokio::test]
async fn create_posts_exact_typed_payload_and_returns_instance() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/api/openapi/instance/start"))
        .and(header("authorization", "Bearer test-token"))
        .and(body_json(json!({"skuName": "4090D-48G", "areaName": "华东一区", "count": 2, "imageId": 1096726823955333120_i64, "imageType": "3"})))
        .respond_with(success(json!({"instanceId": "1096749550260588544", "operationStatus": "9"})))
        .expect(1).mount(&server).await;
    assert_eq!(
        client(&server).create(&create_request()).await.unwrap()["operationStatus"],
        "9"
    );
}

#[tokio::test]
async fn start_only_opens_explicit_existing_stopped_instance() {
    for code in ["22", "26"] {
        let server = MockServer::start().await;
        status_mock(&server, json!(code)).await;
        Mock::given(method("GET"))
            .and(path("/api/openapi/instance/open"))
            .and(query_param("instanceId", "123"))
            .respond_with(success(Value::Null))
            .expect(1)
            .mount(&server)
            .await;
        client(&server).start(123).await.unwrap();
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }
}

#[tokio::test]
async fn stop_checks_running_status_then_calls_close() {
    let server = MockServer::start().await;
    status_mock(&server, json!("2")).await;
    Mock::given(method("GET"))
        .and(path("/api/openapi/instance/close"))
        .and(query_param("instanceId", "123"))
        .respond_with(success(Value::Null))
        .expect(1)
        .mount(&server)
        .await;
    client(&server).stop(123).await.unwrap();
}

#[tokio::test]
async fn invalid_or_unknown_state_never_sends_power_request() {
    for code in ["2", "4", "9", "999"] {
        let server = MockServer::start().await;
        status_mock(&server, json!(code)).await;
        assert!(client(&server).start(123).await.is_err());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
    for code in ["22", "26", "4", "999"] {
        let server = MockServer::start().await;
        status_mock(&server, json!(code)).await;
        assert!(client(&server).stop(123).await.is_err());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn release_uses_documented_mutating_get_and_accepts_null_data() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/openapi/instance/release"))
        .and(header("authorization", "Bearer test-token"))
        .and(query_param("instanceId", "123"))
        .respond_with(success(Value::Null))
        .expect(1)
        .mount(&server)
        .await;
    client(&server).release(123).await.unwrap();
}

#[tokio::test]
async fn business_errors_and_inconsistent_success_envelopes_fail_safely() {
    for body in [
        json!({"code": 1, "ok": false, "msg": "invalid test-token", "errCode": "NO_STOCK", "data": null}),
        json!({"code": 0, "ok": false, "msg": "test-token", "data": "2"}),
        json!({"code": "0", "ok": true, "data": "2"}),
        json!({"ok": true, "data": "2"}),
        json!({"code": 0, "ok": true}),
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/api/openapi/instance/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let error = client(&server).status(123).await.err().unwrap().to_string();
        assert!(!error.contains("test-token"));
    }
}

#[tokio::test]
async fn http_errors_and_non_json_do_not_echo_response_bodies() {
    for response in [
        ResponseTemplate::new(401).set_body_string("secret response"),
        ResponseTemplate::new(502).set_body_string("<html>secret response</html>"),
        ResponseTemplate::new(200).set_body_string("not-json secret response"),
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/api/openapi/instance/status"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let error = client(&server).status(123).await.err().unwrap().to_string();
        assert!(!error.contains("secret response"));
    }
}

#[tokio::test]
async fn invalid_data_shapes_are_errors() {
    for (route, data) in [
        ("skuList", json!({})),
        ("page", json!({"records": []})),
        ("get", Value::Null),
        ("status", Value::Null),
    ] {
        let server = MockServer::start().await;
        Mock::given(path(format!("/api/openapi/instance/{route}")))
            .respond_with(success(data))
            .expect(1)
            .mount(&server)
            .await;
        let api = client(&server);
        let failed = match route {
            "skuList" => api.skus("华东一区").await.is_err(),
            "page" => api.instances(1, 20, None, false).await.is_err(),
            "get" => api.info(123).await.is_err(),
            _ => api.status(123).await.is_err(),
        };
        assert!(failed);
    }
}

#[tokio::test]
async fn create_missing_id_does_not_claim_success_or_retry() {
    let server = MockServer::start().await;
    Mock::given(path("/api/openapi/instance/start"))
        .respond_with(success(json!({})))
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server)
        .create(&create_request())
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("可能已提交"));
}

#[tokio::test]
async fn create_rejects_invalid_ids_without_claiming_success_or_retrying() {
    for id in [
        json!("not-an-id"),
        json!(""),
        json!("0"),
        json!("-1"),
        json!("+1"),
        json!(" 1"),
        json!("1.0"),
        json!("9223372036854775808"),
        json!(0),
        json!(-1),
        json!(1.5),
        json!(true),
        Value::Null,
        json!(9223372036854775808_u64),
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/api/openapi/instance/start"))
            .respond_with(success(json!({"instanceId": id})))
            .expect(1)
            .mount(&server)
            .await;
        let error = client(&server)
            .create(&create_request())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("有效 instanceId"), "id={id}");
        assert!(error.contains("可能已提交"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn create_preserves_valid_numeric_and_string_int64_ids() {
    for id in [json!(1), json!(i64::MAX), json!("9223372036854775807")] {
        let server = MockServer::start().await;
        Mock::given(path("/api/openapi/instance/start"))
            .respond_with(success(json!({"instanceId": id})))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            client(&server).create(&create_request()).await.unwrap()["instanceId"],
            id
        );
    }
}

#[tokio::test]
async fn api_error_fields_remove_controls_and_redact_credentials() {
    let server = MockServer::start().await;
    Mock::given(path("/api/openapi/instance/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 1, "ok": false, "data": null,
            "msg": "错误\u{1b}[2J\r\n\u{7}test-token\u{8}message",
            "errCode": "BAD\u{1b}]0;title\u{7}\u{0}"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server).status(123).await.unwrap_err().to_string();
    assert!(!error.chars().any(char::is_control));
    assert!(!error.contains("test-token"));
    assert!(error.contains("[REDACTED]"));
    assert!(error.contains("错误"));
}

#[tokio::test]
async fn no_mutation_is_retried_on_http_503() {
    for route in ["start", "open", "close", "release"] {
        let server = MockServer::start().await;
        if route == "open" || route == "close" {
            status_mock(&server, json!(if route == "open" { "22" } else { "2" })).await;
        }
        Mock::given(path(format!("/api/openapi/instance/{route}")))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&server)
            .await;
        let api = client(&server);
        let error = match route {
            "start" => api.create(&create_request()).await.err().unwrap(),
            "open" => api.start(123).await.err().unwrap(),
            "close" => api.stop(123).await.err().unwrap(),
            _ => api.release(123).await.err().unwrap(),
        }
        .to_string();
        assert!(error.contains("HTTP 503"));
        assert!(error.contains("可能已提交"));
    }
}

#[tokio::test]
async fn timeout_reports_uncertain_mutation_without_retry() {
    let server = MockServer::start().await;
    Mock::given(path("/api/openapi/instance/release"))
        .respond_with(success(Value::Null).set_delay(Duration::from_millis(200)))
        .expect(1)
        .mount(&server)
        .await;
    let api = Client::new(
        Token::parse("test-token").unwrap(),
        &server.uri(),
        Duration::from_millis(50),
        true,
    )
    .unwrap();
    let error = api.release(123).await.err().unwrap().to_string();
    assert!(error.contains("超时"));
    assert!(error.contains("可能已提交"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn redirect_is_never_followed_or_given_the_token() {
    let source = MockServer::start().await;
    let destination = MockServer::start().await;
    Mock::given(path("/api/openapi/instance/release"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("Location", format!("{}/sink", destination.uri())),
        )
        .expect(1)
        .mount(&source)
        .await;
    let error = client(&source)
        .release(123)
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("HTTP 307"));
    assert!(destination.received_requests().await.unwrap().is_empty());
}

#[test]
fn insecure_or_credential_bearing_origins_are_rejected() {
    for base in [
        "http://waas.aigate.cc",
        "https://user:private@waas.aigate.cc",
        "https://waas.aigate.cc/api/openapi",
        "https://waas.aigate.cc?token=private",
        "https://waas.aigate.cc#private",
        "invalid",
    ] {
        let error = Client::new(
            Token::parse("test-token").unwrap(),
            base,
            Duration::from_secs(1),
            true,
        )
        .err()
        .unwrap()
        .to_string();
        assert!(!error.contains("private"));
    }
}
