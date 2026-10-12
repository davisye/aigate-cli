use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const AREAS: [&str; 2] = ["华东一区", "华东二区"];

pub fn parse_positive_id(raw: &str) -> Option<i64> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    raw.parse::<i64>().ok().filter(|n| *n > 0)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_model: Option<String>,
    pub area_name: String,
    pub count: u8,
    pub image_id: i64,
    pub image_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Page {
    pub total: u64,
    pub records: Vec<Value>,
}

// Keep raw codes visible, including codes added after this CLI was written.
pub fn status_label(code: &str) -> &str {
    match code {
        "1" => "实例创建成功",
        "2" => "运行中",
        "3" => "正在保存",
        "4" => "实例已释放",
        "8" => "正在拉取镜像",
        "9" => "正在初始化实例",
        "10" => "已保存",
        "11" => "正在关机",
        "13" => "实例开机中",
        "15" => "正在更换镜像",
        "19" => "实例迁移中",
        "20" => "实例重启中",
        "21" => "实例已清理",
        "22" => "实例已关机",
        "23" => "保存中",
        "24" => "实例不释放镜像正在保存",
        "26" => "实例关机且已上传",
        _ => "未知状态",
    }
}
