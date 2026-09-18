#!/usr/bin/env bash
set -euo pipefail

# Always use this checkout's tasks, even when called from another directory.
cd -- "$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
if ! command -v dotnet >/dev/null 2>&1; then
  echo "The .NET SDK is required. Install the SDK selected by global.json and retry." >&2
  exit 127
fi
if ! dotnet --version >/dev/null; then
  echo "A compatible .NET SDK is required. Check global.json and the installed SDKs." >&2
  exit 1
fi

dotask_bootstrap_dir="$(mktemp -d "${TMPDIR:-/tmp}/dotask-bootstrap.XXXXXXXX")"
trap 'rm -rf -- "$dotask_bootstrap_dir"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# MSBuild stages the evaluated publish output; no bin/obj paths are assumed.
dotnet publish src/Dotask.Cli/Dotask.Cli.csproj -c Release --nologo --verbosity minimal \
  "-p:DotaskBootstrapDirectory=$dotask_bootstrap_dir"
if [[ $# -eq 0 ]]; then
  set -- verify
fi
dotnet "$dotask_bootstrap_dir/dotask.dll" "$@"
