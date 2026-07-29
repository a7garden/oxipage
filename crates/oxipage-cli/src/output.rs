//! 출력 포맷팅 — `--json` 전역 옵션 처리 (doc/04 §4.3).

use serde_json::Value;

pub struct Output {
    pub json: bool,
}

impl Output {
    pub fn new(json: bool) -> Self {
        Output { json }
    }

    /// JSON 모드면 그대로 출력, 사람 모드면 요약 출력 (단건 data 추출).
    pub fn data(&self, value: Value, human_label: &str) -> anyhow::Result<()> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            let data = value.get("data").cloned().unwrap_or(value);
            println!("{human_label}:");
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        Ok(())
    }

    /// 사람 친화적 한 줄 메시지. JSON 모드면 `{"status":"ok",...}` 형태로 출력.
    pub fn ok(&self, message: impl Into<String>) -> anyhow::Result<()> {
        let msg = message.into();
        if self.json {
            let v = serde_json::json!({ "status": "ok", "message": msg });
            println!("{}", serde_json::to_string_pretty(&v)?);
        } else {
            println!("{msg}");
        }
        Ok(())
    }
}
