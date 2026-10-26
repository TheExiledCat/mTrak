# Configuration

mTrak's settings and themes live in an `mtrak` folder in your config folder, which is created
on startup if it doesn't exist yet:

| Platform | Config folder                         |
| -------- | ------------------------------------- |
| Linux    | `~/.config/mtrak`                     |
| macOS    | `~/Library/Application Support/mtrak` |
| Windows  | `%LOCALAPPDATA%\mtrak`                |

On Linux it follows `XDG_CONFIG_HOME`, if that's set.

The config folder holds `config.toml` and a `themes` folder. `config.toml` is written with its
defaults when it's missing, so deleting it resets it. The `themes` folder always holds the
[example themes](https://github.com/TheExiledCat/mTrak/tree/main/example_themes): any that are missing are written again on startup, so
deleting one only lasts until the next start, and editing one keeps your changes. A
`theme.toml` from before v0.4.2 is moved to `themes/custom.toml` on startup, so `theme =
"custom"` keeps working. The files use a small part of TOML: one `key = "value"` or `key =
true` per line, with `#` starting a comment. An error in a file is shown when mTrak starts,
with the line it's on, and the defaults are used instead.

## config.toml

Settings that aren't part of a project.

| Setting        | Values                                                  | Default | Description                                                     |
| -------------- | ------------------------------------------------------- | ------- | --------------------------------------------------------------- |
| `theme`        | `auto`, `mono`, `ansi16`, `full` or a theme's name      | `auto`  | The theme to open with                                          |
| `ascii_mode`   | `true` or `false`                                       | `false` | Draw with ASCII characters only, for fonts without box drawing  |
| `update_check` | `true` or `false`                                       | `true`  | Look for a new version on startup                               |
| `midi_input`   | A port name, or `""` for none                           | `""`    | The MIDI input a project connects to when it doesn't name one   |
| `midi_output`  | A port name, or `""` for none                           | `""`    | The MIDI output a project connects to when it doesn't name one  |
| `line_numbers` | `left`, `right`, `both` or `none`                       | `left`  | Which sides of the pattern editor show the row numbers; `none` gives the room to the tracks |
| `hex_line_numbers` | `true` or `false`                                   | `true`  | Row numbers in hex, or in decimal from `00` to `99` and round again, so they stay two digits |
| `show_dashes`  | `true` or `false`                                       | `true`  | Dashes in empty instrument and volume columns; blank when `false` |
| `track_numbers` | `true` or `false`                                      | `true`  | The header with the track numbers above the pattern editor; hiding it gives two more rows |

The themes:

| Theme    | Description                                                                      |
| -------- | -------------------------------------------------------------------------------- |
| `auto`   | Picks one from what the terminal supports: `full`, `ansi16` or `mono`            |
| `mono`   | No colors, only bold, dim, underline and reverse                                 |
| `ansi16` | The 16 standard terminal colors, so it follows your terminal's color scheme      |
| `full`   | The FT2 theme, for terminals with 24-bit color                                   |
| a name   | The colors in that file in the `themes` folder, e.g. `dracula` for `dracula.toml` |

The command line overrides these for one session: `--color` (or `--theme`) picks the theme,
`--ascii-mode` turns ASCII mode on and `--no-update-check` skips the update check. `NO_COLOR`
or `TERM=dumb` select `mono` when `theme` is `auto`.

## The Config dialog

`Ctrl+P` opens the Config dialog. It shows the settings in use, including any command line
options, and every change previews straight away; `Esc` puts back what was there. Confirm
with `Save to config.toml` ticked to keep the settings for next time, or unticked to use them
for this session only. Confirming default MIDI ports also connects them if the open project
has none. The update check takes effect on the next start. `Open config folder` opens the
config folder in your file manager, to get at the files directly.

The dialog can also edit the colors of the selected theme, previewing every change:

- `Color` picks one of the eleven colors, and below it the red, green and blue values
  (`Up`/`Down` by 1, `Left`/`Right` by 16) and the hex field change it.
- `Reset color` puts back the color the theme had when you picked it, and `Reset all` every
  color.
- `Save theme as` saves the colors to that name in the `themes` folder and switches to it,
  replacing a theme of that name. Without a name, edits to a theme from the `themes` folder
  are saved back to its file. The built-in themes have no file, so editing one fills in
  `custom` as the name; change it to keep your colors under another name.

## Theme files

Each file in the `themes` folder lists the colors of a theme, and its name without `.toml` is
the theme's name. Names use letters, digits, `-` and `_`. Every part of mTrak is drawn with
these eleven colors:

| Color          | Used for                                                                      |
| -------------- | ----------------------------------------------------------------------------- |
| `backdrop`     | The background around the panels, the cursor row bar and subtle borders       |
| `muted`        | Toned-down text and borders, and the fill behind the selection                |
| `disabled`     | Text and key fills for something unavailable, e.g. undo with nothing to undo  |
| `surface`      | The pattern editor's background, and the text on filled labels                |
| `surface_text` | Text in the pattern editor                                                    |
| `text`         | Text on the backdrop, and the fill under the cursor                           |
| `accent`       | Text that stands out: every 4th row number, the logo and section titles       |
| `danger`       | Errors, edit mode and recording                                               |
| `warning`      | Warnings, and the playing row while the row is locked                         |
| `success`      | Success, e.g. the About dialog's border                                       |
| `mark`         | The fill behind a marked block                                                |

Colors are hex (`"#49688C"`) or ANSI color names (`"Red"`, `"DarkGray"`, `"LightBlue"`, ...).
Colors you leave out come from the palette named by `base`, `"ft2"` (the default) or
`"ansi16"`, so a theme can change just a few:

```toml
base = "ansi16"
danger = "#FF5555"
mark = "#44475A"
```

The `themes` folder holds ports of Catppuccin, Dracula, Gruvbox, Nord, One Dark, Solarized
and Tokyo Night, from the [example_themes](https://github.com/TheExiledCat/mTrak/tree/main/example_themes) folder. To get one back as it came,
delete its file and start mTrak again.
