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
                } else if (key == "host" || key.contains("url") || key.contains("domain"))
                    && value.is_string()
                {
                    redact_query(value);
                } else {
                    redact(value);
                }
            }
        }
        Value::Array(array) => array.iter_mut().for_each(redact),
        Value::String(text) if text.starts_with("http://") || text.starts_with("https://") => {
            redact_query(value)
        }
        _ => {}
    }
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
        Value::String(text) => text
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect(),
        other => other.to_string(),
    }
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
}
