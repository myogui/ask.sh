use std::process::Command;
use std::time::Duration;
use std::{env, thread};

use uuid::Uuid;

pub struct TmuxCommandExecutor {
    session: String,
    prompt_pattern: String,
}

impl TmuxCommandExecutor {
    // Create a new TmuxCommandExecutor for a specific pane
    pub fn new(session: &str) -> Self {
        let executor = Self {
            session: session.to_string(),
            prompt_pattern: Self::capture_prompt_pattern(&session.to_string()),
        };

        // Create the session
        let result = executor.ensure_session();

        if result.is_err() {}

        executor
    }

    pub fn execute_command(&self, command: &str) -> Result<String, Box<dyn std::error::Error>> {
        let session_pane = format!("{}", self.session);

        // Send command with marker
        let marker = format!("__CMD_COMPLETE_{}__", Uuid::new_v4());
        let full_command = format!("{} && echo {}", command, marker);

        // Set Tmux window size
        Command::new("tmux")
            .args(&["set-option", "-g", "window-size", "manual"])
            .output()?;
        Command::new("tmux")
            .args(&["resize-window", "-x", "1000"])
            .output()?;

        // Clear history
        Command::new("tmux")
            .args(&["clear-history", "-t", &session_pane])
            .output()?;

        // Clear visible screen
        Command::new("tmux")
            .args(&["send-keys", "-t", &session_pane, "C-l"])
            .output()?;

        // Small delay to ensure clear completes
        thread::sleep(Duration::from_millis(100));

        // Send the command
        Command::new("tmux")
            .args(&["send-keys", "-t", &session_pane, &full_command, "Enter"])
            .output()?;

        // Wait for command to complete
        // Poll until prompt reappears or timeout
        let mut attempts = 0;
        let max_attempts = 100;

        loop {
            thread::sleep(Duration::from_millis(100));

            let output = Command::new("tmux")
                .args(&["capture-pane", "-p", "-t", &session_pane])
                .output()?;

            let output_stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let content = output_stdout.trim_end();

            // if a single line contains the marker and doesn't contain 'echo MARKER'
            let marker_found = content
                .lines()
                .any(|line| line.contains(&marker) && !line.contains(&format!("echo {}", marker)));

            if marker_found {
                break;
            }

            let output_stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let content_stderr = output_stderr.trim_end();

            if content_stderr != "" {
                return Ok(content_stderr.to_string());
            }

            attempts += 1;

            if attempts >= max_attempts {
                return Err("Command timeout".into());
            }
        }

        // Capture the final output
        let output = Command::new("tmux")
            .args(&[
                "capture-pane",
                "-pJ",
                "-t",
                &session_pane,
                "-S",
                "-",
                "-E",
                "-",
            ])
            .output()?;

        let content = String::from_utf8_lossy(&output.stdout);
        let cleaned = self.clean_command_output(&content, &marker);

        Ok(cleaned.to_string())
    }

    fn capture_prompt_pattern(pane: &str) -> String {
        // Send a newline to trigger a fresh prompt
        Command::new("tmux")
            .arg("send-keys")
            .arg("-t")
            .arg(&pane)
            .arg("")
            .arg("Enter");

        let mut prompt_line = "".to_string();

        // Wait for command to complete
        // Poll until prompt reappears or timeout
        let mut attempts = 0;
        let max_attempts = 100;

        loop {
            thread::sleep(Duration::from_millis(10));

            // Capture the pane
            let output = Command::new("tmux")
                .arg("capture-pane")
                .arg("-t")
                .arg(&pane)
                .arg("-p")
                .output();

            let output_stdout = String::from_utf8_lossy(&output.unwrap().stdout).to_string();

            if output_stdout.trim() != "" {
                // Get the last few lines (your prompt)
                prompt_line = output_stdout
                    .trim()
                    .lines()
                    .last()
                    .unwrap_or("")
                    .to_string();

                if prompt_line != "" {
                    break;
                }
            }

            attempts += 1;

            if attempts >= max_attempts {
                break;
            }
        }

        prompt_line
    }

    /// Ensure the tmux session exists
    fn ensure_session(&self) -> Result<(), Box<dyn std::error::Error>> {
        let in_tmux: bool;

        match env::var("TMUX") {
            Ok(_value) => in_tmux = true,
            Err(_) => in_tmux = false,
        }

        if !in_tmux {
            // Start server if not running
            let _ = Command::new("tmux")
                .arg("start-server")
                .env_remove("TMUX")
                .output();

            thread::sleep(Duration::from_millis(100));

            // Check if session exists
            let check = Command::new("tmux")
                .args(&["has-session", "-t", &self.session])
                .output()?;

            if check.status.success() {
                return Ok(()); // Session already exists
            }

            // Create session
            let output = Command::new("tmux")
                .args(&["new-session", "-d", "-s", &self.session])
                .output()?;

            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to create session: {}", error).into());
            }

            // Wait for session to be ready
            thread::sleep(Duration::from_millis(200));

            // Verify session was created
            let verify = Command::new("tmux")
                .args(&["has-session", "-t", &self.session])
                .output()?;

            if !verify.status.success() {
                return Err("Session created but not found".into());
            }
        }

        Ok(())
    }

    fn clean_command_output(&self, content: &str, marker: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut collecting = false;

        for line in lines.iter().rev() {
            if line.contains(marker) && !line.contains(&format!("echo {}", marker)) {
                // Found marker line - clean it and start collecting
                let cleaned = line.replace(marker, "");
                if !cleaned.trim().is_empty() {
                    result.push(cleaned.to_string());
                }
                collecting = true;
            } else if collecting {
                // Stop when we hit the prompt line
                if line.starts_with(&self.prompt_pattern) {
                    break;
                }
                // Skip empty lines and wrapped prompts
                if !line.trim().is_empty() && !line.starts_with(&self.prompt_pattern) {
                    result.push(line.to_string());
                }
            }
        }

        result.reverse();
        result.join("\n")
    }
}
