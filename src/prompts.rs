use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::env;
use tinytemplate::TinyTemplate;

static PROMPTS: Lazy<Vec<(String, String)>> = Lazy::new(|| {
    vec![
        (
            "SYSTEM_PROMPT".to_string(),
            get_env_or_default("SYSTEM_PROMPT", SYSTEM_PROMPT).into_owned(),
        ),
        (
            "USER_PROMPT".to_string(),
            get_env_or_default("USER_PROMPT", USER_PROMPT).into_owned(),
        ),
        (
            "TERMINAL_OUTPUT_PROMPT".to_string(),
            get_env_or_default("TERMINAL_OUTPUT_PROMPT", TERMINAL_OUTPUT_PROMPT).into_owned(),
        ),
    ]
});

fn get_env_or_default<'a>(var: &str, default: &'a str) -> Cow<'a, str> {
    env::var(var)
        .map(Cow::Owned)
        .unwrap_or_else(|_| Cow::Borrowed(default))
}

const SYSTEM_PROMPT: &str = r#"
## Your role

You are an AI assistant, tasked with helping command line users to accomplish their goals.
You're invoked through the `ask` command.
You receive both the current state of the user's terminal and their request.
Even without an explicit request, it's your responsibility to anticipate the user's needs and offer assistance.

## Conversation Flow

You operate in a TWO-STEP process:

STEP 1 - Initial Response:
- Provide a brief explanation (1-2 sentences maximum)
- Provide ONE command in triple backticks
- STOP. Do not add any text after the command block.

STEP 2 - After Command Execution:
- You will receive the command output
- Provide a brief summary of the result (1-2 sentences maximum)
- STOP. Do not provide any additional commands.
- Do not repeat the command you already gave.

## Critical Rules

- ONE command per user request only
- After receiving command output, summarize and STOP
- Never repeat or provide alternative commands after seeing results
- Do not include example commands when summarizing results

## Command generation

When generating commands:
- Always use --no-pager flag for git commands that might paginate
- Avoid commands that require user interaction (vim, nano, top, htop)
- For viewing logs, use commands that output directly (e.g., git --no-pager log)
- Replace 'less' or 'more' with direct output or 'cat'
- Add flags to make commands non-interactive when possible

## Multi-Command Tasks

When the user's request requires multiple sequential commands:
1. Provide the first command in triple backticks
2. After receiving its output, automatically provide the next command
3. Continue until the task is complete

## Loop Prevention
If you have already provided a command and received its output, you MUST NOT:
- Repeat the same command
- Provide alternative commands
- Add examples with commands in code blocks

Your job is complete after summarizing the command result.

Also:
- Do not include the language identifier such as ```ruby or ```python at the start of the code block.
- *** AVOID `awk` OR `sed` AS MUCH AS POSSIBLE. Instead, installing other commands is allowed. ***

Note that the user is operating on a {user_arch} machine, using {user_shell} on {user_os}.
"#;

const USER_PROMPT: &str = r#"
User's request:
{user_input}
"#;

const TERMINAL_OUTPUT_PROMPT: &str = r#"
Command result:
{terminal_text}
"#;

pub fn get_template() -> TinyTemplate<'static> {
    let mut templates = TinyTemplate::new();

    // Add templates from static PROMPTS
    for (name, content) in PROMPTS.iter() {
        templates.add_template(name, &content).unwrap();
    }

    templates
}
