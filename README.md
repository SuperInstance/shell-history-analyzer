# 🔍 Shell History Analyzer

A Rust CLI tool that analyzes your shell history (`~/.bash_history`, `~/.zsh_history`) to discover patterns, workflows, and habits.

## Features

| Module | Description |
|--------|-------------|
| **HistoryParser** | Parses bash and zsh history files with timestamp support |
| **CommandFrequency** | Top-N most used commands with percentages and categories |
| **CommandSequence** | Markov chain analysis — what command follows what? |
| **TimePatterns** | Time-of-day analysis (morning vs night, peak hours, night owl detection) |
| **ErrorDetector** | Identifies commands that likely failed (retried with sudo, typo corrections) |
| **WorkflowDetector** | Detects common workflows (edit→build→test, cd→ls exploration, git commit flow) |

## Installation

```bash
git clone https://github.com/SuperInstance/shell-history-analyzer.git
cd shell-history-analyzer
cargo install --path .
```

## Usage

```bash
# Full analysis report
shell-history-analyzer report

# Individual analyses
shell-history-analyzer frequency          # Top-N commands
shell-history-analyzer sequence           # Command transitions
shell-history-analyzer sequence -c git    # What follows "git"?
shell-history-analyzer time               # Time-of-day patterns
shell-history-analyzer errors             # Likely failed commands
shell-history-analyzer workflows          # Detected workflows

# Options
shell-history-analyzer report --top 30    # Show top 30 results
shell-history-analyzer report --json      # JSON output
shell-history-analyzer report --file /path/to/history  # Custom history file
```

## Real Analysis Results

Run against a real `~/.bash_history` (1,109 entries from a development machine).

### Top 15 Commands

```
#    Command                        Count           %
1    openclaw                       88         8.0%
2    export                         85         7.7%
3    claude                         68         6.1%
4    cd                             62         5.6%
5    kimi                           42         3.8%
6    apt-get                        26         2.4%
7    apt                            24         2.2%
8    npm                            23         2.1%
9    curl                           18         1.6%
10   rm                             18         1.6%
11   telnet                         17         1.5%
12   kimi-cli                       17         1.5%
13   exit                           15         1.4%
14   grok                           11         1.0%
15   docker                         11         1.0%
```

**Insight:** This is an AI-heavy development environment — `openclaw`, `claude`, and `kimi` (AI coding assistants) account for ~18% of all commands. Package management (`apt`, `apt-get`, `npm`) is the second biggest category.

### Command Sequences (What follows `openclaw`?)

```
What follows `openclaw`:
  1    openclaw                       48      54.5%   # Repeated openclaw commands
  2    exit                           8        9.1%   # Exit after openclaw session
  3    nano                           6        6.8%   # Edit config after changes
  4    npm                            3        3.4%   # Node package management
  5    claude                         2        2.3%   # Switch to Claude
```

**Insight:** OpenClaw commands tend to cluster — 54.5% of the time, another `openclaw` command follows. This suggests interactive configuration sessions.

### What follows `cd`?

```
What follows `cd`:
  1    cd                             18      29.0%   # Continue navigating
  2    claude                         14      22.6%   # Launch Claude in directory
  3    kimi-cli                       4        6.5%   # Launch Kimi CLI
  4    kimi                           4        6.5%   # Launch Kimi
  5    ls                             2        3.2%   # List directory contents
```

**Insight:** The dominant pattern is `cd` → `claude` (22.6%), showing a workflow of navigating to a project then immediately launching an AI assistant.

### Likely Errors

```
  1. cd cocpn → cd cocapn             # Typo correction (edit dist=1)
  2. wrangler login → wrangler whoami  # Different approach (edit dist=5)
  3. docker system df → sudo docker system df   # Permission denied, retried with sudo
  4. telnet 147.224.38.131 4040 → telnet 147.224.38.131 7777  # Wrong port, tried another
  5. openclaw gateway start → openclaw gateway restart  # start failed, tried restart
```

**Insight:** The error detector correctly identified:
- Typos in directory names (`cocpn` → `cocapn`)
- Permission issues requiring `sudo`
- Network connections to wrong ports
- Subcommand mistakes (`start` vs `restart`)

### Detected Workflows

```
Docker Build & Run (×8)
  Docker container workflow
  Example: docker → docker

Directory Exploration (×2)
  cd followed by ls — browsing the filesystem
  Example: cd → ls
```

**Insight:** Docker workflows are the most common multi-step pattern (build → run → inspect). The low count of `cd → ls` suggests this user navigates directly without browsing.

## Architecture

```
src/
├── main.rs          # CLI entry point with clap
├── parser.rs        # HistoryParser: bash/zsh parsing, command classification
└── analysis.rs      # All analysis modules (frequency, sequence, time, errors, workflows)
```

### Supported History Formats

- **Bash** (`~/.bash_history`): Plain lines, `#timestamp` prefixed (HISTTIMEFORMAT), multi-line with `\`
- **Zsh** (`~/.zsh_history`): Extended format `: timestamp:elapsed;command` with escape handling

### Command Classification

Commands are automatically categorized:
- 🔀 VCS (git)
- 🔨 Build (cargo, make, npm, go, gcc)
- ✏️ Editor (vim, nano, code, hx)
- 🔍 Navigate (cd, ls, find, grep, rg)
- 🌐 Network (ssh, curl, wget, ping)
- 🐳 Containers (docker, kubectl, podman)
- 🐚 Shell (export, source, alias)
- 📦 Packages (apt, brew, pip)
- And more...

## Development

```bash
cargo build
cargo test    # 19 tests
```

## License

MIT
