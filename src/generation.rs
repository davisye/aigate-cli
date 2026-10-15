//! Workflow and Comfy task APIs. Instance uploads deliberately use no platform credentials.
use crate::auth::Token;
use crate::client::Client;
use crate::models::parse_positive_id;
use anyhow::{anyhow, bail, Context, Result};
use reqwest::{multipart, Method, Url};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const WORKFLOW_PREFIX: &str = "/api/comfyui/open/v1/comfyui_workflow";
pub const COMFY_PREFIX: &str = "/api/openapi/comfyui";
const TASK_UNCERTAIN: &str = "；任务可能已提交，请先在控制台核对任务记录，不要直接重复提交";
const UPLOAD_UNCERTAIN: &str = "；文件可能已上传，请先核对上传结果，不要直接重复上传";

/// Parameters vary by application; accept only a bounded JSON object, without echoing its content.
pub fn read_input(path: &Path) -> Result<Value> {
    const LIMIT: u64 = 16 * 1024 * 1024;
    if !path.metadata().context("无法检查输入参数文件")?.is_file() {
        bail!("输入参数必须是普通 JSON 文件");
    }
    let file = std::fs::File::open(path).map_err(|_| anyhow!("无法打开输入参数文件"))?;
    if !file.metadata().context("无法检查输入参数文件")?.is_file() {
        bail!("输入参数必须是普通 JSON 文件");
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .context("无法读取输入参数文件")?;
    if bytes.len() as u64 > LIMIT {
        bail!("输入参数文件超过 16 MiB");
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| anyhow!("输入参数文件不是有效 JSON"))?;
    require_object(&value)?;
    Ok(value)
}

fn require_object(value: &Value) -> Result<()> {
    if !value.is_object() {
        bail!("输入参数必须是 JSON object");
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub enum MediaType {
    Image,
    Video,
    Audio,
}

impl MediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
        }
    }

    fn limit(self) -> u64 {
        match self {
            Self::Image => 5_000_000,
            Self::Video | Self::Audio => 20_000_000,
        }
    }
}

pub struct UploadFile {
    path: PathBuf,
    pub name: String,
    pub bytes: u64,
}

impl UploadFile {
    pub fn inspect(path: &Path, media: Option<MediaType>) -> Result<Self> {
        let metadata = path.metadata().map_err(|_| anyhow!("无法检查上传文件"))?;
        if !metadata.is_file() || metadata.len() == 0 {
            bail!("上传文件必须是非空普通文件");
        }
        if media.is_some_and(|media| metadata.len() > media.limit()) {
            bail!("Workflow 上传超过大小限制：图片 5 MB，视频或音频 20 MB");
        }
        let name = path
            .file_name()
            .and_then(|v| v.to_str())
            .filter(|name| !name.chars().any(char::is_control))
            .context("上传文件名必须为有效 UTF-8 且不含控制字符")?;
        Ok(Self {
            path: path.to_owned(),
            name: name.to_owned(),
            bytes: metadata.len(),
        })
    }

    async fn part(&self, media: Option<MediaType>) -> Result<multipart::Part> {
        // Recheck immediately before opening the streaming body.
        Self::inspect(&self.path, media)?;
        multipart::Part::file(&self.path)
            .await
            .map_err(|_| anyhow!("无法读取上传文件"))
    }
}

pub fn valid_task_id(raw: &str) -> bool {
    !raw.is_empty()
        && raw.len() <= 256
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        && raw.as_bytes()[0].is_ascii_alphanumeric()
        && !["null", "undefined", "true", "false", "ok", "success"]
            .iter()
            .any(|reserved| raw.eq_ignore_ascii_case(reserved))
        && (!raw.bytes().all(|b| b.is_ascii_digit()) || raw.parse::<i64>().is_ok_and(|id| id > 0))
        && !raw.starts_with('-')
}

fn task_id(value: &Value) -> Option<String> {
    match value {
        Value::String(id) if valid_task_id(id) => Some(id.clone()),
        Value::Number(id) => id.as_i64().filter(|id| *id > 0).map(|id| id.to_string()),
        _ => None,
    }
}

