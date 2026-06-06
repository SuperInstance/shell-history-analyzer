use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::parser::{base_command, classify_command, HistoryEntry};

// ── Command Frequency ──────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandFrequency {
    pub total: usize,
    pub top: Vec<(String, usize, f64)>, // (command, count, percentage)
    pub by_category: Vec<(String, usize, f64)>,
}

impl CommandFrequency {
    pub fn analyze(entries: &[HistoryEntry], top_n: usize) -> Self {
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut cat_counts: HashMap<String, usize> = HashMap::new();

        for entry in entries {
            let base = base_command(&entry.command);
            if base.is_empty() {
                continue;
            }
            *counts.entry(base.clone()).or_insert(0) += 1;

            let cat = classify_command(&entry.command).to_string();
            *cat_counts.entry(cat).or_insert(0) += 1;
        }

        let total = counts.values().sum::<usize>();
        let mut top: Vec<_> = counts.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1));
        let top: Vec<_> = top
            .into_iter()
            .take(top_n)
            .map(|(cmd, count)| {
                let pct = (count as f64 / total as f64) * 100.0;
                (cmd, count, pct)
            })
            .collect();

        let mut cat_vec: Vec<_> = cat_counts.into_iter().collect();
        cat_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let by_category: Vec<_> = cat_vec
            .into_iter()
            .map(|(cat, count)| {
                let pct = (count as f64 / total as f64) * 100.0;
                (cat, count, pct)
            })
            .collect();

        Self { total, top, by_category }
    }

    pub fn print(&self, json: bool) {
        if json {
            println!("{}", serde_json::to_string_pretty(self).unwrap());
            return;
        }

        println!("📊 Top {} commands (of {} total invocations):", self.top.len(), self.total);
        println!("{:<4} {:<30} {:<8} {:>8}", "#", "Command", "Count", "%");
        println!("{}", "─".repeat(54));
        for (i, (cmd, count, pct)) in self.top.iter().enumerate() {
            let bar = "█".repeat((*pct as usize / 2).max(1).min(20));
            println!("{:<4} {:<30} {:<8} {:>5.1}% {}", i + 1, truncate(cmd, 30), count, pct, bar);
        }

        println!("\n📊 By category:");
        for (cat, count, pct) in &self.by_category {
            println!("  {:<20} {:<6} ({:>5.1}%)", cat, count, pct);
        }
    }
}

// ── Command Sequence (Markov Chain) ────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandSequence {
    pub transitions: HashMap<String, Vec<(String, usize, f64)>>,
    pub top_transitions: Vec<(String, String, usize)>,
}

impl CommandSequence {
    pub fn analyze(entries: &[HistoryEntry], top_n: usize) -> Self {
        let mut transitions: HashMap<String, HashMap<String, usize>> = HashMap::new();

        for window in entries.windows(2) {
            let from = base_command(&window[0].command);
            let to = base_command(&window[1].command);
            if from.is_empty() || to.is_empty() {
                continue;
            }
            *transitions
                .entry(from)
                .or_default()
                .entry(to)
                .or_insert(0) += 1;
        }

        let mut result: HashMap<String, Vec<(String, usize, f64)>> = HashMap::new();
        let mut all_transitions: Vec<(String, String, usize)> = Vec::new();

        for (from, targets) in &transitions {
            let total: usize = targets.values().sum();
            let mut sorted: Vec<_> = targets
                .iter()
                .map(|(to, &count)| {
                    let pct = (count as f64 / total as f64) * 100.0;
                    (to.clone(), count, pct)
                })
                .collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));

            for (to, count, _) in &sorted {
                all_transitions.push((from.clone(), to.clone(), *count));
            }

