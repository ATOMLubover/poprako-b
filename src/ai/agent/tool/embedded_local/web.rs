use std::env;

use reqwest::Client;
use serde::Deserialize;

use crate::ai::agent::tool::ITool;
use crate::ai::agent::tool::result::{ExecutionError, ExecutionResult};
use crate::ai::resolver::tool::{ParamDef, PropDef, ToolDefination};

// ---- Tavily API types ------------------------------------------------------

const TAVILY_URL: &str = "https://api.tavily.com/search";
const MAX_CONTENT_CHARS: usize = 300;

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

// ---- WebSearchTool ---------------------------------------------------------

pub struct WebSearchTool {
    api_key: String,
    client: Client,
}

impl WebSearchTool {
    pub fn from_env() -> Option<Self> {
        let api_key = env::var("TAVILY_API_KEY").ok()?;
        Some(Self {
            api_key,
            client: Client::new(),
        })
    }
}

#[async_trait::async_trait]
impl ITool for WebSearchTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "query",
                    PropDef::String {
                        desc: "Search query. Be specific — use keywords, exact phrases in quotes, \
                               and include the current year for recent information."
                            .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "max_results",
                    PropDef::Number {
                        desc: "Maximum number of results to return (1–10, default 5).".to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec!["query".to_string()]);

        ToolDefination::new(
            "web_search",
            "Search the web using Tavily. Returns titles, URLs, and content snippets. \
             Use this to access information beyond your knowledge cutoff or find \
             current/recent data. Searches are performed in real-time.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let (query, max_results) = Self::parse_args(args)?;
        let results = self.call_tavily(&query, max_results).await?;
        Ok(format_results(&results))
    }
}

// ---- parsing ---------------------------------------------------------------

impl WebSearchTool {
    fn parse_args(args: &str) -> Result<(String, u64), ExecutionError> {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ExecutionError::args_schema(format!("Invalid JSON args: {}", e)))?;

        let query = v
            .get("query")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ExecutionError::args_schema("Missing required field 'query'".into()))?
            .to_string();

        let max_results = v
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .clamp(1, 10);

        Ok((query, max_results))
    }
}

// ---- API call --------------------------------------------------------------

impl WebSearchTool {
    async fn call_tavily(
        &self,
        query: &str,
        max_results: u64,
    ) -> Result<Vec<TavilyResult>, ExecutionError> {
        let body = serde_json::json!({
            "api_key": self.api_key,
            "query": query,
            "search_depth": "basic",
            "max_results": max_results,
        });

        let response = self
            .client
            .post(TAVILY_URL)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ExecutionError::exec_fail(format!("Search request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ExecutionError::exec_fail(format!(
                "Search API returned {status}: {text}"
            )));
        }

        let data: TavilyResponse = response.json().await.map_err(|e| {
            ExecutionError::exec_fail(format!("Failed to parse search response: {}", e))
        })?;

        Ok(data.results)
    }
}

// ---- formatting ------------------------------------------------------------

struct FormattedResult {
    index: usize,
    title: String,
    url: String,
    snippet: String,
}

fn truncate_content(content: &str) -> String {
    if content.chars().count() > MAX_CONTENT_CHARS {
        let truncated: String = content.chars().take(MAX_CONTENT_CHARS).collect();
        format!("{}…", truncated)
    } else {
        content.to_string()
    }
}

fn format_results(results: &[TavilyResult]) -> String {
    if results.is_empty() {
        return "No search results found.".to_string();
    }

    let formatted: Vec<FormattedResult> = results
        .iter()
        .enumerate()
        .map(|(i, r)| FormattedResult {
            index: i + 1,
            title: r.title.clone(),
            url: r.url.clone(),
            snippet: truncate_content(&r.content),
        })
        .collect();

    let lines: Vec<String> = formatted
        .iter()
        .map(|r| {
            let header = format!("{}. **{}**", r.index, r.title);
            let url_line = format!("   URL: {}", r.url);
            let body = format!("   {}", r.snippet);
            format!("{}\n{}\n{}\n", header, url_line, body)
        })
        .collect();

    lines.join("\n")
}

// ---- WebFetchTool ---------------------------------------------------------

const DEFAULT_MAX_BYTES: usize = 64 * 1024; // 64 KB
const MAX_REDIRECTS: usize = 5;

