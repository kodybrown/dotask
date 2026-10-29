#!/usr/bin/env bash
set -euo pipefail

# Always use this checkout's tasks, even when called from another directory.
cd -- "$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# Child tools and nested launchers share a protected scratch base. This changes
# only this process tree, leaving the user's persistent temp settings untouched.
export DOTASK_DEV_TEMP_ROOT="${DOTASK_DEV_TEMP_ROOT:-${TMPDIR:-/tmp}/_rust}"
mkdir -p -- "$DOTASK_DEV_TEMP_ROOT"
export TMPDIR="$DOTASK_DEV_TEMP_ROOT"

dotask_bootstrap_dir="$(mktemp -d "${TMPDIR:-/tmp}/dotask-bootstrap.XXXXXXXX")"
trap 'rm -rf -- "$dotask_bootstrap_dir"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Build both native executables before staging; consumers never compile helpers.
cargo build --manifest-path Cargo.toml --package dotask-cli --locked --profile bootstrap

# Cargo locates its output. The runner stages itself and exits before tasks can
# rebuild it, so neither a JSON parser nor a .NET toolchain is needed here.
cargo run --manifest-path Cargo.toml --package dotask-cli --bin dotask --locked --profile bootstrap -- __stage-runner "$dotask_bootstrap_dir"
export DOTASK_SOURCE_RUST_SDK="$PWD/src/dotask-sdk"
export DOTASK_SOURCE_ROOT="$PWD"
export DOTASK_SOURCE_SDK_STAGE="$dotask_bootstrap_dir/sdk"
"$dotask_bootstrap_dir/dotask" "$@"
