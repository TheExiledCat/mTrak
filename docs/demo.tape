# Records docs/demo.gif. Run `cargo build --release`, then `vhs docs/demo.tape` from the
# repository root. The committed gif is re-encoded with a shared 64-color palette to keep it
# under 10 MB:
#   ffmpeg -i docs/demo.gif -vf "split[a][b];[a]palettegen=max_colors=64:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" demo.gif

Output docs/demo.gif

Set Shell bash
Set FontSize 12
Set Width 1280
Set Height 720
Set Padding 8
Set Framerate 15
Set TypingSpeed 35ms

# a throwaway home, so the demo doesn't touch the real config; mTrak writes the example
# themes into it on startup. VHS can't send Shift+Down, so the next instrument also gets Alt+N
Hide
Type `export DEMO=$(mktemp -d) && export HOME=$DEMO XDG_CONFIG_HOME=$DEMO/.config COLORTERM=truecolor`
Enter
Type `mkdir -p $XDG_CONFIG_HOME/mtrak && printf 'theme = "full"\n' > $XDG_CONFIG_HOME/mtrak/config.toml`
Enter
Type `printf 'instruments.next_instrument = "S-Dn A-n"\n' > $XDG_CONFIG_HOME/mtrak/keybinds.toml && clear`
Enter
Type "./target/release/mtrak --no-update-check"
Enter
Sleep 2s
# closes the MIDI input and output errors shown on machines without MIDI support; leave
# these out where MIDI works
Enter
Sleep 300ms
Enter
Sleep 500ms
Show

Sleep 750ms

# edit mode, kick on every beat (line add 4)
Space
Sleep 200ms
Type "]]]"
Sleep 200ms
Type "zzzzzzzzzzzzzzzz"
Sleep 300ms

# snare on the backbeat (line add 8, from row 4), then its instrument column
Tab
Type "]]]]"
Down 4
Type "xxxxxxxx"
Right
Type@20ms "11111111"
Up 4
Sleep 200ms

# hats on every other row (line add 2), with retrig rolls typed in place (line add 0),
# then their instrument column
Tab
Type "[[[[[["
Type "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn"
Type "[["
PageDown
Right 4
Type "r"
Right 2
Type "2"
PageDown
Up 4
Left 2
Type "r"
Right 2
Type "3"
Left 5
PageUp
PageUp
Type "]]"
Type@15ms "22222222222222222222222222222222"
Sleep 200ms

# bass line, down an octave with track transpose, then its instrument column
Tab
Type "zzbzvvbzzzbzvvbzzzbzvvbzvvzvbbmb"
Sleep 200ms
Alt+g
Alt+g
Alt+g
Alt+g
Alt+g
Alt+g
Alt+g
Alt+g
Alt+g
Alt+g
Alt+g
Alt+g
Sleep 300ms
Right
Type@15ms "33333333333333333333333333333333"
Sleep 200ms

# lead melody (line add 4), a mod wheel sweep typed in place (line add 0), then its
# instrument column
Tab
Type "]]"
Type "tewqteweytewqwet"
Sleep 200ms
Type "[[[["
Right 4
Type "w"
Down 4
Type "v"
PageDown
Up 4
Type "m"
Right
Type "f"
Right
Type "f"
Left 5
PageUp
Type "]]]]"
Type@20ms "4444444444444444"
Sleep 400ms

# play the song
Space
Enter
Sleep 1500ms

# while it plays, name the lead (instrument 4) and give it an arpeggiator: the root, a fifth,
# an octave and a fifth again, a 16th note each
Alt+n
Alt+n
Alt+n
Alt+n
Sleep 300ms
Ctrl+R
Sleep 400ms
Type "Lead"
Sleep 200ms
Tab 5
Sleep 200ms
Space
Sleep 400ms
Space
Sleep 400ms
Type "1"
Sleep 300ms
Enter
Sleep 400ms
Up
Sleep 200ms
Space
Sleep 400ms
Tab 2
Type "07"
Sleep 250ms
Tab
Type "0C"
Sleep 250ms
Tab
Type "07"
Sleep 1s
Escape
Sleep 300ms
Escape
Sleep 200ms
Escape
Sleep 1500ms

# name the song in the Info dialog, which also says how big and long it is
Ctrl+F
Sleep 300ms
Type "Demo song"
Sleep 1250ms
Escape
Sleep 500ms

# collapse the header for more rows, then scroll through the example themes in the Config
# dialog while it plays; each one previews as it's picked, and Esc puts back the theme there was
Ctrl+X
Sleep 1s
Ctrl+P
Sleep 300ms
Tab
Sleep 300ms
Down
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 800ms
Escape
Sleep 750ms

# expand the header again, stop, and look up the commands
Ctrl+X
Sleep 500ms
Space
Sleep 400ms
Type "/"
Sleep 1500ms
Escape
Sleep 500ms
