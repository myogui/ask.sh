// Credits: nagarx/LLM-based-Search-Engine
// https://github.com/nagarx/LLM-based-Search-Engine/blob/main/src/search/searxng.rs

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

use crate::{
    tools::{FunctionCall, FunctionDef, Tool, ToolCallResult, ToolError},
    ENV_SEARXNG_BASE_URL,
};

pub struct WebSearchToolBuilder;

impl WebSearchToolBuilder {
    pub fn tool_available() -> bool {
        env::var(ENV_SEARXNG_BASE_URL).is_ok()
    }

    pub fn create_tool() -> Tool {
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "web_search".to_string(),
                description: "Search the web when the user asks for current information, web lookups, or information not available locally".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query to run on the search engine."
                        }
                    },
                    "required": ["query"]
                }),
            },
        }
    }
}

pub struct WebSearchTool;

impl WebSearchTool {
    pub async fn call_tool_function(function_call: &FunctionCall) -> ToolCallResult {
        let query = function_call.arguments["query"].as_str().unwrap();
        let searxng_client = SearxngClient::new(env::var(ENV_SEARXNG_BASE_URL).unwrap());
        let query_result = searxng_client.search(query).await;

        ToolCallResult {
            content: serde_json::to_value(&query_result.unwrap()).unwrap(),
            function_call: function_call.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
    pub img_src: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearxngResponse {
    #[allow(dead_code)]
    query: String,
    results: Vec<SearxngResult>,
}

#[derive(Debug, Deserialize)]
struct SearxngResult {
    title: String,
    url: String,
    content: String,
    #[serde(default)]
    img_src: Option<String>,
}

pub struct SearxngClient {
    base_url: String,
    client: Client,
}

impl SearxngClient {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self { base_url, client }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>, ToolError> {
        let mut params = HashMap::new();
        params.insert("q", query);
        params.insert("format", "json");
        params.insert("engines", "google,bing,duckduckgo");

        let url = format!("{}/search", self.base_url);

        println!("🔍 Searching with SearXNG: '{query}'");

        let response = self
            .client
            .get(&url)
            .query(&params)
            .header("User-Agent", "ash-sh-rust/1.0.0")
            .send()
            .await
            .map_err(|e| ToolError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ToolError::ApiError(format!(
                "SearXNG API error: {}: {}",
                status, error_text
            )));
        }

        let searxng_response: SearxngResponse = response.json().await.unwrap();

        let results: Vec<SearchResult> = searxng_response
            .results
            .into_iter()
            .take(5) // Limit to top 5 results
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                content: r.content,
                img_src: r.img_src,
            })
            .collect();

        println!("✅ Processing {} search results", results.len());
        println!();
        Ok(results)
    }
}
