use async_recursion::async_recursion;
use std::process;

use regex::Regex;

use crate::{
    llm::{create_llm_provider, LLMConfig, LLMProvider, Provider},
    prompts,
    tmux_command_executor::TmuxCommandExecutor,
    user_system_info::UserSystemInfo,
};

pub struct ChatHandler {
    pub user_system_info: UserSystemInfo,
    llm_config: LLMConfig,
}

impl ChatHandler {
    pub fn new(llm_config: LLMConfig) -> Self {
        Self {
            user_system_info: UserSystemInfo::new(),
            llm_config: llm_config,
        }
    }

    pub async fn process_user_prompt(&self, user_input: String) {
        let mut vars = std::collections::HashMap::new();
        vars.insert("user_input".to_owned(), user_input.to_owned());
        vars.insert("user_os".to_owned(), self.user_system_info.os.to_owned());
        vars.insert(
            "user_arch".to_owned(),
            self.user_system_info.arch.to_owned(),
        );
        vars.insert(
            "user_shell".to_owned(),
            self.user_system_info.shell.to_owned(),
        );

        let templates = prompts::get_template();
        let system_message = templates.render("SYSTEM_PROMPT", &vars).unwrap();
        let user_input = templates.render("USER_PROMPT", &vars).unwrap();

        let mut provider = create_llm_provider(self.llm_config.clone()).unwrap();
        provider.with_system_prompt(&system_message);

        let response = provider.chat(user_input).await;

        let response = match response {
            Ok(val) => val,
            Err(e) => {
                eprintln!("Communication with LLM provider failed: {}", e);
                process::exit(1);
            }
        };

        let tmux_executor = TmuxCommandExecutor::new();
        self.process_response(response, &tmux_executor, &mut provider, "".to_string())
            .await;

        tmux_executor.terminate_session();
    }

    #[async_recursion(?Send)]
    async fn process_response(
        &self,
        response: String,
        tmux_executor: &TmuxCommandExecutor,
        provider: &mut Provider,
        previous_command: String,
    ) {
        // Create executor for a specific tmux pane
        let command = extract_commands_to_run(&response)
            .first()
            .cloned()
            .unwrap_or_default();

        if !command.is_empty() {
            if command.to_string() != previous_command {
                println!("");
                println!("I'll run the following command:");
                println!("");
                println!("{}", create_box(&command));
                println!("");

                let command_output = tmux_executor.execute_command(&command).unwrap();

                let mut vars = std::collections::HashMap::new();
                vars.insert("terminal_text".to_owned(), command_output.to_owned());
                let templates = prompts::get_template();
                let user_input = templates.render("TERMINAL_OUTPUT_PROMPT", &vars).unwrap();

                let response = provider.chat(user_input).await;
                let response = response.unwrap();

                self.process_response(response, tmux_executor, provider, command.to_string())
                    .await;
            } else {
                println!("");
                println!("Previous command was the same!!");
                println!("");

                let response = provider.chat("That last command `{}` didn't work the time before. Please try another approach.".to_string()).await;
                let response = response.unwrap();

                self.process_response(response, tmux_executor, provider, command.to_string())
                    .await;
            }
        }
    }
}

fn extract_commands_to_run(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    // extract all commands enclosed in ``` ```
    let re = Regex::new(r#"```(.+?)```"#).unwrap();
    re.captures_iter(&text.replace('\n', ";")).for_each(|cap| {
        commands.push(
            cap[1]
                .to_string()
                .replace('\n', " ")
                .trim_start_matches(';')
                .trim_end_matches(';')
                .trim()
                .to_owned(),
        );
    });
    // if command start from bash; or sh; remove it
    commands = commands
        .iter()
        .map(|command| {
            if command.starts_with("bash;") {
                command.trim_start_matches("bash;").trim().to_owned()
            } else if command.starts_with("zsh;") {
                command.trim_start_matches("zsh;").trim().to_owned()
            } else if command.starts_with("sh;") {
                command.trim_start_matches("sh;").trim().to_owned()
            } else {
                command.to_owned()
            }
        })
        .collect();
    // deduplicate with keeping the order
    // count the number of occurrence of each command
    let mut counts = std::collections::HashMap::new();
    for command in &commands {
        let count = counts.entry(command).or_insert(0);
        *count += 1;
    }
    // add only the first occurrence of each command to deduped_commands
    // TODO: not elegant
    let mut deduped_commands: Vec<String> = Vec::new();
    for command in &commands {
        if deduped_commands.contains(command) {
        } else {
            deduped_commands.push(command.to_string());
        }
    }
    deduped_commands
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
