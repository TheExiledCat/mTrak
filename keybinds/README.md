# Terminal keybindings

Some terminals keep keys for themselves, so mTrak never sees them. Each file here rebinds the
actions those keys are for in one terminal. Copy it over `keybinds.toml` in mTrak's config
folder (see the Keybindings section of [docs/CONFIG.md](../docs/CONFIG.md)), or copy the lines
you need into the `keybinds.toml` you have. The comment at the top of each file says which
keys the terminal takes, and what to change in the terminal to get them back instead.

| File                    | Terminal                                         | Moves                                    |
| ----------------------- | ------------------------------------------------ | ---------------------------------------- |
| `gnome-terminal.toml`   | GNOME Terminal, Ptyxis, Tilix and other VTE ones | Row jumps on `F10` and `F11`             |
| `konsole.toml`          | Konsole                                          | Everything on `Shift` with an arrow      |
| `macos-terminal.toml`   | Terminal on macOS                                | Pattern keys on `Ctrl` with an arrow     |
| `tmux.toml`             | tmux                                             | The block loop on `Ctrl+B`               |
| `vscode.toml`           | The terminal in VS Code                          | The Config and Global dialogs            |
| `windows-terminal.toml` | Windows Terminal                                 | The row jump on `F11`                    |

Some keys can't be rebound, like `F1` to `F8` for the octave; the files say where a terminal
takes one of those.

If a key doesn't work in your terminal and there's no file for it here, rebind it on the
keybindings screen (`?`) and let us know which terminal and key it was.
