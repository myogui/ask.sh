use dotenv::dotenv;
use futures::stream::StreamExt;
use regex::Regex;
use std::{
    env::{
        self,
        consts::{ARCH, OS},
    },
    error::Error,
    io::{self, BufRead},
    process::{self, Command},
};

mod llm;
mod prompts;
mod tmux_command_executor;

use llm::{create_provider, LLMConfig, LLMError, LLMProvider};
use tmux_command_executor::TmuxCommandExecutor;

// args
const ARG_DEBUG: &str = "--debug_ask_sh";
const ARG_VERSION: &str = "--version";
const ARG_VERSION_SHORT: &str = "-v";

const ARG_STRINGS: &[&str] = &[ARG_DEBUG, ARG_VERSION, ARG_VERSION_SHORT];

// special arg
const ARG_INIT: &str = "--init";

// env
const ENV_DEBUG: &str = "ASK_SH_DEBUG";

// LLM provider settings
const ENV_LLM_PROVIDER: &str = "ASK_SH_LLM_PROVIDER";
const ENV_OPENAI_API_KEY: &str = "ASK_SH_OPENAI_API_KEY";
const ENV_OPENAI_MODEL: &str = "ASK_SH_OPENAI_MODEL";
const ENV_OPENAI_BASE_URL: &str = "ASK_SH_OPENAI_BASE_URL";
const ENV_ANTHROPIC_API_KEY: &str = "ASK_SH_ANTHROPIC_API_KEY";
const ENV_ANTHROPIC_MODEL: &str = "ASK_SH_ANTHROPIC_MODEL";
const ENV_OLLAMA_BASE_URL: &str = "ASK_SH_OLLAMA_BASE_URL";
const ENV_OLLAMA_MODEL: &str = "ASK_SH_OLLAMA_MODEL";
const ENV_OLLAMA_KEEP_ALIVE: &str = "ASK_SH_OLLAMA_KEEP_ALIVE";

fn get_llm_config() -> Result<LLMConfig, LLMError> {
    // Select provider (default is OpenAI)
    let provider = env::var(ENV_LLM_PROVIDER).unwrap_or_else(|_| "openai".to_string());

    match provider.as_str() {
        "openai" => {
            let api_key = env::var(ENV_OPENAI_API_KEY)
                .map_err(|_| LLMError::ConfigError("OpenAI API key not found".to_string()))?;

            let model = env::var(ENV_OPENAI_MODEL).unwrap_or_else(|_| "gpt-3.5-turbo".to_string());

            let base_url = env::var(ENV_OPENAI_BASE_URL).ok();

            Ok(LLMConfig {
                provider,
                api_key,
                model,
                base_url,
                keep_alive: None,
            })
        }
        "anthropic" => {
            let api_key = env::var(ENV_ANTHROPIC_API_KEY)
                .map_err(|_| LLMError::ConfigError("Anthropic API key not found".to_string()))?;

            let model = env::var(ENV_ANTHROPIC_MODEL)
                .unwrap_or_else(|_| "claude-3-5-sonnet-latest".to_string());

            Ok(LLMConfig {
                provider,
                api_key,
                model,
                base_url: None, // Anthropic does not support custom endpoints
                keep_alive: None,
            })
        }
        "ollama" => {
            let api_key = "ollama dummy key".to_string();

            let model = env::var(ENV_OLLAMA_MODEL).unwrap_or_else(|_| "gemma3:4b".to_string());

            let base_url = env::var(ENV_OLLAMA_BASE_URL).ok();

            let keep_alive: Option<i64> = env::var(ENV_OLLAMA_KEEP_ALIVE)
                .ok()
                .and_then(|s| s.parse().ok());

            Ok(LLMConfig {
                provider,
                api_key,
                model,
                base_url,
                keep_alive,
            })
        }
        _ => Err(LLMError::ConfigError(format!(
            "Unknown provider: {}",
            provider
        ))),
    }
}

fn get_env_flag(key: &str) -> bool {
    dotenv().ok();
    match env::var(key) {
        Ok(val) => val.parse::<bool>().unwrap_or(false),
        Err(_e) => false,
    }
}

struct UserInfo {
    arch: String,
    os: String,
    shell: String,
    // TODO: add distro info if linux
}

