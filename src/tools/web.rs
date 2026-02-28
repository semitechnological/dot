use anyhow::{Context, Result};
use serde_json::Value;

use super::Tool;

const MAX_CONTENT: usize = 50_000;

pub struct WebFetchTool;

impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "webfetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL and return it as text. Automatically strips HTML tags for web pages."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from"
                }
            },
            "required": ["url"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let url = input["url"]
            .as_str()
            .context("Missing required parameter 'url'")?;
        tracing::debug!("webfetch: {}", url);

        let response =
            reqwest::blocking::get(url).with_context(|| format!("failed to fetch: {}", url))?;

        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("HTTP {}: {}", status.as_u16(), url);
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = response
            .text()
            .with_context(|| format!("failed to read response from: {}", url))?;

        let text = if content_type.contains("text/html") {
            strip_html(&body)
        } else {
            body
        };

        if text.len() > MAX_CONTENT {
            Ok(format!(
                "{}\n... (truncated at {} chars)",
                &text[..MAX_CONTENT],
                MAX_CONTENT
            ))
        } else {
            Ok(text)
        }
    }
}

fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len() / 3);
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut last_was_space = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if !in_tag && chars[i] == '<' {
            let remaining: String = lower_chars[i..].iter().take(10).collect();
            if remaining.starts_with("<script") {
                in_script = true;
            } else if remaining.starts_with("<style") {
                in_style = true;
            }
            if remaining.starts_with("</script") {
                in_script = false;
            } else if remaining.starts_with("</style") {
                in_style = false;
            }

            let tag: String = lower_chars[i..].iter().take(5).collect();
            if (tag.starts_with("<br")
                || tag.starts_with("<p")
                || tag.starts_with("<div")
                || tag.starts_with("<h")
                || tag.starts_with("<li")
                || tag.starts_with("<tr"))
                && !result.ends_with('\n')
            {
                result.push('\n');
            }

            in_tag = true;
            i += 1;
            continue;
        }

        if in_tag {
            if chars[i] == '>' {
                in_tag = false;
            }
            i += 1;
            continue;
        }

        if in_script || in_style {
            i += 1;
            continue;
        }

        if chars[i] == '&'
            && let Some(semi) = html[i..].find(';')
        {
            let entity = &html[i..i + semi + 1];
            let decoded = match entity {
                "&amp;" => "&",
                "&lt;" => "<",
                "&gt;" => ">",
                "&quot;" => "\"",
                "&apos;" => "'",
                "&nbsp;" => " ",
                _ => " ",
            };
            result.push_str(decoded);
            last_was_space = decoded == " ";
            i += semi + 1;
            continue;
        }

        if chars[i].is_whitespace() {
            if !last_was_space && !result.is_empty() {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(chars[i]);
            last_was_space = false;
        }

        i += 1;
    }

    let mut cleaned = String::new();
    let mut consecutive = 0;
    for c in result.chars() {
        if c == '\n' {
            consecutive += 1;
            if consecutive <= 2 {
                cleaned.push(c);
            }
        } else {
            consecutive = 0;
            cleaned.push(c);
        }
    }

    cleaned.trim().to_string()
}
