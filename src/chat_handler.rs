use async_recursion::async_recursion;
use futures::future::join_all;
use std::io::Write;
use std::process;
use std::process::Command;

use crate::{
    llm::{create_llm_provider, LLMConfig, LLMProvider, Message, Provider},
    prompts,
    tools::{execute_tool, ToolCall},
    user_system_info::UserSystemInfo,
};

pub struct ChatHandler {
    llm_provider: Provider,
    display_fn: Option<fn(&str) -> Result<(), Box<dyn std::error::Error>>>,
}

impl ChatHandler {
    pub fn new(llm_config: LLMConfig) -> Self {
        let user_system_info = UserSystemInfo::new();
        let mut vars = std::collections::HashMap::new();
        vars.insert("user_os".to_owned(), user_system_info.os.to_owned());
        vars.insert("user_arch".to_owned(), user_system_info.arch.to_owned());
        vars.insert("user_shell".to_owned(), user_system_info.shell.to_owned());

        let mut display_fn: Option<fn(&str) -> Result<(), Box<dyn std::error::Error>>> = None;
        if get_glow_installed() {
            display_fn = Some(display_with_glow_pipe);
        }

        let templates = prompts::get_template();
        let system_message = templates.render("SYSTEM_PROMPT", &vars).unwrap();

        let mut llm_provider = create_llm_provider(llm_config).unwrap();
        llm_provider.with_system_prompt(&system_message);

        Self {
            llm_provider: llm_provider,
            display_fn: display_fn,
        }
    }

    pub async fn process_user_prompt(&mut self, user_input: String) {
        let mut vars = std::collections::HashMap::new();
        vars.insert("user_input".to_owned(), user_input.to_owned());

        let templates = prompts::get_template();
        let user_input = templates.render("USER_PROMPT", &vars).unwrap();
        let message = Message {
            content: user_input,
            role: "user".to_string(),
            ..Default::default()
        };

        let response = &self.llm_provider.chat(&message, self.display_fn).await;

        let response = match response {
            Ok(val) => val,
            Err(e) => {
                eprintln!("Communication with LLM provider failed: {}", e);
                process::exit(1);
            }
        };

        if response.tool_calls.is_some() {
            let tool_calls = response.tool_calls.clone().unwrap();
            self.process_response_tool_calls(tool_calls).await;
        }
    }

    #[async_recursion(?Send)]
    async fn process_response_tool_calls(&mut self, tool_calls: Vec<ToolCall>) {
        if !tool_calls.is_empty() {
            // Execute each tool call
            let handles = tool_calls.into_iter().map(|tool_call| {
                tokio::spawn(async move { execute_tool(&tool_call.function).await.unwrap() })
            });

            let results = join_all(handles)
                .await
                .into_iter()
                .map(|r| r.unwrap())
                .collect::<Vec<_>>();

            let tool_result_message = Message {
                content: serde_json::to_string_pretty(&results).unwrap(),
                role: "tool".to_string(),
                ..Default::default()
            };

            let response = &self
                .llm_provider
                .chat(&tool_result_message, self.display_fn)
                .await
                .unwrap();
            let response_tool_calls = response.tool_calls.clone().unwrap();
            if !response_tool_calls.is_empty() {
                self.process_response_tool_calls(response_tool_calls).await;
            }
        }
    }
}

fn get_glow_installed() -> bool {
    // Use sh -c to run echo | glow
    let glow_version = Command::new("glow").arg("-v").output();
    glow_version.is_ok()
}

fn display_with_glow_pipe(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Use sh -c to run echo | glow
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("glow -s auto -w 100 -")
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(content.as_bytes())?;
    }

    child.wait()?;

    Ok(())
}
