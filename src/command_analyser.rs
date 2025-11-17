pub struct CommandAnalyser;

impl CommandAnalyser {
    /// Checks if a command requires user approval before execution.
    /// Returns (needs_approval, reason)
    pub fn requires_approval(command: &str) -> (bool, Option<&'static str>) {
        let cmd = command.trim();
        let base_cmd = Self::extract_base_command(cmd);

        if base_cmd == "git" {
            return Self::check_git_command(cmd);
        }

        if Self::is_file_modifying(&base_cmd) {
            return (true, Some("modifies files or system state"));
        }

        if Self::is_package_manager(&base_cmd) {
            return (true, Some("installs or manages software"));
        }

        if Self::is_network_operation(&base_cmd) {
            return (true, Some("performs network operations"));
        }

        if Self::is_system_config(cmd) {
            return (true, Some("modifies system configuration"));
        }

        if Self::is_database_operation(&base_cmd) {
            return (true, Some("performs database operations"));
        }

        if Self::is_risky(cmd, &base_cmd) {
            return (true, Some("potentially risky operation"));
        }

        // Safe read-only command
        (false, None)
    }

    /// Extracts the base command name from a shell command string
    fn extract_base_command(cmd: &str) -> String {
        cmd.split_whitespace()
            .skip_while(|word| word.contains('=')) // Skip env vars
            .next()
            .and_then(|segment| segment.split('|').next()) // Handle pipes
            .and_then(|segment| segment.split_whitespace().next()) // Get first word
            .unwrap_or("")
            .to_lowercase()
    }

    fn is_file_modifying(cmd: &str) -> bool {
        const FILE_COMMANDS: &[&str] = &[
            "rm", "rmdir", "mv", "cp", "dd", "touch", "mkdir", "ln", "chmod", "chown", "chgrp",
            "shred", "nano", "vim", "vi", "emacs", "sed", "tee", "truncate", "split", ">>", ">",
        ];

        FILE_COMMANDS.contains(&cmd) || cmd.starts_with("write") || cmd.ends_with("fs")
    }

    fn is_package_manager(cmd: &str) -> bool {
        const PACKAGE_MANAGERS: &[&str] = &[
            "brew", "apt", "apt-get", "yum", "dnf", "pacman", "npm", "yarn", "pnpm", "pip", "pip3",
            "cargo", "gem", "go", "composer", "mvn", "gradle", "snap", "flatpak", "apk", "zypper",
        ];

        PACKAGE_MANAGERS.contains(&cmd) || cmd.starts_with("install")
    }

    fn is_network_operation(cmd: &str) -> bool {
        const NETWORK_COMMANDS: &[&str] = &[
            "curl", "wget", "fetch", "http", "scp", "rsync", "ssh", "sftp", "ftp", "nc", "netcat",
            "telnet",
        ];

        NETWORK_COMMANDS.contains(&cmd)
    }

    fn is_system_config(full_cmd: &str) -> bool {
        // Check for system paths
        if full_cmd.contains("/etc/") || full_cmd.contains("/sys/") {
            return true;
        }

        const SYSTEM_COMMANDS: &[&str] = &[
            "systemctl",
            "service",
            "launchctl",
            "export",
            "source",
            "chsh",
            "usermod",
            "useradd",
            "userdel",
            "groupadd",
            "groupdel",
            "passwd",
            "sudo",
            "su",
            "mount",
            "umount",
            "sysctl",
            "modprobe",
        ];

        let base = Self::extract_base_command(full_cmd);
        SYSTEM_COMMANDS.contains(&base.as_str())
    }

    fn is_database_operation(cmd: &str) -> bool {
        const DB_COMMANDS: &[&str] = &[
            "mysql",
            "psql",
            "sqlite",
            "sqlite3",
            "mongo",
            "mongosh",
            "redis-cli",
            "influx",
            "cql",
            "cqlsh",
        ];

        const SQL_KEYWORDS: &[&str] = &["DROP", "DELETE", "UPDATE", "INSERT", "ALTER", "CREATE"];

        DB_COMMANDS.contains(&cmd) || SQL_KEYWORDS.iter().any(|kw| cmd.contains(kw))
    }

    fn is_risky(full_cmd: &str, base_cmd: &str) -> bool {
        const DANGEROUS_PATTERNS: &[&str] = &[
            "/dev/",
            "rm -rf",
            "rm -fr",
            ":(){ :|:& };:",
            "/dev/null",
            "> /dev/sda",
            "mkfs",
            "format",
        ];

        const DANGEROUS_COMMANDS: &[&str] = &[
            "eval", "exec", "sh", "bash", "zsh", "python", "perl", "ruby", "kill", "killall",
            "pkill", "reboot", "shutdown", "halt", "crontab", "at", "batch",
        ];

        DANGEROUS_PATTERNS.iter().any(|p| full_cmd.contains(p))
            || DANGEROUS_COMMANDS.contains(&base_cmd)
    }

