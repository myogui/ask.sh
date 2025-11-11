use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use unicode_width::UnicodeWidthStr;

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
                description: "Execute a shell command when the user asks to run terminal commands, check system status, or perform local operations".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        }
                    },
                    "required": ["command"]
                }),
            },
        }
    }
}

pub struct ExecuteCommandTool;
impl ExecuteCommandTool {
    pub fn call_tool_function(function_call: &FunctionCall) -> ToolCallResult {
        let command = function_call.arguments["command"].as_str().unwrap_or("");

        let spinner = display_command_with_spinner_status(command);

        let tmux_executor = TmuxCommandExecutor::new();
        let command_result = tmux_executor.execute_command(command);

        let command_output: String;

        match command_result {
            Ok(output) => {
                update_spinner_status(&spinner, command, true);
                command_output = output;
            }
            Err(error_output) => {
                update_spinner_status(&spinner, command, false);
                command_output = error_output.to_string()
            }
        }

        // println!("{}", command_output);
        println!();
        println!();

        tmux_executor.terminate_session();

        ToolCallResult {
            function_call: function_call.clone(),
            content: serde_json::Value::String(command_output),
        }
    }
}

fn display_command_with_spinner_status(command: &str) -> ProgressBar {
    let template = create_progress_bar_template(command);
    let ticks: Vec<String> = vec!["◐", "◓", "◑", "◒"]
        .into_iter()
        .map(|s| style(s).yellow().to_string())
        .collect();

    // Then, you need a Vec<&str> from Vec<String>:
    let ticks_ref: Vec<&str> = ticks.iter().map(|s| s.as_str()).collect();
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.set_style(
        ProgressStyle::with_template(&template)
            .unwrap()
            .tick_strings(&ticks_ref),
    );
    progress_bar.set_message(command.to_string());
    progress_bar.enable_steady_tick(std::time::Duration::from_millis(150));

    progress_bar
}

fn update_spinner_status(progress_bar: &ProgressBar, command: &str, command_successful: bool) {
    let template = create_progress_bar_template(command);

    // Update with final status
    if command_successful {
        progress_bar.set_style(
            ProgressStyle::with_template(&template)
                .unwrap()
                .tick_strings(&[style("✓").green().to_string().as_ref()]),
        );
    } else {
        progress_bar.set_style(
            ProgressStyle::with_template(&template)
                .unwrap()
                .tick_strings(&[style("✗").red().to_string().as_ref()]),
        );
    }
    progress_bar.finish_with_message(command.to_string());
}

fn create_progress_bar_template(command: &str) -> String {
    let padding = 1;
    let unstyled_content = command;
    let content_width = UnicodeWidthStr::width(unstyled_content);
    let box_width = content_width + 2 + 2 * padding;

    let template = format!(
        "{top_bar}\n│{left_pad}{{spinner}} {{msg}}{right_pad}│\n{bottom_bar}\n",
        top_bar = format!("╭{}╮", "─".repeat(box_width)),
        left_pad = " ".repeat(padding),
        right_pad = " ".repeat(padding),
        bottom_bar = format!("╰{}╯", "─".repeat(box_width)),
    );

    template
}
