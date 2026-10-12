use crate::auth::Token;
use crate::models::{parse_positive_id, status_label, CreateRequest, Page};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Client as Http, RequestBuilder, Url};
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_BASE: &str = "https://waas.aigate.cc";
const PREFIX: &str = "/api/openapi";
const UNCERTAIN: &str = "；操作可能已提交，请查询实例列表或状态后再决定是否重试";

pub fn normalize_origin(base: &str) -> Result<Url> {
    let base = Url::parse(base).map_err(|_| anyhow!("API base URL 格式无效"))?;
    let loopback = matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !(base.scheme() == "https" || base.scheme() == "http" && loopback)
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || base.path() != "/"
        || base.host_str().is_none()
    {
        bail!("API base URL 必须为 HTTPS origin；本地测试可用 HTTP loopback，不可含用户信息、路径或查询串");
    }
    Ok(base)
}

pub struct Client {
    http: Http,
    base: Url,
    token: Token,
}

impl Client {
    pub fn new(token: Token, base: &str, timeout: Duration, no_proxy: bool) -> Result<Self> {
        let base = normalize_origin(base)?;
        let mut authorization = HeaderValue::from_str(&format!("Bearer {}", token.expose()))
            .map_err(|_| anyhow!("Token 不能用于 HTTP 认证"))?;
        authorization.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        let mut builder = Http::builder()
            .default_headers(headers)
            .user_agent(concat!("aigate-cli/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(10)))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never());
        if no_proxy {
            builder = builder.no_proxy();
        }
        Ok(Self {
            http: builder.build().context("无法初始化 HTTP client")?,
            base,
            token,
        })
    }

    fn url(&self, path: &str) -> Url {
        let mut url = self.base.clone();
        url.set_path(&format!("{PREFIX}{path}"));
        url
    }

    async fn send(&self, request: RequestBuilder, mutation: bool) -> Result<Value> {
        let uncertain = if mutation { UNCERTAIN } else { "" };
        let response = request.send().await.map_err(|error| {
            let reason = if error.is_timeout() {
                "请求超时"
            } else if error.is_connect() {
                "连接失败（可尝试 --no-proxy）"
            } else {
                "网络请求失败"
            };
            anyhow!("{reason}{uncertain}")
        })?;
        let status = response.status();
        if !status.is_success() {
            // Never echo raw HTML, request URLs, headers, or authentication errors.
            bail!("API 返回 HTTP {}{uncertain}", status.as_u16());
        }
        let envelope: Value = response
            .json()
            .await
            .map_err(|_| anyhow!("API 返回无效 JSON{uncertain}"))?;
        let code = envelope
            .get("code")
            .and_then(Value::as_i64)
            .ok_or_else(|| anyhow!("API 响应缺少有效 code{uncertain}"))?;
        if code != 0 || envelope.get("ok").is_some_and(|v| v != &Value::Bool(true)) {
            let msg = self.safe_message(
                envelope
                    .get("msg")
                    .and_then(Value::as_str)
                    .unwrap_or("请求被拒绝"),
            );
            let err_code = self.safe_message(
                envelope
                    .get("errCode")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
            );
            bail!("API 错误 code={code}, errCode={err_code}: {msg}{uncertain}");
        }
        envelope
            .get("data")
            .cloned()
            .ok_or_else(|| anyhow!("API 响应缺少 data{uncertain}"))
    }

    fn safe_message(&self, raw: &str) -> String {
        raw.replace(self.token.expose(), "[REDACTED]")
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(300)
            .collect()
    }

    pub async fn skus(&self, area: &str) -> Result<Vec<Value>> {
        let data = self
            .send(
                self.http
                    .get(self.url("/instance/skuList"))
                    .query(&[("areaName", area)]),
                false,
            )
            .await?;
        let rows = data.as_array().context("API 未返回有效 SKU 列表")?;
        if rows.iter().any(|v| !v.is_object()) {
            bail!("API SKU 列表包含无效记录");
        }
        Ok(rows.clone())
    }

    pub async fn instances(
        &self,
        page: u32,
        size: u8,
        operation_status: Option<&str>,
        all: bool,
    ) -> Result<Page> {
        let mut body = json!({"current": page, "pageSize": size});
        if let Some(status) = operation_status {
            body["operationStatus"] = json!(status);
        }
        self.pages("/instance/page", body, all).await
    }

    pub async fn images(&self, body: Value, all: bool) -> Result<Page> {
        self.pages("/image/page", body, all).await
    }

    async fn pages(&self, path: &str, mut body: Value, all: bool) -> Result<Page> {
        let mut records = Vec::new();
        for _ in 0..1000 {
            let data = self
                .send(self.http.post(self.url(path)).json(&body), false)
                .await?;
            let mut page: Page =
                serde_json::from_value(data).context("API 未返回有效分页数据（total/records）")?;
            if page.records.iter().any(|v| !v.is_object()) {
                bail!("API 分页数据包含无效记录");
            }
            if !all {
                return Ok(page);
            }
            let empty = page.records.is_empty();
            records.append(&mut page.records);
            if records.len() as u64 >= page.total {
                return Ok(Page {
                    total: page.total,
                    records,
                });
            }
            if empty {
                bail!("分页提前返回空页，结果可能不完整；请重新查询");
            }
            let current = body["current"].as_u64().context("分页页码无效")?;
            body["current"] = json!(current + 1);
        }
        bail!("分页超过 1000 页，请使用单页查询");
    }

    pub async fn info(&self, id: i64) -> Result<Value> {
        let data = self
            .send(
                self.http
                    .get(self.url("/instance/get"))
                    .query(&[("instanceId", id)]),
                false,
            )
            .await?;
        if !data.is_object() {
            bail!("API 未返回有效实例详情");
        }
        Ok(data)
    }

    pub async fn status(&self, id: i64) -> Result<String> {
        let data = self
            .send(
                self.http
                    .get(self.url("/instance/status"))
                    .query(&[("instanceId", id)]),
                false,
            )
            .await?;
        match data {
            Value::String(code) if !code.is_empty() => Ok(code),
            Value::Number(code) if code.is_i64() => Ok(code.to_string()),
            _ => bail!("API 未返回有效实例状态"),
        }
    }

    pub async fn create(&self, request: &CreateRequest) -> Result<Value> {
        let data = self
            .send(
                self.http.post(self.url("/instance/start")).json(request),
                true,
            )
            .await?;
        let id = data.get("instanceId");
        if !id.is_some_and(|v| {
            v.as_str().and_then(parse_positive_id).is_some() || v.as_i64().is_some_and(|n| n > 0)
        }) {
            bail!("创建响应缺少有效 instanceId{UNCERTAIN}");
        }
        Ok(data)
    }

    pub async fn start(&self, id: i64) -> Result<()> {
        let status = self.status(id).await?;
        if !matches!(status.as_str(), "22" | "26") {
            bail!(
                "实例 {id} 为 {}({status})，开机仅允许状态 22 或 26",
                status_label(&status)
            );
        }
        self.action("open", id).await
    }

    pub async fn stop(&self, id: i64) -> Result<()> {
        let status = self.status(id).await?;
        if status != "2" {
            bail!(
                "实例 {id} 为 {}({status})，关机仅允许运行中状态 2",
                status_label(&status)
            );
        }
        self.action("close", id).await
    }

    pub async fn release(&self, id: i64) -> Result<()> {
        self.action("release", id).await
    }

    async fn action(&self, name: &str, id: i64) -> Result<()> {
        self.send(
            self.http
                .get(self.url(&format!("/instance/{name}")))
                .query(&[("instanceId", id)]),
            true,
        )
        .await?;
        Ok(())
    }
}
