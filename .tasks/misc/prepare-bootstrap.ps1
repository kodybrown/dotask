param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
try {
  # Build the native runner and installer helper. Rust tasks use the source SDK;
  # the configured build-sdks task prepares the C# helper lazily when needed.
  $dotaskOldRustFlags = $env:RUSTFLAGS
  $env:RUSTFLAGS = '-C target-feature=+crt-static'
  try {
    & cargo build --manifest-path Cargo.toml --package dotask-cli --locked --profile bootstrap
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally { $env:RUSTFLAGS = $dotaskOldRustFlags }
  $dotaskCargoJson = (& cargo metadata --manifest-path Cargo.toml --locked --format-version 1 --no-deps | Out-String)
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $dotaskCargo = $dotaskCargoJson | ConvertFrom-Json
  Copy-Item -LiteralPath (Join-Path $dotaskCargo.target_directory 'bootstrap/dotask-doinstall-create-installer.exe') -Destination $Destination
  Copy-Item -LiteralPath (Join-Path $dotaskCargo.target_directory 'bootstrap/dotask.exe') -Destination $Destination
} catch { Write-Error $_; exit 1 }
