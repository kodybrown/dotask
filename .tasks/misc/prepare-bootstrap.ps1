param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
try {
  # Build only the native runner. Rust tasks use the source SDK; the configured
  # build-sdks task prepares the C# helper lazily when a C# task needs it.
  $dotaskOldRustFlags = $env:RUSTFLAGS
  $env:RUSTFLAGS = '-C target-feature=+crt-static'
  try {
    & cargo build --manifest-path Cargo.toml --package dotask-cli --locked --release
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally { $env:RUSTFLAGS = $dotaskOldRustFlags }
  $dotaskCargoJson = (& cargo metadata --manifest-path Cargo.toml --locked --format-version 1 --no-deps | Out-String)
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $dotaskCargo = $dotaskCargoJson | ConvertFrom-Json
  Copy-Item -LiteralPath (Join-Path $dotaskCargo.target_directory 'release/dotask.exe') -Destination $Destination
} catch { Write-Error $_; exit 1 }
