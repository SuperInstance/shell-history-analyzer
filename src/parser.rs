use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A single history entry with optional timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub command: String,
    pub timestamp: Option<DateTime<Local>>,
    pub index: usize,
}

/// Detect the most likely history file for the current user.
pub fn detect_history_file() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let home = Path::new(&home);

    // Prefer zsh if it exists and is larger
    let zsh = home.join(".zsh_history");
    let bash = home.join(".bash_history");

    match (zsh.exists(), bash.exists()) {
        (true, _) => Some(zsh),
        (false, true) => Some(bash),
        _ => None,
    }
}

/// Parse a history file, auto-detecting format (bash vs zsh).
pub fn parse_history(path: &Path) -> anyhow::Result<Vec<HistoryEntry>> {
    let content = fs::read_to_string(path)?;
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    if filename.contains("zsh") {
        parse_zsh_history(&content)
    } else {
        parse_bash_history(&content)
    }
}

/// Parse bash history format.
/// Supports:
/// - Plain lines (no timestamp)
/// - `#timestamp` prefixed lines (HISTTIMEFORMAT)
/// - Multi-line commands joined with backslash-newline
pub fn parse_bash_history(content: &str) -> anyhow::Result<Vec<HistoryEntry>> {
    let mut entries = Vec::new();
    let mut current_timestamp: Option<i64> = None;
    let mut current_command = String::new();
    let mut idx = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Check for timestamp line: `#1234567890`
        if let Some(ts_str) = trimmed.strip_prefix('#') {
            if let Ok(ts) = ts_str.trim().parse::<i64>() {
                if ts > 1_000_000_000 && ts < 2_000_000_000 {
                    current_timestamp = Some(ts);
                    continue;
                }
            }
        }

        // Accumulate multi-line commands (ending with \)
        if trimmed.ends_with('\\') {
            current_command.push_str(trimmed.trim_end_matches('\\'));
            current_command.push(' ');
            continue;
        }

        if !current_command.is_empty() {
            current_command.push_str(trimmed);
        } else {
            current_command = trimmed.to_string();
        }

        let timestamp = current_timestamp
            .and_then(|ts| Local.timestamp_opt(ts, 0).single());

        entries.push(HistoryEntry {
            command: current_command.clone(),
            timestamp,
            index: idx,
        });
        idx += 1;
        current_command.clear();
        current_timestamp = None;
    }

    // Handle trailing command
    if !current_command.is_empty() {
        let timestamp = current_timestamp
            .and_then(|ts| Local.timestamp_opt(ts, 0).single());
        entries.push(HistoryEntry {
            command: current_command,
            timestamp,
            index: idx,
        });
    }

    Ok(entries)
}

/// Parse zsh extended history format.
/// Format: `: timestamp:elapsed;command`
pub fn parse_zsh_history(content: &str) -> anyhow::Result<Vec<HistoryEntry>> {
    let mut entries = Vec::new();
    let mut idx = 0;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Zsh extended format: `: 1234567890:0;command`
        if let Some(rest) = trimmed.strip_prefix(':') {
            if let Some(semicolon_pos) = rest.find(';') {
                let meta = &rest[..semicolon_pos];
                let command = &rest[semicolon_pos + 1..];

                // Parse timestamp from meta (format: " timestamp:elapsed")
                let timestamp = meta
                    .split(':')
                    .next()
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .and_then(|ts| {
                        if ts > 1_000_000_000 && ts < 2_000_000_000 {
                            Local.timestamp_opt(ts, 0).single()
                        } else {
                            None
                        }
                    });

                if !command.is_empty() {
                    entries.push(HistoryEntry {
                        command: unescape_zsh_command(command),
                        timestamp,
                        index: idx,
                    });
                    idx += 1;
                }
                continue;
            }
        }

        // Fallback: plain command line
        entries.push(HistoryEntry {
            command: trimmed.to_string(),
            timestamp: None,
            index: idx,
        });
        idx += 1;
    }

    Ok(entries)
}

