pub mod execute_command;
pub mod searxng_web_search;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::tools::execute_command::{ExecuteCommandTool, ExecuteCommandToolBuilder};
use crate::tools::searxng_web_search::{WebSearchTool, WebSearchToolBuilder};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tool {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionDef,
}
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("API error: {0}")]
    ApiError(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Serialize)]
pub struct ToolCallResult {
    function_call: FunctionCall,
    content: serde_json::Value,
}

pub fn get_available_tools() -> Vec<Tool> {
    let mut available_tools = vec![ExecuteCommandToolBuilder::create_tool()];

    if WebSearchToolBuilder::tool_available() {
        available_tools.push(WebSearchToolBuilder::create_tool());
    }

    available_tools
}

pub async fn execute_tool(
    function_call: &FunctionCall,
) -> Result<ToolCallResult, Box<dyn std::error::Error>> {
    match function_call.name.as_str() {
        "execute_command" => {
            let result = ExecuteCommandTool::call_tool_function(function_call);
            Ok(result)
        }
        "web_search" => {
            let result = WebSearchTool::call_tool_function(function_call).await;
            Ok(result)
        }
        _ => Err(format!("Unknown function: {}", function_call.name).into()),
    }
}
