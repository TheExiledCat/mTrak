---
name: create-release
description: Create a new mTrak release for a given version (e.g. "0.0.2" or "v0.0.2"). Bumps the crate version, writes a detailed changelog entry in docs/CHANGELOG.md covering everything since the previous release, commits, tags and pushes so the release workflow builds and publishes it. Use when asked to "create a release", "release vX.Y.Z", "cut a release" or "publish a new version".
argument-hint: <version>
---

# Create release

Make release `v<version>` of mTrak. The argument is the version; accept it with or without a
leading `v` and normalize: the tag is `vX.Y.Z`, the crate version is `X.Y.Z`. If no version was
given, ask for one.

Pushing a tag publishes a GitHub release, so nothing is pushed until the user has approved the
changelog (step 4).

## 1. Check the state

- `git fetch --tags origin`, then make sure the tag doesn't already exist locally or on the
  remote. If it does, stop and ask; never move or overwrite a release tag unless the user says
  so explicitly.
- Must be on `main` and level with or ahead of `origin/main`. If behind, stop and ask.
- The version must be higher than the previous release tag (semver). If not, stop and ask.
- If `git status` shows uncommitted changes, ask whether they belong in the release. Never
  commit them without that answer; never stash or discard them.

## 2. Collect what changed

Previous release: `git describe --tags --abbrev=0 --match 'v*'` (if there is none, this is the
first release and the range is the whole history).

Read all of this, not just commit messages: commit messages in this repo are often short or
vague ("themefixes", "0.0.1redo"), so the diff is the source of truth.

- `git log --reverse --format='%h %s%n%b' <prev>..HEAD`
- `git diff --stat <prev>..HEAD`
- `git diff <prev>..HEAD -- src docs README.md Cargo.toml` and read the hunks. Open the changed
  files where the diff alone doesn't make the behavior clear.
- `docs/COMMANDS.md` diff for new or changed pattern commands, and the README roadmap diff for
  items that were ticked off.
- `Cargo.toml` dependency changes.

Work out what changed for someone using mTrak: new features, changed behavior, keys and
commands, file format changes, fixes. Internal refactors only get a line if they change
something users notice (performance, binary size, stability).

Check every claim against the code. Something whose implementation is `todo!()` or otherwise
unfinished is not a feature; leave it out, or list it under known issues if a user could
run into it (a `todo!()` reached during playback crashes the app).

## 3. Write the changelog entry

`docs/CHANGELOG.md`, newest release first. Create it if missing with a
`# Changelog` heading. The release workflow takes the release notes from this file by finding
the line `## vX.Y.Z` and reading up to the next `## ` heading, so the entry heading must be
exactly:

```
## vX.Y.Z - YYYY-MM-DD
```

Use today's date. Under it, use `###` sections, only the ones that have entries, in this order:

- `### Added`: new features, commands, keys, options
- `### Changed`: behavior that works differently now
- `### Fixed`: bugs fixed
- `### Removed`
- `### Known issues`

Write for users, in plain words: what they can do now and how (the key, the command syntax,
where in the UI), not which function changed. One bullet per change, more detail for bigger
features: a short example (like a command cell `---|1|--|F8C`) is good where it helps. Link to
`docs/COMMANDS.md` for full command details rather than copying them. Don't mention commit
hashes, internal type names or file paths, except docs a user would open.

If the previous release has no entry in the file, don't backfill it unless asked.

## 4. Get approval

Show the user the full changelog entry and summarize what will happen: version bump, the
commit, the tag, and that pushing publishes a release. Wait for approval and apply any edits
they ask for.

## 5. Bump, commit, tag, push

- Set `version = "X.Y.Z"` in `Cargo.toml` under `[package]`. Run a plain `cargo build` so
  `Cargo.lock` picks up the new version (CI builds with `--locked`, which fails on a stale
  lock), then `cargo build --release --locked` to make sure it builds. If it doesn't build,
  stop and report; don't tag broken code.
- Stage `Cargo.toml`, `Cargo.lock`, `docs/CHANGELOG.md` and whatever the user agreed to include.
- Commit with the single-line message `vX.Y.Z` and nothing else: no body, no co-author
  trailer. The author is the repo's configured git user.
- `git tag vX.Y.Z`, then `git push origin main` and `git push origin vX.Y.Z`.

## 6. Report

Say what was pushed (commit hash, tag) and link the run and the release:
`https://github.com/TheExiledCat/mTrak/actions` and
`https://github.com/TheExiledCat/mTrak/releases/tag/vX.Y.Z`. `gh` is not installed, so you
can't watch the run; tell the user to check it.