fn submitted(mut data: Value) -> Result<Value> {
    // A named taskId may be opaque. An untyped scalar must be a positive numeric ID,
    // so status messages and placeholder strings cannot masquerade as accepted tasks.
    let id = if data.is_object() {
        data.get("taskId").and_then(task_id)
    } else {
        data.as_str()
            .and_then(parse_positive_id)
            .or_else(|| data.as_i64().filter(|id| *id > 0))
            .map(|id| id.to_string())
    }
    .ok_or_else(|| anyhow!("提交响应缺少有效 taskId{TASK_UNCERTAIN}"))?;
    if !data.is_object() {
        data = json!({});
    }
    data["taskId"] = json!(id);
    Ok(data)
}

fn task_status(mut data: Value, expected: &str, workflow: bool) -> Result<Value> {
    let id = data
        .get("taskId")
        .and_then(task_id)
        .context("任务响应缺少有效 taskId")?;
    if id != expected {
        bail!("任务响应 taskId 与查询不一致");
    }
    let code = match &data["status"] {
        Value::Number(n) if n.is_i64() => n.to_string(),
        Value::String(s) if !s.is_empty() && !s.chars().any(char::is_control) => s.clone(),
        _ => bail!("任务响应缺少有效 status"),
    };
    if data.get("files").is_some_and(|files| {
        !files.is_null()
            && !files.as_array().is_some_and(|files| {
                files.iter().all(|file| {
                    file.is_object() || workflow && file.as_str().is_some_and(|s| !s.is_empty())
                })
            })
    }) {
        bail!("任务响应包含无效 files");
    }
    let label = match code.as_str() {
        "0" => "排队中",
        "1" => "生成中",
        "2" => "成功",
        "3" => "已取消",
        "4" => "失败",
        "5" => "超时",
        "6" if workflow => "上传中",
        _ => "未知",
    };
    data["taskId"] = json!(id);
    data["label"] = json!(label);
    Ok(data)
}

pub struct WorkflowClient(Client);

impl WorkflowClient {
    pub fn new(token: Token, base: &str, timeout: Duration, no_proxy: bool) -> Result<Self> {
        Ok(Self(Client::with_prefix(
            token,
            base,
            timeout,
            no_proxy,
            WORKFLOW_PREFIX,
        )?))
    }

    pub async fn run(&self, app: &str, input: &Value) -> Result<Value> {
        if app.trim().is_empty() || app.chars().any(char::is_control) {
            bail!("appId 必须为非空文本且不含控制字符");
        }
        require_object(input)?;
        let data = self
            .0
            .send_with_hint(
                self.0
                    .request(Method::POST, "/create")
                    .json(&json!({"appId":app,"inputParams":input})),
                TASK_UNCERTAIN,
            )
            .await?;
        submitted(data)
    }

    pub async fn status(&self, id: &str) -> Result<Value> {
        if !valid_task_id(id) {
            bail!("taskId 无效");
        }
        let data = self
            .0
            .send_with_hint(
                self.0
                    .request(Method::GET, "/getTask")
                    .query(&[("taskId", id)]),
                "",
            )
            .await?;
        task_status(data, id, true)
    }

    pub async fn upload(&self, file: &UploadFile, media: MediaType) -> Result<Value> {
        let form = multipart::Form::new().part("file", file.part(Some(media)).await?);
        let data = self
            .0
            .send_with_hint(
                self.0
                    .request(Method::POST, "/uploadFile")
                    .query(&[("media_type", media.as_str())])
                    .multipart(form),
                UPLOAD_UNCERTAIN,
            )
            .await?;
        if !(data.as_str().is_some_and(|s| !s.is_empty())
            || data.as_object().is_some_and(|o| !o.is_empty()))
        {
            bail!("上传响应缺少有效文件结果{UPLOAD_UNCERTAIN}");
        }
        Ok(data)
    }
}

pub struct ComfyClient {
    api: Client,
    upload: reqwest::Client,
}

