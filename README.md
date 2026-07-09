# tmenu

![tmenu](https://github.com/pmoracho/tmenu/raw/main/images/tmenu.svg)

**tmenu** is a small, fast, keyboard-driven menu launcher for your terminal, written in Rust.

Define a menu tree in a plain-text `.toon` config file, and tmenu turns it into a navigable, searchable launcher for your shell commands — no more digging through `history` or aliases you forgot the name of.

📖 **[Full usage tutorial](tutorial/tutorial.md)** (with screenshots)

## Features

- **Nested submenus** with a breadcrumb trail, so you always know where you are
- **Fuzzy, recursive search** (`Tab`) — matches across all submenus at once, not just the current level
- **Command preview** (`F2`) — see the exact command that will run before you hit Enter
- **Interpolated parameters** — commands can prompt for input via `{{text: Label}}`, with a step-by-step wizard when a command needs several values
- **Confirmation dialogs** — tag any command with `[confirm=true]` to get a Yes/No prompt before it runs (useful for destructive commands)
- **Clean execution mode** — optionally wipe the screen before showing command output
- Built-in help screen (`F1`)
- Fully keyboard-driven, single static binary, no runtime dependencies

## Requirements

- Rust (stable toolchain) and `cargo`

## Installation

Clone the repository and build it:

```bash
git clone https://github.com/pmoracho/tmenu.git
cd tmenu

# Debug build
cargo build

# Release (optimized) build
cargo build --release
```

## Usage

```bash
# Run in debug mode
cargo run -- <options>

# Run the release binary directly
./target/release/tmenu

# Use the default config file (tmenu.toon in the current directory)
tmenu

# Or point to a specific file
tmenu my-project.toon
```

## Configuration file (`.toon`)

The whole menu structure lives in a plain-text file with a `.toon` extension:

```toon
# Example menu for a project with Git and Docker commands
config:
    execution_mode: clean
"My project":
    Git:
        Status:       "git status"
        Pull:         "git pull"
        Push:         "git push"
        "New branch": "git checkout -b {{text: Branch name}}"
    Docker:
        Up:   "docker compose up -d"
        Down: "docker compose down"
        Logs: "docker compose logs -f"
    Exit: exit
```

**Basic rules:**

- The first line ending in `:` is the menu **title**.
- Entries with no value after the `:` are **submenus**.
- Entries with a value are **commands**.
- `{{text: Label}}` defines a **parameter** the user will be prompted for before the command runs.
- Lines starting with `#` are comments and are ignored.
- The file must be UTF-8 without BOM.
- Don't expect full flexibility from the `.toon` format — the parser is intentionally simple.

## Keyboard shortcuts

| Key           | Context     | Action                              |
| ------------- | ----------- | ------------------------------------ |
| `↑` / `↓`     | Navigation  | Move between items                   |
| `Enter` / `→` | Navigation  | Select item / enter submenu          |
| `Esc` / `←`   | Navigation  | Go back one level                    |
| `Esc`         | Root menu   | **Quit the application**             |
| `Home`        | Navigation  | Jump back to the root menu           |
| `Tab`         | Navigation  | Enter search mode                    |
| `Tab` / `Esc` | Search      | Exit search mode                     |
| `Enter`       | Search      | Run the first matching result        |
| `F2`          | Anywhere    | Toggle command preview               |
| `F1`          | Anywhere    | Open/close help                      |
| `Ctrl+Q`      | Anywhere    | Quit the application                 |
| `Enter`       | Wizard      | Confirm the current field            |
| `Esc`         | Wizard      | Cancel and return to the menu        |
| `Ctrl+Q`      | Wizard      | Cancel and quit the application      |

## Advanced configuration

**Clear the screen before running a command**

Add an `execution_mode: clean` option to the `config` section. By default tmenu shows a command's output below the menu; with this option the screen is cleared first, which is useful for commands with verbose output or that need the full screen:

```toon
config:
    execution_mode: clean
```

**Require confirmation for specific commands**

Tag any command with `[confirm=true]` to show a Yes/No confirmation popup before it runs — a good way to avoid triggering sensitive commands by accident:

```toon
"Staging and Commits":
  "Add all": git add . [confirm=true]
```

## Contributing

Contributions are welcome! Please open an issue to discuss significant changes, and submit pull requests for improvements or fixes.

## License

*(No license file yet — add the one you prefer, e.g. MIT or Apache-2.0.)*