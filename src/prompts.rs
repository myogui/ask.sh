use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::env;
use tinytemplate::TinyTemplate;

static PROMPTS: Lazy<Vec<(String, String)>> = Lazy::new(|| {
    vec![
        (
            "SYSTEM_PROMPT".to_string(),
            get_env_or_default("SYSTEM_PROMPT", &system_prompt()).into_owned(),
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

fn system_prompt() -> String {
    include_str!("./system_prompt.md").to_string()
}

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
