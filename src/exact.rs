use anyhow::{Context, Result};

/// Counting model: counts are model-specific; current Opus is the default
/// target people paste into.
const MODEL: &str = "claude-opus-4-8";
const ENDPOINT: &str = "https://api.anthropic.com/v1/messages/count_tokens";

/// Exact Claude token count for `text` via Anthropic's count-tokens
/// endpoint (free to call). Requires ANTHROPIC_API_KEY in the environment.
pub fn count(text: &str) -> Result<usize> {
    let key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is not set")?;

    let mut response = ureq::post(ENDPOINT)
        .header("x-api-key", &key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .send(request_body(text, MODEL).to_string())
        .context("calling the count-tokens API")?;

    let raw = response
        .body_mut()
        .read_to_string()
        .context("reading count-tokens response")?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).context("parsing count-tokens response")?;
    json["input_tokens"]
        .as_u64()
        .map(|n| n as usize)
        .context("count-tokens response missing input_tokens")
}

/// The request body, separated for offline testing.
fn request_body(text: &str, model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": text}],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_shape() {
        let body = request_body("hello", "claude-opus-4-8");
        assert_eq!(body["model"], "claude-opus-4-8");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hello");
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    }

    /// Live API test — run explicitly with:
    ///   ANTHROPIC_API_KEY=... cargo test exact_count_live -- --ignored
    #[test]
    #[ignore = "requires ANTHROPIC_API_KEY and network"]
    fn exact_count_live() {
        let n = count("hello world").unwrap();
        assert!(n > 0 && n < 50, "unexpected count: {n}");
    }
}
