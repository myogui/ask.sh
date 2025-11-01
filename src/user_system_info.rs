use std::env::{
    self,
    consts::{ARCH, OS},
};

pub struct UserSystemInfo {
    pub arch: String,
    pub os: String,
    pub shell: String,
}

impl UserSystemInfo {
    pub fn new() -> Self {
        Self {
            arch: ARCH.to_string(),
            os: OS.to_string(),
            shell: get_system_shell(),
        }
    }
}

fn get_system_shell() -> String {
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

    shell
}