/// Chat with LLM provider
#[tokio::main]
async fn chat(
    user_input: String,
    system_message: String,
    _debug_mode: &bool, // currently unused
) -> Result<String, Box<dyn Error>> {
    let config = get_llm_config().map_err(|e| Box::new(e) as Box<dyn Error>)?;
    let mut provider = create_provider(config).map_err(|e| Box::new(e) as Box<dyn Error>)?;

    provider.with_system_prompt(&system_message);

    let mut stream = provider
        .chat_stream(user_input)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error>)?;

    let mut response_to_return = String::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(content) => {
                response_to_return.push_str(&content);
                eprint!("{}", content);
            }
            Err(err) => {
                eprint!("{}", err);
            }
        }
    }
    Ok(response_to_return)
}

fn get_commands_to_run(text: &str) -> Vec<String> {
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

fn print_init_script() {
    print!(
        r#"# This function is automatically generated by ask-sh --init
# ask.sh shell function v2
ask() {{
    if ! command -v ask-sh &> /dev/null; then
        printf "❌ Necessary rust package ask-sh is installed but cannot be accessed. Rust's bin path may not be added to your PATH."
        printf "👉 It's usually under ~/.cargo/bin/"
        printf "👀 Please add it to your PATH and restart your shell."
    fi
    suggested_commands=`echo "$@" | ask-sh 2> >(cat 1>&2)`
    if [ -n "$suggested_commands" ]; then
        printf "\n" # add one empty line to create space
        printf "👋 Hey, AI has suggested some commands that can be typed into your terminal.\n"
        printf "🔍 Press Enter to view and select the commands, or type any other key to exit:"
        if [ -n "$ZSH_VERSION" ]; then # read a single char
            read -r -k 1 REPLY # zsh
        else
            read -r -n 1 REPLY # bash
        fi
        REPLY="${{REPLY#"${{REPLY%%[![:space:]]*}}"}}"  # trim whitespaces
        if [ -z "$REPLY" ] ; then
            # As Enter will move cursor to the next line, we need to go back two lines
            printf "\033[2A"
            # \033[2K: delete current line (👋 line), \n: go next line, \033[2K: delete next line (🔍 line)
            printf "\033[2K\n\033[2K\n"
            # We're at the emptified 🔍 line. So, go back two lines, including empty line to make space
            printf "\033[2A" # go back again
            selected_command=`echo "$suggested_commands" | peco  --prompt "AI suggested commands (Enter to use / Ctrl+C to exit):"`
            if [ -n "$selected_command" ]; then
                if ! print -z $selected_command 2>/dev/null; then
                    history -s $selected_command
                fi
            fi
        else
            # We're at the end of 🔍 line. So, go back one line (👋 line)
            printf "\033[1A"
            printf "\033[2K\n\033[2K\n"
            printf "\033[2A"
        fi
    fi
    if [ -z "$ASK_SH_NO_UPDATE" ]; then
        latest_version=`cargo search ask-sh | grep ask-sh | awk '{{print $3}}' | cut -d '"' -f2`
        current_version=`ask-sh --version`
        if [ "$(printf '%s\n' "$latest_version" "$current_version" | sort -rV | head -n1)" = "$latest_version" ] && [ "$latest_version" != "$current_version" ]; then
            # clear line
            printf "\n"
            printf "🎉 New version of ask-sh is available! (Current: $current_version vs New: $latest_version) Set \$ASK_SH_NO_UPDATE=1 to suppress this notice.\n"
            printf "🆙 Press Enter to run update now, or type any other key to exit:"
            if [ -n "$ZSH_VERSION" ]; then # read a single char
                read -r -k 1 REPLY # zsh
            else
                read -r -n 1 REPLY # bash
            fi
            REPLY="${{REPLY#"${{REPLY%%[![:space:]]*}}"}}"  # trim whitespaces
            if [ -z "$REPLY" ] ; then
                cargo install --force ask-sh
                printf "\nDone! Please restart your shell or source ~/.zshrc or ~/.bashrc etc... to use the new version.\n"
            else
                printf "\nOk, you can update ask-sh later by running 'cargo install --force ask-sh'.\n"
            fi
        fi
    fi
}}
"#
    );
}

fn create_box(text: &str, stats: &str) -> String {
    let padding = 5; // For "│ ✓  " prefix
    let max_width = text.len().max(stats.len()) + padding + 3;

    let top_line = format!("╭{}╮", "─".repeat(max_width));
    let bottom_line = format!("╰{}╯", "─".repeat(max_width));

    format!(
        "{}\n│ ✓  {:<width$} │\n│    {:<width$} │\n{}",
        top_line,
        text,
        stats,
        bottom_line,
        width = max_width - padding
    )
}

fn main() {
    dotenv().ok();

    // if called with only --init, the command emits a shell script to be sourced
    if env::args().len() == 2 && env::args().nth(1).unwrap() == ARG_INIT {
        print_init_script();
        return;
    }

    // if called with only --version or -v, print version and exit
    if env::args().len() == 2 {
        let arg = env::args().nth(1).unwrap();
        if arg == ARG_VERSION || arg == ARG_VERSION_SHORT {
            println!("{}", env!("CARGO_PKG_VERSION"));
            return;
        }
    }

    // check input from users
    // arg without the first executable name
    let args: Vec<String> = env::args().skip(1).collect();
    // check if args are all predefined args
    let is_using_stdin = args.iter().all(|arg| ARG_STRINGS.contains(&arg.as_str()));

    let user_input = if is_using_stdin {
        io::stdin().lock().lines().next().unwrap().unwrap()
    } else {
        args.join(" ")
    };

    // filter out predefined args
    let user_input_without_flags = user_input
        .split_whitespace()
        .filter(|arg| !ARG_STRINGS.contains(arg))
        .collect::<Vec<&str>>()
        .join(" ");

    // debug_mode is true if args contains --debug_ASK_SH or stdin text contains "--debug_ASK_SH" or env var ASK_SH_DEBUG is defined
    let debug_mode = env::args()
        .any(|arg| arg == ARG_DEBUG || user_input.contains(ARG_DEBUG) || get_env_flag(ENV_DEBUG));

    // get user's shell name
    // when env::var("SHELL") is not set, use BASH_VERSION or ZSH_VERSION to guess the shell
    let shell = match env::var("SHELL") {
        Ok(value) => value,
        Err(_e) => {
            if env::var("BASH_VERSION").is_ok() {
                "Bash".to_string()
            } else if env::var("ZSH_VERSION").is_ok() {
                "zsh".to_string()
            } else {
                "Unknown".to_string()
            }
        }
    };

    // print user info
    if debug_mode {
        eprintln!("OS: {}", OS);
        eprintln!("osArch: {}", ARCH);
        eprintln!("shell: {}", shell);
    }

    let user_info: UserInfo = UserInfo {
        arch: ARCH.to_string(),
        os: OS.to_string(),
        shell,
    };

    if debug_mode {
        eprintln!("args: {}", args.join(" "));
        eprintln!("is_using_stdin: {}", is_using_stdin);
        eprintln!("user_input: {}", user_input);
        eprintln!("user_input_without_flags: {}", user_input_without_flags);
        eprintln!("debug_mode: {}", debug_mode);
    }

    let templates = prompts::get_template();
    let mut vars = std::collections::HashMap::new();
    vars.insert("user_input".to_owned(), user_input_without_flags.to_owned());
    vars.insert("user_os".to_owned(), user_info.os.to_owned());
    vars.insert("user_arch".to_owned(), user_info.arch.to_owned());
    vars.insert("user_shell".to_owned(), user_info.shell.to_owned());

    let system_message = templates.render("SYSTEM_PROMPT", &vars).unwrap();
    let user_input = templates.render("USER_PROMPT", &vars).unwrap();

    let response = chat(user_input, system_message, &debug_mode);

    let response = match response {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Communication with LLM provider failed: {}", e);
            process::exit(1);
        }
    };

    let tmux_session_name = "ask_sh_session";

    // Create executor for a specific tmux pane
    let tmux_executor = TmuxCommandExecutor::new(&tmux_session_name);
    let commands = get_commands_to_run(&response);

    // print suggested commands to stdout to further process
    for command in commands {
        println!("");
        println!("I'll run the following command:");
        println!("");
        println!("{}", create_box(&command, ""));
        println!("");

        let command_output = tmux_executor.execute_command(&command);
        println!("The command returned: {}", command_output.unwrap());
    }

    match Command::new("tmux")
        .arg("kill-session")
        .arg("-a")
        .arg("-t")
        .arg(&tmux_session_name)
        .output()
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Somehow tmux capture-pane -p failed: {}", e);
        }
    }
}