impl ComfyClient {
    pub fn new(token: Token, base: &str, timeout: Duration, no_proxy: bool) -> Result<Self> {
        // Never reuse the platform client: instance hosts must not receive the platform Bearer token.
        let mut builder = reqwest::Client::builder()
            .user_agent(concat!("aigate-cli/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(10)))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never());
        if no_proxy {
            builder = builder.no_proxy();
        }
        Ok(Self {
            api: Client::with_prefix(token, base, timeout, no_proxy, COMFY_PREFIX)?,
            upload: builder.build().context("无法初始化上传 client")?,
        })
    }

    async fn rows(&self, path: &str, app: Option<i64>) -> Result<Vec<Value>> {
        let mut request = self.api.request(Method::GET, path);
        if let Some(app) = app {
            request = request.query(&[("appId", app)]);
        }
        let data = self.api.send_with_hint(request, "").await?;
        let rows = data
            .as_array()
            .filter(|rows| rows.iter().all(Value::is_object))
            .context("Comfy API 未返回有效列表")?;
        Ok(rows.clone())
    }

    pub async fn apps(&self) -> Result<Vec<Value>> {
        self.rows("/comfyuiApp/list", None).await
    }

    pub async fn instances(&self, app: i64) -> Result<Vec<Value>> {
        if app <= 0 {
            bail!("appId 必须为正 int64");
        }
        self.rows("/comfyuiInstance/list", Some(app)).await
    }

    pub async fn run(&self, app: i64, instance: i64, input: &Value) -> Result<Value> {
        if app <= 0 || instance <= 0 {
            bail!("appId 和 instanceId 必须为正 int64");
        }
        require_object(input)?;
        let body =
            json!({"appId":app.to_string(),"instanceId":instance.to_string(),"allowInput":input});
        let data = self
            .api
            .send_with_hint(
                self.api
                    .request(Method::POST, "/comfyuiTask/create")
                    .json(&body),
                TASK_UNCERTAIN,
            )
            .await?;
        submitted(data)
    }

    pub async fn status(&self, id: &str) -> Result<Value> {
        if !valid_task_id(id) {
            bail!("taskId 无效");
        }
        let data = self
            .api
            .send_with_hint(
                self.api
                    .request(Method::GET, "/comfyuiTask/status")
                    .query(&[("taskId", id)]),
                "",
            )
            .await?;
        task_status(data, id, false)
    }

    pub async fn upload(&self, app: i64, instance: i64, file: &UploadFile) -> Result<Value> {
        if instance <= 0 {
            bail!("instanceId 必须为正 int64");
        }
        let rows = self.instances(app).await?;
        let matching: Vec<_> = rows
            .iter()
            .filter(|row| task_id(&row["instanceId"]).is_some_and(|id| id == instance.to_string()))
            .collect();
        if matching.len() != 1 {
            bail!("应用实例列表中未找到唯一的目标实例");
        }
        let row = matching[0];
        if !task_id(&row["appId"]).is_some_and(|id| id == app.to_string()) {
            bail!("目标实例的 appId 与请求不一致");
        }
        if row["instanceStatus"] != "2" && row["instanceStatus"] != 2 {
            bail!("Comfy 上传仅允许 instanceStatus=2 的运行中实例");
        }
        let url = upload_url(
            row["comfyuiHost"]
                .as_str()
                .context("目标实例缺少 comfyuiHost")?,
        )?;
        let form = multipart::Form::new()
            .part("image", file.part(None).await?)
            .text("type", "input");
        let response = self
            .upload
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|_| anyhow!("实例上传请求失败{UPLOAD_UNCERTAIN}"))?;
        if !response.status().is_success() {
            bail!(
                "实例上传返回 HTTP {}{UPLOAD_UNCERTAIN}",
                response.status().as_u16()
            );
        }
        let data: Value = response
            .json()
            .await
            .map_err(|_| anyhow!("实例上传返回无效 JSON{UPLOAD_UNCERTAIN}"))?;
        if !data["name"].as_str().is_some_and(|s| !s.is_empty())
            || !data["subfolder"].is_string()
            || data["type"] != "input"
        {
            bail!("实例上传响应缺少有效 name/subfolder/type{UPLOAD_UNCERTAIN}");
        }
        Ok(data)
    }
}

fn upload_url(host: &str) -> Result<Url> {
    if host.is_empty() || host.chars().any(|c| c.is_control() || c.is_whitespace()) {
        bail!("实例上传地址无效");
    }
    let raw = if host.contains("://") {
        host.to_owned()
    } else {
        format!("https://{host}")
    };
    let mut url = Url::parse(&raw).map_err(|_| anyhow!("实例上传地址无效"))?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        bail!("实例上传地址必须使用 HTTPS，不可含用户信息或 fragment");
    }
    url.set_path(&format!(
        "{}/api/upload/image",
        url.path().trim_end_matches('/')
    ));
    Ok(url)
}
