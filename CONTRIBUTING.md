# Contributing to mTrak

Thanks for your interest in mTrak. Bug reports, feature ideas and pull requests are all
welcome.

## Reporting bugs

Before opening an issue, search the [existing issues](https://github.com/TheExiledCat/mTrak/issues)
to see if it has already been reported.

A good bug report includes:

- What you did, step by step, and the keys you pressed
- What you expected to happen and what happened instead
- Your OS, terminal emulator and mTrak version (`mtrak --version`)
- The MIDI device or synth you were sending to, if the bug is about playback
- A screenshot or the project file, if it helps show the problem

Use the bug report template when opening the issue; it asks for all of the above.

## Suggesting features

Open an issue with the feature request template. Describe the problem you want solved
and how you would expect it to work. If FastTracker 2 or another tracker already does it,
mention how, since mTrak follows FT2 where it makes sense.

## Pull requests

For anything larger than a small fix, open an issue first so the approach can be agreed on
before you spend time on it.

1. Fork the repository and create a branch from `main`.
2. Make your change, keeping it focused on one thing.
3. Run the checks below and make sure they pass.
4. Open a pull request describing what changed and why, and link the related issue.

### Checks

```sh
cargo fmt
cargo test
```

### Code style

- Format with `rustfmt` (`cargo fmt`).
- Use explicit `return` statements, like the rest of the codebase.
- Items are either `pub` or private; don't use `pub(crate)` or `pub(super)`.
- Bind keys through a `Keymap` so the footer hints stay in sync, and put the most used
  bindings first, since that is the order the hints are shown in.
- Take colors and styles from `Theme` instead of hard-coding them, so the monochrome theme
  keeps working.
- Add tests for engine and data changes where you can.
- If you add or change a pattern command, update [docs/COMMANDS.md](docs/COMMANDS.md).