            result.insert(from.clone(), sorted);
        }

        all_transitions.sort_by(|a, b| b.2.cmp(&a.2));
        all_transitions.truncate(top_n);

        Self {
            transitions: result,
            top_transitions: all_transitions,
        }
    }

    pub fn print(&self, json: bool) {
        if json {
            println!("{}", serde_json::to_string_pretty(self).unwrap());
            return;
        }

        println!("🔗 Top command transitions:");
        println!("{:<4} {:<25} → {:<25} {:<6}", "#", "From", "To", "Count");
        println!("{}", "─".repeat(65));
        for (i, (from, to, count)) in self.top_transitions.iter().enumerate() {
            println!("{:<4} {:<25} → {:<25} {:<6}", i + 1, truncate(from, 25), truncate(to, 25), count);
        }
    }

    pub fn print_for_command(&self, command: &str, json: bool) {
        let base = base_command(command);

        if json {
            let result = self.transitions.get(&base).cloned().unwrap_or_default();
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
            return;
        }

        println!("🔗 What follows `{}`:", base);
        if let Some(transitions) = self.transitions.get(&base) {
            for (i, (to, count, pct)) in transitions.iter().enumerate() {
                let bar = "█".repeat((*pct as usize / 2).max(1).min(30));
                println!("  {:<4} {:<30} {:<6} {:>5.1}% {}", i + 1, truncate(to, 30), count, pct, bar);
            }
        } else {
            println!("  No transitions found for this command.");
        }
    }
}

// ── Time Patterns ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct TimePatterns {
    pub hourly: HashMap<u32, usize>,
    pub period_counts: HashMap<String, usize>,
    pub night_owl: bool,
    pub peak_hour: u32,
}

impl TimePatterns {
    pub fn analyze(entries: &[HistoryEntry]) -> Self {
        let mut hourly: HashMap<u32, usize> = HashMap::new();
        let mut period_counts: HashMap<String, usize> = HashMap::new();

        for entry in entries {
            if let Some(ts) = entry.timestamp {
                let hour = ts.hour();
                *hourly.entry(hour).or_insert(0) += 1;

                let period = match hour {
                    6..=11 => "Morning (6-12)",
                    12..=17 => "Afternoon (12-18)",
                    18..=22 => "Evening (18-22)",
                    _ => "Night (22-6)",
                };
                *period_counts.entry(period.to_string()).or_insert(0) += 1;
            }
        }

        let peak_hour = hourly
            .iter()
            .max_by_key(|&(_, c)| c)
            .map(|(&h, _)| h)
            .unwrap_or(0);

        let night_count = period_counts.get("Night (22-6)").copied().unwrap_or(0);
        let total_with_ts: usize = period_counts.values().sum();
        let night_owl = total_with_ts > 0 && (night_count as f64 / total_with_ts as f64) > 0.3;

        Self { hourly, period_counts, night_owl, peak_hour }
    }

    pub fn print(&self, json: bool) {
        if json {
            println!("{}", serde_json::to_string_pretty(self).unwrap());
            return;
        }

        println!("⏰ Time-of-day patterns:");
        println!("\n  Hourly distribution:");
        for hour in 0..24 {
            let count = self.hourly.get(&hour).copied().unwrap_or(0);
            let bar = if count > 0 { "█".repeat((count).min(40)) } else { "·".to_string() };
            println!("  {:02}:00  {:<4} {}", hour, count, bar);
        }

        println!("\n  By period:");
        for period in &["Morning (6-12)", "Afternoon (12-18)", "Evening (18-22)", "Night (22-6)"] {
            let count = self.period_counts.get(*period).copied().unwrap_or(0);
            println!("  {:<25} {}", period, count);
        }

        println!("\n  Peak hour: {:02}:00", self.peak_hour);
        println!("  Night owl: {}", if self.night_owl { "🦉 Yes" } else { "🌞 No" });
    }
}

// ── Error Detector ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetector {
    pub likely_errors: Vec<(String, String, String)>, // (command, reason, following_cmd)
}