    fn check_git_command(cmd: &str) -> (bool, Option<&'static str>) {
        let cmd_lower = cmd.to_lowercase();

        if Self::is_modifying_git(&cmd_lower) {
            return (true, Some("modifies git repository or remote"));
        }

        if Self::is_destructive_git(&cmd_lower) {
            return (true, Some("destructive git operation"));
        }

        // Read-only git command
        (false, None)
    }

    fn is_modifying_git(cmd: &str) -> bool {
        const LOCAL_MODIFY: &[&str] = &[
            "git add",
            "git commit",
            "git checkout",
            "git switch",
            "git restore",
            "git merge",
            "git rebase",
            "git cherry-pick",
            "git revert",
            "git stash",
            "git rm",
            "git mv",
            "git apply",
            "git am",
            "git reset",
            "git submodule",
        ];

        const NETWORK_OPS: &[&str] = &[
            "git clone",
            "git fetch",
            "git pull",
            "git push",
            "git remote add",
            "git remote remove",
            "git remote set-url",
        ];

        const CONFIG_OPS: &[&str] = &["git worktree add", "git worktree remove"];

        LOCAL_MODIFY.iter().any(|p| cmd.starts_with(p))
            || NETWORK_OPS.iter().any(|p| cmd.starts_with(p))
            || CONFIG_OPS.iter().any(|p| cmd.starts_with(p))
            || (cmd.starts_with("git config") && !cmd.contains("--list") && !cmd.contains("--get"))
    }

    fn is_destructive_git(cmd: &str) -> bool {
        const DESTRUCTIVE_PATTERNS: &[&str] = &[
            "reset --hard",
            "clean -f",
            "clean -d",
            "clean -x",
            "branch -d",
            "branch -D",
            "push --force",
            "push -f",
            "push --mirror",
            "filter-branch",
            "reflog delete",
            "reflog expire",
            "prune",
            "gc --prune",
        ];

        DESTRUCTIVE_PATTERNS.iter().any(|p| cmd.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_commands() {
        let safe_cmds = [
            "ls -la",
            "cat file.txt",
            "git status",
            "git log",
            "grep pattern file",
            "find . -name '*.rs'",
            "pwd",
        ];

        for cmd in &safe_cmds {
            assert_eq!(
                CommandAnalyser::requires_approval(cmd).0,
                false,
                "Expected '{}' to be safe",
                cmd
            );
        }
    }

    #[test]
    fn test_file_modification() {
        let modify_cmds = [
            "rm file.txt",
            "mv old.txt new.txt",
            "chmod 755 script.sh",
            "vim config.txt",
        ];

        for cmd in &modify_cmds {
            assert_eq!(
                CommandAnalyser::requires_approval(cmd).0,
                true,
                "Expected '{}' to need approval",
                cmd
            );
        }
    }

    #[test]
    fn test_package_managers() {
        let pkg_cmds = [
            "npm install express",
            "brew install git",
            "pip install requests",
            "cargo install ripgrep",
        ];

        for cmd in &pkg_cmds {
            assert_eq!(
                CommandAnalyser::requires_approval(cmd).0,
                true,
                "Expected '{}' to need approval",
                cmd
            );
        }
    }

    #[test]
    fn test_network_commands() {
        let net_cmds = [
            "curl https://example.com",
            "wget file.tar.gz",
            "git clone repo",
            "scp file.txt remote:",
        ];

        for cmd in &net_cmds {
            assert_eq!(
                CommandAnalyser::requires_approval(cmd).0,
                true,
                "Expected '{}' to need approval",
                cmd
            );
        }
    }

    #[test]
    fn test_system_config() {
        let sys_cmds = [
            "systemctl restart nginx",
            "sudo vim /etc/hosts",
            "export PATH=/new/path",
            "useradd newuser",
        ];

        for cmd in &sys_cmds {
            assert_eq!(
                CommandAnalyser::requires_approval(cmd).0,
                true,
                "Expected '{}' to need approval",
                cmd
            );
        }
    }

    #[test]
    fn test_risky_commands() {
        let risky_cmds = [
            "rm -rf /",
            "eval $(command)",
            "kill -9 1234",
            "reboot",
            "dd if=/dev/zero of=/dev/sda",
        ];

        for cmd in &risky_cmds {
            assert_eq!(
                CommandAnalyser::requires_approval(cmd).0,
                true,
                "Expected '{}' to need approval",
                cmd
            );
        }
    }

    #[test]
    fn test_git_commands() {
        let safe_git = ["git status", "git log", "git diff", "git branch"];
        let modifying_git = ["git add .", "git commit -m 'test'", "git push origin main"];
        let destructive_git = ["git clean -f"];

        for cmd in &safe_git {
            assert_eq!(
                CommandAnalyser::requires_approval(cmd).0,
                false,
                "Expected '{}' to be safe",
                cmd
            );
        }

        for cmd in &modifying_git {
            assert_eq!(
                CommandAnalyser::requires_approval(cmd).0,
                true,
                "Expected '{}' to need approval",
                cmd
            );
        }

        for cmd in &destructive_git {
            let (needs, reason) = CommandAnalyser::requires_approval(cmd);
            assert_eq!(needs, true, "Expected '{}' to need approval", cmd);
            assert_eq!(reason, Some("destructive git operation"));
        }
    }
}