pub struct WebFetchTool {
    client: Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("PopRaKo/1.0 (research bot; +https://github.com/example)")
                .build()
                .expect("Failed to build reqwest Client"),
        }
    }

    fn parse_args(args: &str) -> Result<(String, usize), ExecutionError> {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ExecutionError::args_schema(format!("Invalid JSON args: {e}")))?;

        let url = v
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ExecutionError::args_schema("Missing required field 'url'".into()))?
            .to_string();

        // Validate URL structure.
        if url::Url::parse(&url).is_err() {
            return Err(ExecutionError::args_schema(format!(
                "Invalid URL format: {url}"
            )));
        }

        let max_bytes = v
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .map(|n| (n as usize).max(1_000))
            .unwrap_or(DEFAULT_MAX_BYTES);

        Ok((url, max_bytes))
    }

    async fn fetch_url(&self, url: &str, max_bytes: usize) -> Result<String, ExecutionError> {
        let response = self.client.get(url).send().await.map_err(|e| {
            if e.is_timeout() {
                ExecutionError::exec_fail(format!("Request timed out: {url}"))
            } else if e.is_redirect() {
                ExecutionError::exec_fail(format!("Too many redirects fetching {url}"))
            } else if e.is_connect() {
                ExecutionError::exec_fail(format!("Connection failed: {url} — {e}"))
            } else {
                ExecutionError::exec_fail(format!("Request failed: {url} — {e}"))
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let snippet = body.chars().take(200).collect::<String>();
            return Err(ExecutionError::exec_fail(format!(
                "HTTP {status} for {url}: {snippet}"
            )));
        }

        let body = response.text().await.map_err(|e| {
            ExecutionError::exec_fail(format!("Failed to read response body from {url}: {e}"))
        })?;

        let truncated: String = body.chars().take(max_bytes).collect();
        let info = if body.len() > max_bytes {
            format!(
                "\n\n[Response truncated to {max_bytes} bytes; total was {} bytes]",
                body.len()
            )
        } else {
            String::new()
        };

        Ok(format!("{truncated}{info}"))
    }
}

#[async_trait::async_trait]
impl ITool for WebFetchTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "url",
                    PropDef::String {
                        desc: "The URL to fetch. Must be a valid HTTP or HTTPS URL. Use this to \
                               retrieve a web page, API response, or any URL-addressable content."
                            .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "max_bytes",
                    PropDef::Number {
                        desc: "Maximum bytes of the response to return (default 10,000, \
                               minimum 1,000). Use a larger value (e.g., 50,000) when the \
                               page is expected to be long."
                            .to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec!["url".to_string()]);

        ToolDefination::new(
            "web_fetch",
            "Fetch the content of a web page or URL. Returns the raw text response (HTML or \
             plain text). Use this to access online resources, read web pages, call HTTP APIs, \
             or retrieve any internet content in real time.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let (url, max_bytes) = Self::parse_args(args)?;
        self.fetch_url(&url, max_bytes).await
    }
}

// ---- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- WebFetchTool tests ----

    #[test]
    fn web_fetch_tool_definition_is_correct() {
        let tool = WebFetchTool::new();
        let def = tool.defination();

        assert_eq!(def.name, "web_fetch");
        assert_eq!(def.strict, Some(true));
        assert!(def.parameters.props.contains_key("url"));
        assert!(def.parameters.props.contains_key("max_bytes"));
        assert_eq!(def.parameters.required, Some(vec!["url".to_string()]));
    }

    #[tokio::test]
    async fn web_fetch_reject_missing_url() {
        let mut tool = WebFetchTool::new();
        let result = tool.execute(r#"{}"#).await;
        assert!(result.is_err(), "missing url should be rejected");
    }

    #[tokio::test]
    async fn web_fetch_reject_empty_url() {
        let mut tool = WebFetchTool::new();
        let result = tool.execute(r#"{"url":""}"#).await;
        assert!(result.is_err(), "empty url should be rejected");
    }

    #[tokio::test]
    async fn web_fetch_reject_invalid_url() {
        let mut tool = WebFetchTool::new();
        let result = tool.execute(r#"{"url":"not-a-valid-url"}"#).await;
        assert!(result.is_err(), "invalid url should be rejected");
    }

    #[tokio::test]
    async fn web_fetch_returns_html() {
        let mut tool = WebFetchTool::new();
        // Use a well-known public URL with deterministic content.
        let result = tool
            .execute(r#"{"url":"https://example.com","max_bytes":2000}"#)
            .await;

        assert!(result.is_ok(), "fetch should succeed: {:?}", result);
        let output = result.unwrap();
        assert!(!output.is_empty(), "output should not be empty");
        assert!(
            output.to_lowercase().contains("html"),
            "output should contain HTML content, got: {output}"
        );
    }

    // NOTE: example.com's response body (~500 bytes) is now smaller than the
    // 1000-byte threshold used in this test, so the truncation note never
    // appears. Re-enable when a test endpoint with a reliably large response is
    // available (or use a mock).
    // #[tokio::test]
    #[allow(dead_code)]
    async fn web_fetch_max_bytes_truncates() {
        let mut tool = WebFetchTool::new();
        // Fetch with a very small max_bytes to force truncation.
        let result = tool
            .execute(r#"{"url":"https://example.com","max_bytes":1000}"#)
            .await;

        assert!(result.is_ok(), "fetch should succeed: {:?}", result);
        let output = result.unwrap();
        assert!(
            output.contains("[Response truncated to 1000 bytes"),
            "truncated response should have a note, got: {output}"
        );
    }

    #[tokio::test]
    async fn web_fetch_min_max_bytes_clamped() {
        // max_bytes < 1000 should be clamped to 1000 at parse time.
        let (_url, max_bytes) =
            WebFetchTool::parse_args(r#"{"url":"https://example.com","max_bytes":50}"#).unwrap();
        assert_eq!(
            max_bytes, 1_000,
            "max_bytes should be clamped to minimum 1000"
        );
    }

    #[tokio::test]
    async fn web_fetch_non_existent_domain() {
        let mut tool = WebFetchTool::new();
        let result = tool
            .execute(r#"{"url":"https://this-domain-does-not-exist-123456789.com/"}"#)
            .await;

        assert!(
            result.is_err(),
            "non-existent domain should fail: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn web_fetch_404() {
        let mut tool = WebFetchTool::new();
        let result = tool
            .execute(r#"{"url":"https://example.com/nonexistent-page-12345"}"#)
            .await;

        assert!(result.is_err(), "404 should fail: {:?}", result);
    }

    #[tokio::test]
    async fn web_fetch_default_max_bytes() {
        // When max_bytes is omitted, parse_args should use DEFAULT_MAX_BYTES.
        let (_url, max_bytes) =
            WebFetchTool::parse_args(r#"{"url":"https://example.com"}"#).unwrap();
        assert_eq!(max_bytes, DEFAULT_MAX_BYTES);
    }

    // ---- WebSearchTool tests ----

    #[test]
    fn web_search_tool_definition_is_correct() {
        let tool = WebSearchTool {
            api_key: "test-key".into(),
            client: Client::new(),
        };
        let def = tool.defination();

        assert_eq!(def.name, "web_search");
        assert_eq!(def.strict, Some(true));
        assert!(def.parameters.props.contains_key("query"));
        assert!(def.parameters.props.contains_key("max_results"));
        assert_eq!(def.parameters.required, Some(vec!["query".to_string()]));
    }

    #[tokio::test]
    async fn reject_missing_query() {
        let mut tool = WebSearchTool {
            api_key: "test-key".into(),
            client: Client::new(),
        };

        let result = tool.execute(r#"{"max_results":3}"#).await;
        assert!(result.is_err(), "missing query should be rejected");
    }

    #[tokio::test]
    async fn search_returns_results() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let mut tool = match WebSearchTool::from_env() {
            Some(t) => t,
            None => {
                eprintln!("TAVILY_API_KEY not set, skipping integration test");
                return;
            }
        };

        let result = tool
            .execute(r#"{"query":"Rust programming language latest version 2025","max_results":3}"#)
            .await;

        assert!(result.is_ok(), "search should succeed: {:?}", result);
        let output = result.unwrap();
        assert!(!output.is_empty(), "output should not be empty");
        assert!(
            output.contains("**"),
            "output should contain markdown bold titles"
        );
        assert!(output.contains("URL:"), "output should contain URLs");
    }

    #[tokio::test]
    async fn search_no_results() {
        dotenvy::dotenv().ok();

        let mut tool = match WebSearchTool::from_env() {
            Some(t) => t,
            None => {
                eprintln!("TAVILY_API_KEY not set, skipping integration test");
                return;
            }
        };

        let result = tool
            .execute(r#"{"query":"xyzlmnopqrstuvwxyz1234567890abcdefghijklmnop","max_results":1}"#)
            .await;

        assert!(result.is_ok(), "even empty results should be Ok");
    }

    #[tokio::test]
    async fn max_results_clamped() {
        dotenvy::dotenv().ok();

        let mut tool = match WebSearchTool::from_env() {
            Some(t) => t,
            None => {
                eprintln!("TAVILY_API_KEY not set, skipping integration test");
                return;
            }
        };

        let result = tool
            .execute(r#"{"query":"hello world","max_results":100}"#)
            .await;

        assert!(result.is_ok(), "clamped request should succeed");
    }

    #[tokio::test]
    async fn search_with_empty_query_rejected() {
        let mut tool = WebSearchTool {
            api_key: "test-key".into(),
            client: Client::new(),
        };

        let result = tool.execute(r#"{"query":""}"#).await;
        assert!(result.is_err(), "empty query should be rejected");
    }
}

#[cfg(test)]
mod web_fetch_integration_tests {
    use super::*;

    // NOTE: httpbin.org frequently returns 503, causing flaky CI failures.
    // Re-enable when a more reliable test endpoint is available.
    // /// Run with: cargo test web_fetch::web_fetch_integration_tests -- --nocapture
    // ///
    // /// Requires network access. Fetches a real page and verifies the content.
    // #[tokio::test]
    #[allow(dead_code)]
    async fn fetch_httpbin_ip() {
        let mut tool = WebFetchTool::new();
        let result = tool
            .execute(r#"{"url":"https://httpbin.org/get","max_bytes":3000}"#)
            .await;

        assert!(result.is_ok(), "httpbin should be reachable: {:?}", result);
        let output = result.unwrap();
        assert!(
            output.contains("\"url\""),
            "response should contain 'url' field"
        );
    }
}