/// Unescape zsh command escaping.
fn unescape_zsh_command(cmd: &str) -> String {
    let mut result = String::with_capacity(cmd.len());
    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('\\') => { result.push('\\'); chars.next(); }
                Some('n') => { result.push('\n'); chars.next(); }
                Some('t') => { result.push('\t'); chars.next(); }
                Some(&next) => { result.push(next); chars.next(); }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Extract the base command (first word) from a command string.
pub fn base_command(cmd: &str) -> String {
    let cmd = cmd.trim_start();

    // Handle sudo, env, etc. prefixes
    let prefixes = ["sudo ", "env ", "time ", "nocorrect ", "builtin "];
    let mut cmd = cmd;
    for prefix in &prefixes {
        if cmd.starts_with(prefix) {
            cmd = &cmd[prefix.len()..];
            break;
        }
    }

    // Handle pipes — take first command
    if let Some(pipe_pos) = cmd.find('|') {
        cmd = &cmd[..pipe_pos];
    }

    // Handle && and ||
    for sep in ["&&", "||", ";"] {
        if let Some(pos) = cmd.find(sep) {
            cmd = &cmd[..pos];
            break;
        }
    }

    // Handle redirections
    for redir in ["<", ">", ">>", "2>", "&>"] {
        if let Some(pos) = cmd.find(redir) {
            cmd = &cmd[..pos];
            break;
        }
    }

    cmd.trim()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

/// Classify a command into a category.
pub fn classify_command(cmd: &str) -> CommandCategory {
    let base = base_command(cmd).to_lowercase();
    let args = cmd.to_lowercase();

    match base.as_str() {
        "git" => CommandCategory::VersionControl,
        "cargo" | "npm" | "yarn" | "pnpm" | "pip" | "pip3" | "gem" | "make" | "cmake" | "gradle" | "mvn" => CommandCategory::Build,
        "vim" | "vi" | "nano" | "emacs" | "code" | "hx" | "micro" | "ed" => CommandCategory::Editor,
        "ls" | "dir" | "tree" | "find" | "fd" | "fzf" | "rg" | "grep" | "ag" | "ack" | "cat" | "less" | "more" | "head" | "tail" | "bat" => CommandCategory::Navigation,
        "cd" | "pushd" | "popd" => CommandCategory::Navigation,
        "ssh" | "scp" | "rsync" | "curl" | "wget" | "ping" | "nc" | "ncat" => CommandCategory::Network,
        "docker" | "podman" | "kubectl" | "k3s" | "docker-compose" => CommandCategory::Containers,
        "python" | "python3" | "node" | "ruby" | "perl" | "bash" | "zsh" | "sh" | "lua" | "racket" => CommandCategory::Runtime,
        "export" | "source" | "alias" | "set" | "unset" | "env" => CommandCategory::Shell,
        "rm" | "mv" | "cp" | "mkdir" | "touch" | "chmod" | "chown" | "ln" | "tar" | "zip" | "unzip" | "gzip" | "trash" => CommandCategory::FileSystem,
        "apt" | "apt-get" | "yum" | "dnf" | "pacman" | "brew" | "nix" => CommandCategory::PackageManagement,
        "systemctl" | "service" | "journalctl" | "dmesg" | "top" | "htop" | "ps" | "kill" | "nice" => CommandCategory::System,
        "gh" => CommandCategory::DevPlatform,
        _ if args.contains("test") || args.contains("spec") => CommandCategory::Test,
        _ => CommandCategory::Other(base),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandCategory {
    VersionControl,
    Build,
    Editor,
    Navigation,
    Network,
    Containers,
    Runtime,
    Shell,
    FileSystem,
    PackageManagement,
    System,
    DevPlatform,
    Test,
    Other(String),
}

impl std::fmt::Display for CommandCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandCategory::VersionControl => write!(f, "🔀 VCS"),
            CommandCategory::Build => write!(f, "🔨 Build"),
            CommandCategory::Editor => write!(f, "✏️  Editor"),
            CommandCategory::Navigation => write!(f, "🔍 Navigate"),
            CommandCategory::Network => write!(f, "🌐 Network"),
            CommandCategory::Containers => write!(f, "🐳 Containers"),
            CommandCategory::Runtime => write!(f, "▶️  Runtime"),
            CommandCategory::Shell => write!(f, "🐚 Shell"),
            CommandCategory::FileSystem => write!(f, "📁 FileSystem"),
            CommandCategory::PackageManagement => write!(f, "📦 Packages"),
            CommandCategory::System => write!(f, "⚙️  System"),
            CommandCategory::DevPlatform => write!(f, "🐙 DevPlatform"),
            CommandCategory::Test => write!(f, "🧪 Test"),
            CommandCategory::Other(s) => write!(f, "❓ {}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_command_simple() {
        assert_eq!(base_command("git status"), "git");
    }

    #[test]
    fn test_base_command_sudo() {
        assert_eq!(base_command("sudo apt install foo"), "apt");
    }

    #[test]
    fn test_base_command_pipe() {
        assert_eq!(base_command("cat file.txt | grep foo"), "cat");
    }

    #[test]
    fn test_base_command_and() {
        assert_eq!(base_command("make && make test"), "make");
    }

    #[test]
    fn test_base_command_redirect() {
        assert_eq!(base_command("echo hello > file.txt"), "echo");
    }

    #[test]
    fn test_parse_bash_history_plain() {
        let input = "ls\ncd projects\ngit status\n";
        let entries = parse_bash_history(input).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].command, "ls");
        assert_eq!(entries[1].command, "cd projects");
        assert_eq!(entries[2].command, "git status");
    }

    #[test]
    fn test_parse_bash_history_timestamps() {
        let input = "#1703260800\nls\n#1703260801\ngit status\n";
        let entries = parse_bash_history(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].timestamp.is_some());
        assert!(entries[1].timestamp.is_some());
    }

    #[test]
    fn test_parse_bash_history_multiline() {
        let input = "echo hello \\\nworld\nls\n";
        let entries = parse_bash_history(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "echo hello  world");  // backslash replaced with space
    }

    #[test]
    fn test_parse_zsh_history() {
        let input = ": 1703260800:0;ls\n: 1703260801:0;git status\n";
        let entries = parse_zsh_history(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "ls");
        assert_eq!(entries[1].command, "git status");
        assert!(entries[0].timestamp.is_some());
    }

    #[test]
    fn test_parse_zsh_history_escaped() {
        let input = ": 1703260800:0;echo hello\\\\ world\n";
        let entries = parse_zsh_history(input).unwrap();
        assert_eq!(entries[0].command, "echo hello\\ world");
    }

    #[test]
    fn test_classify_command() {
        assert_eq!(classify_command("git commit -m 'fix'"), CommandCategory::VersionControl);
        assert_eq!(classify_command("cargo build"), CommandCategory::Build);
        assert_eq!(classify_command("vim file.rs"), CommandCategory::Editor);
        assert_eq!(classify_command("ls -la"), CommandCategory::Navigation);
    }

    #[test]
    fn test_detect_history_file_returns_some() {
        // At least bash_history exists in this env
        let result = detect_history_file();
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_empty_history() {
        let entries = parse_bash_history("").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_comment_not_timestamp() {
        // Comments that aren't timestamps should be treated as commands
        let input = "# this is a comment\nls\n";
        let entries = parse_bash_history(input).unwrap();
        assert_eq!(entries.len(), 2);
        // The comment isn't a valid timestamp so it's treated as a command
        assert_eq!(entries[0].command, "# this is a comment");
    }
}
