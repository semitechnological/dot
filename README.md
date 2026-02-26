<div align="center">
    <h3>dot</h3>
    <p>A minimal AI agent that lives in your terminal</p>
    <br/>
    <br/>
</div>

A terminal-native AI coding agent with tool execution, MCP support, extensible hooks, and persistent conversations. Built in Rust with a TUI interface designed to stay out of your way.

## Features

- **Multi-Provider**: Supports Anthropic Claude, OpenAI, and any OpenAI-compatible endpoint
- **Tool Execution**: Built-in file operations, shell commands, and pattern search
- **Custom Tools**: Define shell-backed tools in config.toml
- **MCP Integration**: Connect any Model Context Protocol server for extensible tooling
- **Lifecycle Hooks**: 22 events with blocking/modifying support via shell commands
- **Slash Commands**: Built-in and custom `/commands` accessible from the TUI
- **Extension Packages**: Install extensions from git with `dot install <url>`
- **Persistent Sessions**: SQLite-backed conversation history, resumable across restarts
- **Agent Profiles**: Define custom agents with specific models, prompts, and tool sets
- **Skills System**: Discovers and loads skill definitions from configurable directories
- **Context-Aware**: Auto-loads project-level and global `AGENTS.md` instructions
- **Vim Keybindings**: Modal editing with full vim-style navigation

## Install

```bash
cargo install dot-ai
```

Or from source:

```bash
git clone https://github.com/plyght/dot.git
cd dot
cargo install --path .
```

## Setup

```bash
# Authenticate with a provider
dot login

# Or set environment variables
export ANTHROPIC_API_KEY="..."
export OPENAI_API_KEY="..."
```

## Usage

```bash
# Launch the TUI
dot

# Resume a previous session
dot -s <session-id>

# Show config paths and current settings
dot config
# List MCP servers and discovered tools
dot mcp

# List installed extensions
dot extensions

# Install an extension from git
dot install https://github.com/user/my-dot-extension.git

# Uninstall an extension
dot uninstall my-dot-extension

Inside the TUI: `i` to enter insert mode, `Enter` to send, `Esc` to return to normal mode, `Ctrl+C` to cancel a stream or quit.

## Configuration

All configuration lives in `~/.config/dot/config.toml`:

```toml
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"

[theme]
name = "dark"

[tui]
vim_mode = true

[context]
auto_load_global = true
auto_load_project = true
```

### MCP Servers

```toml
[mcp.filesystem]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
enabled = true
env = {}
timeout = 30
```

### Agent Profiles

```toml
[agents.reviewer]
description = "Code review agent"
model = "claude-sonnet-4-20250514"
system_prompt = "You are a thorough code reviewer."
enabled = true

[agents.reviewer.tools]
run_command = false
```

### Custom Providers

```toml
[providers.my-proxy]
api = "openai"
base_url = "https://proxy.example.com/v1"
api_key_env = "MY_API_KEY"
models = ["gpt-4o"]
default_model = "gpt-4o"
```

### Custom Tools

```toml
[custom_tools.deploy]
description = "Deploy to production"
command = "scripts/deploy.sh"
schema = { type = "object", properties = { env = { type = "string" } } }
timeout = 60
```

### Slash Commands

```toml
[commands.review]
description = "Run code review"
command = "scripts/review.sh"
timeout = 30
```

### Lifecycle Hooks

```toml
[hooks.before_tool_call]
command = "scripts/notify.sh"
timeout = 10

[hooks.after_prompt]
command = "scripts/log.sh"
```

Available events: `session_start`, `session_end`, `before_prompt`, `after_prompt`, `before_tool_call`, `after_tool_call`, `before_compact`, `after_compact`, `model_switch`, `agent_switch`, `on_error`, `on_stream_start`, `on_stream_end`, `on_resume`, `on_user_input`, `on_tool_error`, `before_exit`, `on_thinking_start`, `on_thinking_end`, `on_title_generated`, `before_permission_check`, `on_context_load`.

`before_*` hooks can block (exit non-zero) or modify (stdout replaces input). Other hooks are fire-and-forget.

### Extension Packages

Extensions live in `~/.config/dot/extensions/` with an `extension.toml` manifest:

```toml
name = "my-extension"
description = "An example extension"
version = "0.1.0"

[tools.lint]
description = "Run linter"
command = "scripts/lint.sh"

[commands.deploy]
description = "Deploy"
command = "scripts/deploy.sh"

[hooks.after_tool_call]
command = "scripts/notify.sh"
```

## Architecture

```
src/
  main.rs          CLI entry point and provider/tool wiring
  config.rs        TOML configuration loading
  extension.rs     Extension trait, hooks, lifecycle events, script tools
  command.rs       Slash command registry
  packages.rs      Extension package discovery, install, uninstall
  context.rs       AGENTS.md discovery and injection
  mcp.rs           MCP client (stdio transport, JSON-RPC)
  skills.rs        Skill discovery and loading
  agent/           Conversation loop, profiles, event types, hook integration
  provider/        Provider trait + Anthropic and OpenAI implementations
  tools/           Tool trait, file operations, shell execution
  tui/             Ratatui-based interface, input handling, markdown rendering
  auth/            OAuth and API key credential management
  db/              SQLite session and message persistence
```

## Development

```bash
cargo build
cargo test
```

Requires Rust nightly (edition 2024). Key dependencies: ratatui, crossterm, tokio, clap, reqwest, rusqlite, async-openai, syntect.

## License

MIT
