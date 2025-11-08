use crate::{
    tmux_command_executor::TmuxCommandExecutor,
    tools::{FunctionCall, FunctionDef, Tool, ToolCallResult},
};

pub struct ExecuteCommandToolBuilder;

impl ExecuteCommandToolBuilder {
    pub fn create_tool() -> Tool {
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "execute_command".to_string(),
                description: "Execute a shell command based on user intent".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        },
                        "description": {
                            "type": "string",
                            "description": "A brief explanation of what the command does"
                        }
                    },
                    "required": ["command", "description"]
                }),
            },
        }
    }
}

pub struct ExecuteCommandTool;
impl ExecuteCommandTool {
    pub fn call_tool_function(function_call: &FunctionCall) -> ToolCallResult {
        let command = function_call.arguments["command"].as_str().unwrap_or("");

        println!("I'll execute the following command:");
        println!();

        let boxed_command = create_box(command);
        println!("{}", &boxed_command);
        println!();

        let tmux_executor = TmuxCommandExecutor::new();
        let command_output = tmux_executor
            .execute_command(command)
            .unwrap_or_else(|_| "Error".to_string());

        tmux_executor.terminate_session();

        ToolCallResult {
            function_call: function_call.clone(),
            content: command_output,
        }
    }
}

fn create_box(text: &str) -> String {
    let padding = 5; // For "│ ✓  " prefix
    let max_width = text.len() + padding + 3;

    let top_line = format!("╭{}╮", "─".repeat(max_width));
    let bottom_line = format!("╰{}╯", "─".repeat(max_width));

    format!(
        "{}\n│ ✓  {:<width$} │\n{}",
        top_line,
        text,
        bottom_line,
        width = max_width - padding
    )
}