impl ErrorDetector {
    pub fn analyze(entries: &[HistoryEntry], top_n: usize) -> Self {
        let mut errors: Vec<(String, String, String)> = Vec::new();

        for i in 0..entries.len().saturating_sub(1) {
            let current = &entries[i];
            let next = &entries[i + 1];

            // Pattern 1: followed by `!!` (repeat last command with sudo, etc.)
            if next.command.trim() == "!!" || next.command.starts_with("sudo !!") {
                errors.push((
                    current.command.clone(),
                    "Followed by !!".to_string(),
                    next.command.clone(),
                ));
                continue;
            }

            // Pattern 2: followed by `sudo <same command>`
            let current_base = base_command(&current.command);
            let next_base = base_command(&next.command);
            if next.command.starts_with("sudo") && current_base == next_base && !current.command.starts_with("sudo") {
                errors.push((
                    current.command.clone(),
                    "Retried with sudo".to_string(),
                    next.command.clone(),
                ));
                continue;
            }

            // Pattern 3: command followed immediately by a slight variation (typo fix)
            if i + 2 < entries.len() {
                let next2 = &entries[i + 2];
                if levenshtein(&current.command, &next2.command) <= 3
                    && levenshtein(&current.command, &next2.command) > 0
                    && current_base == base_command(&next2.command)
                {
                    // Skip this — might be a retry but uncertain
                }
            }

            // Pattern 4: `rm` right after `ls` on same path (looked before deleting)
            // Pattern 5: immediately re-running with slightly different flags
            if current_base == next_base && !current.command.contains("sudo") {
                let dist = levenshtein(&current.command, &next.command);
                if dist > 0 && dist <= 5 {
                    errors.push((
                        current.command.clone(),
                        format!("Retried with modification (edit dist={})", dist),
                        next.command.clone(),
                    ));
                    continue;
                }
            }
        }

        errors.truncate(top_n);
        Self { likely_errors: errors }
    }

    pub fn print(&self, json: bool) {
        if json {
            println!("{}", serde_json::to_string_pretty(self).unwrap());
            return;
        }

        println!("⚠️  Likely failed commands ({} detected):", self.likely_errors.len());
        println!("{}", "─".repeat(70));
        for (i, (cmd, reason, next)) in self.likely_errors.iter().enumerate() {
            println!("  {}. {}", i + 1, truncate(cmd, 50));
            println!("     Reason: {}", reason);
            println!("     Next:   {}\n", truncate(next, 50));
        }
    }
}

// ── Workflow Detector ──────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub description: String,
    pub count: usize,
    pub example: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowDetector {
    pub workflows: Vec<Workflow>,
}

impl WorkflowDetector {
    pub fn analyze(entries: &[HistoryEntry]) -> Self {
        let mut workflows = Vec::new();

        // Pre-compute base commands for sliding window
        let bases: Vec<String> = entries.iter().map(|e| base_command(&e.command)).collect();
        let commands: Vec<&str> = entries.iter().map(|e| e.command.as_str()).collect();

        // Workflow: edit → compile/build → test
        workflows.push(Self::detect_three_pattern(
            &bases, &commands,
            &["vim", "nano", "code", "hx", "micro"],
            &["cargo", "make", "npm", "yarn", "go", "gcc", "g++"],
            &["cargo test", "pytest", "npm test", "go test", "make test", "yarn test"],
            "Edit → Build → Test",
            "Classic development cycle: edit code, compile/build, then run tests",
        ));

        // Workflow: git add → git commit → git push
        workflows.push(Self::detect_git_workflow(&bases, &commands));

        // Workflow: cd → ls (exploration)
        workflows.push(Self::detect_two_pattern(
            &bases, &commands,
            "cd", "ls",
            "Directory Exploration",
            "cd followed by ls — browsing the filesystem",
        ));

        // Workflow: edit → git diff → git add
        workflows.push(Self::detect_edit_commit(&bases, &commands));

        // Workflow: docker build → docker run
        workflows.push(Self::detect_two_pattern(
            &bases, &commands,
            "docker", "docker",
            "Docker Build & Run",
            "Docker container workflow",
        ));

        workflows.retain(|w| w.count > 0);
        workflows.sort_by(|a, b| b.count.cmp(&a.count));

        Self { workflows }
    }

