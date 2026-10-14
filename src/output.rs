use crate::models::StatusOverview;
use anyhow::Result;
use comfy_table::Table;
use serde_json::Value;

// Tool hosts can contain Jupyter query tokens; config can contain nested secrets.
pub fn redact(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let key = key.to_ascii_lowercase().replace(['_', '-'], "");
                if key.contains("password")
                    || key.contains("token")
                    || key.contains("secret")
                    || key == "authorization"
                    || key == "config"
                    || key == "apikey"
                    || key == "credential"
                {
                    if !value.is_null() {
                        *value = Value::String("[REDACTED]".into());
                    }
                } else if (key.ends_with("host") || key.contains("url") || key.contains("domain"))
                    && value.is_string()
                {
                    redact_query(value);
                } else {
                    redact(value);
                }
            }
        }
        Value::Array(array) => array.iter_mut().for_each(redact),
        Value::String(text) if is_http_url(text) => redact_query(value),
        _ => {}
    }
}

fn is_http_url(text: &str) -> bool {
    text.split_once("://").is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

fn redact_query(value: &mut Value) {
    if let Value::String(text) = value {
        if let Some(index) = text.find('?') {
            text.truncate(index);
            text.push_str("?[REDACTED]");
        }
    }
}

pub fn json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn status(id: i64, code: &str) {
    println!(
        "{id}: {} ({})",
        crate::models::status_label(code),
        clean_text(code)
    );
}

pub fn status_overview(overview: &StatusOverview) {
    println!("当前计费中实例：{} 个", overview.total);
    if overview.records.is_empty() {
        println!("暂无计费中实例。");
        return;
    }
    println!(
        "{}",
        overview
            .summary
            .iter()
            .map(|item| format!(
                "{} ({})：{} 个",
                item.label,
                clean_text(&item.status),
                item.count
            ))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    table(
        &overview.records,
        &[
            ("ID", "instanceId"),
            ("名称", "instanceName"),
            ("状态", "operationStatus"),
            ("区域", "areaName"),
            ("GPU数", "count"),
        ],
    );
}

pub fn table(rows: &[Value], columns: &[(&str, &str)]) {
    let mut table = Table::new();
    table.set_header(columns.iter().map(|(label, _)| *label));
    for row in rows {
        table.add_row(columns.iter().map(|(_, key)| {
            if *key == "operationStatus" {
                let code = cell(&row[*key]);
                format!("{} ({code})", crate::models::status_label(&code))
            } else {
                cell(&row[*key])
            }
        }));
    }
    println!("{table}");
}

fn cell(value: &Value) -> String {
    match value {
        Value::Null => "-".into(),
        Value::String(text) => clean_text(text),
        other => other.to_string(),
    }
}

fn clean_text(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_nested_credentials_and_query_urls() {
        let mut value = json!({"instanceId": "123", "instanceName": "test?name", "instanceUtilList": [{"password": "ssh-secret", "host": "host?token=jupyter-secret", "config": "embedded-secret"}], "nested": {"api_key": "api-secret"}});
        redact(&mut value);
        let rendered = value.to_string();
        for secret in [
            "ssh-secret",
            "jupyter-secret",
            "embedded-secret",
            "api-secret",
        ] {
            assert!(!rendered.contains(secret));
        }
        assert_eq!(value["instanceId"], "123");
        assert_eq!(value["instanceName"], "test?name");
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_query_strings_for_case_insensitive_http_schemes() {
        let mut value = json!({
            "inputParams": {"image": "HTTPS://example.invalid/file?signature=input-secret"},
            "files": ["hTtPs://example.invalid/file?token=result-secret", "HTTP://example.invalid/?key=other-secret"]
        });
        redact(&mut value);
        assert_eq!(
            value["inputParams"]["image"],
            "HTTPS://example.invalid/file?[REDACTED]"
        );
        for secret in ["input-secret", "result-secret", "other-secret"] {
            assert!(!value.to_string().contains(secret));
        }
    }
}
