//! The public use-webmcp-tool API registers actual native browser tools.
use js::{Error, JsObject, Kind, Promise, Unknown, classify, json};
use serde::{Serialize, de::DeserializeOwned};
use webapi::AbortSignal;

pub struct ToolOptions<F> {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: &'static JsObject,
    pub annotations: Annotations,
    pub enabled: bool,
    pub execute: F,
}
pub struct Annotations {
    pub read_only_hint: bool,
    pub untrusted_content_hint: bool,
}
pub struct ExecutionOptions {
    pub signal: &'static AbortSignal,
}
pub struct ToolState {
    pub supported: bool,
    pub registered: bool,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub error: Option<&'static Error>,
}
pub struct ToolResponse {
    pub content: Vec<Content>,
    pub is_error: bool,
}
pub struct Content {
    pub r#type: &'static str,
    pub text: Option<String>,
    pub data: Option<String>,
    pub mime_type: Option<&'static str>,
}
impl ToolResponse {
    pub fn text(text: impl Into<String>, error: bool) -> Self {
        Self {
            content: vec![Content {
                r#type: "text",
                text: Some(text.into()),
                data: None,
                mime_type: None,
            }],
            is_error: error,
        }
    }
    pub fn json<T: Serialize>(value: &T) -> Self {
        Self::text(
            serde_json::to_string(value).expect("tool output is JSON"),
            false,
        )
    }
    pub fn result<T: Serialize>(value: Result<T, String>) -> Self {
        match value {
            Ok(value) => Self::json(&value),
            Err(error) => Self::text(error, true),
        }
    }
    pub fn image<T: Serialize>(info: &T, data_url: &str) -> Self {
        let data = data_url.split_once(',').map_or("", |(_, data)| data);
        Self {
            content: vec![
                Content {
                    r#type: "text",
                    text: Some(serde_json::to_string(info).expect("capture metadata is JSON")),
                    data: None,
                    mime_type: None,
                },
                Content {
                    r#type: "image",
                    text: None,
                    data: Some(data.into()),
                    mime_type: Some("image/png"),
                },
            ],
            is_error: false,
        }
    }
}
#[cfg_attr(rust_js, rust_js::link_name = "use-webmcp-tool#useWebMCP")]
pub fn use_web_mcp<F>(options: ToolOptions<F>) -> ToolState
where
    F: Fn(Option<&Unknown>, ExecutionOptions) -> Promise<ToolResponse> + 'static,
{
    unreachable!()
}
pub fn input<T: DeserializeOwned>(value: Option<&Unknown>) -> Result<T, String> {
    let text = match value {
        Some(value) => json::stringify(value)
            .map_err(|_| "Tool input is not JSON.".to_string())?
            .ok_or("Tool input is missing.")?,
        None => "{}".into(),
    };
    serde_json::from_str(&text).map_err(|e| format!("Invalid tool input: {e}"))
}
pub fn schema(name: &str) -> &'static JsObject {
    let all: serde_json::Value = serde_json::from_str(include_str!("tool-schemas.json"))
        .expect("tool schemas are valid JSON");
    let text = all[name].to_string();
    let value = json::parse(&text)
        .expect("schema is JSON")
        .expect("schema is an object");
    match classify(Some(value)) {
        Kind::Object(object) => object,
        _ => panic!("schema must be an object"),
    }
}
