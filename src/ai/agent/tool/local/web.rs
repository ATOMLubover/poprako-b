use std::env;

use reqwest::Client;
use serde::Deserialize;

use crate::ai::agent::tool::ITool;
use crate::ai::agent::tool::result::{ExecutionError, ExecutionResult};
use crate::ai::resolver::tool::{ParamDef, PropDef, ToolDef};

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
    fn defination(&self) -> ToolDef {
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

        ToolDef::new(
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

// ---- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_is_correct() {
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
