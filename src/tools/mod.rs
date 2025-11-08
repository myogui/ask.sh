pub mod execute_command;
use serde::{Deserialize, Serialize};

use crate::tools::execute_command::{ExecuteCommandTool, ExecuteCommandToolBuilder};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tool {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionDef,
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
    content: String,
}

pub fn get_available_tools() -> Vec<Tool> {
    let available_tools = vec![ExecuteCommandToolBuilder::create_tool()];
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
        _ => Err(format!("Unknown function: {}", function_call.name).into()),
    }
}
