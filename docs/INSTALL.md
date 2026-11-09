# Installing mTrak

## From a release

Download the binary for your platform from the
[releases page](https://github.com/TheExiledCat/mTrak/releases) and put it somewhere on
your `PATH` or run it directly.

## From source

You need Rust 1.88 or newer (install it with [rustup](https://rustup.rs)).
On Linux you also need the ALSA development headers, e.g. `libasound2-dev` on
Debian/Ubuntu or `alsa-lib-devel` on Fedora.

```sh
cargo install --git https://github.com/TheExiledCat/mTrak
```

Or from a local clone:

```sh
git clone https://github.com/TheExiledCat/mTrak
cd mTrak
cargo install --path .
```

**Note**: v0.1.3 to v0.1.5 could be flagged by some antivirus software (especially Windows
Defender) as a false positive because of how the self updater replaced the executable. v0.1.6
changes how updates are installed on Windows to avoid this, but the release binaries are not
signed yet, so warnings can still show up. Once the project gets enough adoption, I will apply
for signing credentials. Until then, building from source with `cargo install` avoids the
issue.