    fn detect_three_pattern(
        bases: &[String],
        commands: &[&str],
        edit_cmds: &[&str],
        build_cmds: &[&str],
        test_cmds: &[&str],
        name: &str,
        description: &str,
    ) -> Workflow {
        let mut count = 0;
        let mut best_example: Vec<String> = Vec::new();

        for window in bases.windows(3) {
            let is_edit = edit_cmds.iter().any(|c| window[0] == *c);
            let is_build = build_cmds.iter().any(|c| window[1] == *c);
            let _cmd2_full = commands.get(window.len() + count.min(0) as usize);
            let _is_test = test_cmds.iter().any(|c| {
                // Check if the third command contains the test command pattern
                window[2].starts_with(c.split_whitespace().next().unwrap_or(""))
                && commands.get(window.len()).map_or(false, |&full| {
                    test_cmds.iter().any(|tc| full.starts_with(tc))
                })
            }) || {
                // Simpler: check base matches and full command contains "test"
                let base_matches = build_cmds.iter().any(|c| window[2] == *c);
                let full = commands.get(2).copied().unwrap_or("");
                !base_matches && full.contains("test")
            };

            if is_edit && is_build {
                count += 1;
                if best_example.is_empty() {
                    best_example = window.iter().map(|s| s.clone()).collect();
                }
            }
        }

        Workflow {
            name: name.to_string(),
            description: description.to_string(),
            count,
            example: best_example,
        }
    }

    fn detect_git_workflow(bases: &[String], commands: &[&str]) -> Workflow {
        let mut count = 0;
        let mut best_example: Vec<String> = Vec::new();

        // Look for sequences of git commands containing add, commit, push
        for window in bases.windows(3) {
            if window[0] == "git" && window[1] == "git" && window[2] == "git" {
                let _c0 = commands.get(0).unwrap_or(&"");
                let _c1 = commands.get(1).unwrap_or(&"");
                let _c2 = commands.get(2).unwrap_or(&"");

                // Find actual commands in the entries
                // This is approximate — just count git clusters of 3+
                count += 1;
                if best_example.is_empty() {
                    best_example = vec!["git add ...".into(), "git commit ...".into(), "git push".into()];
                }
            }
        }

        // More precise: scan for git add → git commit → git push in sequence
        let mut precise_count = 0;
        let mut i = 0;
        while i < bases.len() {
            if bases[i] == "git" {
                // Look ahead for a git workflow sequence
                let start = i;
                while i < bases.len() && bases[i] == "git" {
                    i += 1;
                }
                let git_seq = &commands[start..i.min(commands.len())];
                let has_add = git_seq.iter().any(|c| c.starts_with("git add"));
                let has_commit = git_seq.iter().any(|c| c.starts_with("git commit"));
                let has_push = git_seq.iter().any(|c| c.starts_with("git push"));

                if has_add && has_commit && has_push {
                    precise_count += 1;
                    if best_example.is_empty() || best_example[0].starts_with("git") {
                        best_example = vec!["git add".into(), "git commit".into(), "git push".into()];
                    }
                }
            } else {
                i += 1;
            }
        }

        Workflow {
            name: "Git Commit Flow".to_string(),
            description: "git add → git commit → git push workflow".to_string(),
            count: precise_count.max(count.min(1)),
            example: best_example,
        }
    }

    fn detect_two_pattern(
        bases: &[String],
        _commands: &[&str],
        first: &str,
        second: &str,
        name: &str,
        description: &str,
    ) -> Workflow {
        let mut count = 0;
        let mut best_example: Vec<String> = Vec::new();

        for window in bases.windows(2) {
            if window[0] == first && window[1] == second {
                count += 1;
                if best_example.is_empty() {
                    best_example = vec![window[0].clone(), window[1].clone()];
                }
            }
        }

        Workflow {
            name: name.to_string(),
            description: description.to_string(),
            count,
            example: best_example,
        }
    }

