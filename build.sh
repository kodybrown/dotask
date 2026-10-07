#!/usr/bin/env bash
set -euo pipefail

# Always use this checkout's tasks, even when called from another directory.
cd -- "$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
dotask_bootstrap_dir="$(mktemp -d "${TMPDIR:-/tmp}/dotask-bootstrap.XXXXXXXX")"
trap 'rm -rf -- "$dotask_bootstrap_dir"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Cargo locates its output. The runner stages itself and exits before tasks can
# rebuild it, so neither a JSON parser nor a .NET toolchain is needed here.
cargo run --manifest-path Cargo.toml --package dotask-cli --locked --release -- __stage-runner "$dotask_bootstrap_dir"
export DOTASK_SOURCE_RUST_SDK="$PWD/src/dotask-sdk"
export DOTASK_SOURCE_SDK_STAGE="$dotask_bootstrap_dir/sdk"
"$dotask_bootstrap_dir/dotask" "$@"
