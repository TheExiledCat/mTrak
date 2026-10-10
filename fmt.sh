#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

cargo clippy --fix --allow-dirty --allow-staged --all-targets --quiet

find src -name '*.rs' -exec perl -pi -e 's/^(\s*)Self::new\(\)$/$1return Self::new();/' {} +

if rustup run nightly rustfmt --version >/dev/null 2>&1; then
    cargo +nightly fmt -- --config group_imports=StdExternalCrate,imports_granularity=Crate
else
    echo "nightly rustfmt not found, imports won't be grouped (rustup component add rustfmt --toolchain nightly)" >&2
    cargo fmt
fi

cargo clippy --all-targets --quiet
