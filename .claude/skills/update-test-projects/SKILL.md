---
name: update-test-projects
description: Keep the saved test projects in src/data/testdata (one .mtrak file per file version) in line with the code, so the tests prove every file version still loads and that projects are saved exactly as the current version's file. Use when the test saving_writes_the_current_versions_test_project_byte_for_byte or the_test_project_of_every_version_loads fails, after changing anything that's saved in a project, after raising PROJECT_VERSION, or when asked to "update the test projects".
---

# Update test projects

`src/data/testdata/v<N>.mtrak` holds the test project saved by file version N, for every version
from `FIRST_MESSAGEPACK_VERSION` up to `PROJECT_VERSION` (both in `src/data/migration.rs`). The
tests in `src/data/project.rs` read them from disk, so they're never built into mTrak itself:

- `the_test_project_of_every_version_loads` loads each file, through the migrations for older
  versions, and checks it with `assert_test_project`. A missing file fails it.
- `saving_writes_the_current_versions_test_project_byte_for_byte` saves `test_project()` and
  compares it with the current version's file, so any change to how projects are saved fails it.

Files are written by `test_project()` through the ignored test `write_test_project`:

```
cargo test write_test_project -- --ignored
```

It writes the file of the version the code is at. Never edit a `.mtrak` file by hand.

The goal is that the files for the current version and the version before it are present and
right, without ever rewriting a version that's released.

## 1. Find out where things stand

- The current version: `PROJECT_VERSION` in `src/data/migration.rs`.
- Which versions are released: for each release tag, the `PROJECT_VERSION` it shipped:
  `git show <tag>:src/data/migration.rs | grep "pub const PROJECT_VERSION"` (tags from before
  v0.6.0 don't have the file; they wrote versions 1 and 2, which have no test projects).
- Which files exist in `src/data/testdata`, and what `cargo test project::tests` says.

## 2. The current version's file

- **The current version isn't released yet:** write its file with `write_test_project`. This is
  also how to accept a format change made before the release.
- **The current version is released and the byte-for-byte test fails:** the code saves projects
  differently from files that exist. Don't write the file again. Either undo the change, or make
  it a new version: raise `PROJECT_VERSION`, add a migration for the old version to
  `MIGRATIONS`, then write the new version's file. Ask the user which before doing either.

## 3. The previous version's file

After `PROJECT_VERSION` goes up, the file of the version before it must stay as it was: it's
what files saved by that version look like, and loading it tests the new migration.

If it's missing, write it with the code of the last release that saved that version, in a
separate worktree so the working tree isn't touched:

```
git worktree add <scratchpad>/old-mtrak <tag>
cd <scratchpad>/old-mtrak && CARGO_TARGET_DIR=<scratchpad>/old-mtrak-target cargo test write_test_project -- --ignored
cp <scratchpad>/old-mtrak/src/data/testdata/v<N>.mtrak <repo>/src/data/testdata/
git worktree remove --force <scratchpad>/old-mtrak
```

If that release doesn't have `write_test_project`, say so and stop; don't make the file with the
current code, as that wouldn't be the old format.

## 4. The test project itself

- When a saved field is added, set it away from its default in `test_project()`, so the field is
  saved and compared, and write the current version's file again (step 2).
- `assert_test_project` must pass for the file of every version. Only check a new field there if
  the migrations give older files the same value; otherwise leave it to the byte-for-byte test.
- Don't change what `test_project()` puts in fields that older files already hold: their files
  can't change, so they'd stop matching `assert_test_project`.

## 5. Verify

- `cargo test` passes, with `write_test_project` as the only ignored test.
- `git status` shows only the `.mtrak` files you meant to add or change, and no released version's
  file is modified.
- If the format changed, run the `update-file-format` skill too.

Finish with which files were written or kept, from which code, and anything that needs the
user's call, such as a released version that's saved differently now.