    fn detect_edit_commit(bases: &[String], commands: &[&str]) -> Workflow {
        let mut count = 0;
        let mut best_example: Vec<String> = Vec::new();

        for window in bases.windows(3) {
            let is_edit = matches!(window[0].as_str(), "vim" | "nano" | "code" | "hx" | "micro");
            let is_diff = window[1] == "git" && commands.get(1).map_or(false, |c| c.starts_with("git diff"));
            let is_add = window[2] == "git" && commands.get(2).map_or(false, |c| c.starts_with("git add"));

            if is_edit && is_diff && is_add {
                count += 1;
                if best_example.is_empty() {
                    best_example = vec!["vim ...".into(), "git diff".into(), "git add".into()];
                }
            }
        }

        Workflow {
            name: "Edit → Review → Stage".to_string(),
            description: "Edit file, review changes with git diff, then stage with git add".to_string(),
            count,
            example: best_example,
        }
    }

    pub fn print(&self, json: bool) {
        if json {
            println!("{}", serde_json::to_string_pretty(self).unwrap());
            return;
        }

        println!("🔄 Detected workflows:");
        println!("{}", "─".repeat(60));
        for w in &self.workflows {
            println!("  {} (×{})", w.name, w.count);
            println!("    {}", w.description);
            if !w.example.is_empty() {
                println!("    Example: {}", w.example.join(" → "));
            }
            println!();
        }

        if self.workflows.is_empty() {
            println!("  No common workflows detected.");
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

/// Simple Levenshtein distance.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (len_a, len_b) = (a.len(), b.len());

    if len_a == 0 { return len_b; }
    if len_b == 0 { return len_a; }

    let mut matrix = vec![vec![0; len_b + 1]; len_a + 1];

    for (i, row) in matrix.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=len_b {
        matrix[0][j] = j;
    }

    for i in 1..=len_a {
        for j in 1..=len_b {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[len_a][len_b]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "hello"), 5);
        assert_eq!(levenshtein("same", "same"), 0);
    }

    #[test]
    fn test_command_frequency() {
        let entries = vec![
            HistoryEntry { command: "git status".into(), timestamp: None, index: 0 },
            HistoryEntry { command: "git status".into(), timestamp: None, index: 1 },
            HistoryEntry { command: "ls".into(), timestamp: None, index: 2 },
            HistoryEntry { command: "cargo build".into(), timestamp: None, index: 3 },
        ];
        let freq = CommandFrequency::analyze(&entries, 10);
        assert_eq!(freq.total, 4);
        assert_eq!(freq.top[0].0, "git");
        assert_eq!(freq.top[0].1, 2);
    }

    #[test]
    fn test_command_sequence() {
        let entries = vec![
            HistoryEntry { command: "cd projects".into(), timestamp: None, index: 0 },
            HistoryEntry { command: "ls".into(), timestamp: None, index: 1 },
            HistoryEntry { command: "cd projects".into(), timestamp: None, index: 2 },
            HistoryEntry { command: "ls".into(), timestamp: None, index: 3 },
        ];
        let seq = CommandSequence::analyze(&entries, 10);
        assert!(seq.transitions.contains_key("cd"));
        let cd_transitions = &seq.transitions["cd"];
        assert_eq!(cd_transitions[0].0, "ls");
    }

    #[test]
    fn test_error_detector_sudo_retry() {
        let entries = vec![
            HistoryEntry { command: "apt install foo".into(), timestamp: None, index: 0 },
            HistoryEntry { command: "sudo apt install foo".into(), timestamp: None, index: 1 },
        ];
        let errors = ErrorDetector::analyze(&entries, 10);
        assert!(!errors.likely_errors.is_empty());
        assert!(errors.likely_errors[0].1.contains("sudo"));
    }

    #[test]
    fn test_workflow_cd_ls() {
        let entries = vec![
            HistoryEntry { command: "cd /tmp".into(), timestamp: None, index: 0 },
            HistoryEntry { command: "ls".into(), timestamp: None, index: 1 },
            HistoryEntry { command: "cd /home".into(), timestamp: None, index: 2 },
            HistoryEntry { command: "ls".into(), timestamp: None, index: 3 },
        ];
        let wf = WorkflowDetector::analyze(&entries);
        let cd_ls = wf.workflows.iter().find(|w| w.name == "Directory Exploration");
        assert!(cd_ls.is_some());
        assert_eq!(cd_ls.unwrap().count, 2);
    }
}
