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

# Bootstrap only builds/stages the runner. All repository operations are tasks.
cargo build --manifest-path Cargo.toml --package dotask-cli --locked --release
dotask_cargo_json="$(cargo metadata --manifest-path Cargo.toml --locked --format-version 1 --no-deps)"
dotask_publish_json="$(dotnet publish src/Dotask.CSharpHost/Dotask.CSharpHost.csproj -c Release \
  --self-contained=false -p:UseAppHost=false --nologo --verbosity quiet -getProperty:PublishDir,Version -getItem:ResolvedFileToPublish)"
# Use the SDK already required for C# support to read tool metadata; no Python,
# jq, installed dotask, or task execution is part of the bootstrap.
dotask_metadata_reader="$dotask_bootstrap_dir/paths.cs"
cat > "$dotask_metadata_reader" <<'CS'
using System.Text.Json;
var text = File.ReadAllText(args[0]);
using var json = JsonDocument.Parse(text[text.IndexOf('{')..]);
if (args[1] == "publish") {
  var root = json.RootElement.GetProperty("Properties").GetProperty("PublishDir").GetString()!;
  foreach (var file in json.RootElement.GetProperty("Items").GetProperty("ResolvedFileToPublish").EnumerateArray()) {
    var relative = file.GetProperty("RelativePath").GetString()!;
    var destination = Path.Combine(args[2], relative);
    Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
    File.Copy(Path.Combine(root, relative), destination);
  }
}
Console.WriteLine(args[1] == "cargo" ? json.RootElement.GetProperty("target_directory").GetString()
  : json.RootElement.GetProperty("Properties").GetProperty("PublishDir").GetString());
CS
printf '%s' "$dotask_cargo_json" > "$dotask_bootstrap_dir/cargo.json"
printf '%s' "$dotask_publish_json" > "$dotask_bootstrap_dir/publish.json"
dotask_cargo_output="$(dotnet run --file "$dotask_metadata_reader" -- "$dotask_bootstrap_dir/cargo.json" cargo)"
dotask_publish_output="$(dotnet run --file "$dotask_metadata_reader" -- "$dotask_bootstrap_dir/publish.json" publish "$dotask_bootstrap_dir/csharp")"
mkdir -p "$dotask_bootstrap_dir/csharp" "$dotask_bootstrap_dir/sdk/src"
cp "$dotask_cargo_output/release/dotask" "$dotask_bootstrap_dir/dotask"
cp src/dotask-sdk/Cargo.toml "$dotask_bootstrap_dir/sdk/"
cp src/dotask-sdk/src/lib.rs "$dotask_bootstrap_dir/sdk/src/"
if [[ $# -eq 0 ]]; then
  set -- verify
fi
"$dotask_bootstrap_dir/dotask" "$@"
