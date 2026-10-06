param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
try {
  # Only build and stage the runner here. Verification, formatting, catalogs,
  # packaging, and installer creation are ordinary repository tasks.
  $dotaskOldRustFlags = $env:RUSTFLAGS
  $env:RUSTFLAGS = '-C target-feature=+crt-static'
  try {
    & cargo build --manifest-path Cargo.toml --package dotask-cli --locked --release
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally { $env:RUSTFLAGS = $dotaskOldRustFlags }
  $dotaskCargoJson = (& cargo metadata --manifest-path Cargo.toml --locked --format-version 1 --no-deps | Out-String)
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $dotaskCargo = $dotaskCargoJson | ConvertFrom-Json
  $dotaskHostProject = Join-Path (Get-Location) 'src/Dotask/Dotask.csproj'
  $dotaskPublishText = (& dotnet publish $dotaskHostProject -c Release --self-contained=false -p:UseAppHost=false --nologo --verbosity quiet -getProperty:PublishDir,Version -getItem:ResolvedFileToPublish | Out-String)
  if ($LASTEXITCODE -ne 0) { Write-Output $dotaskPublishText; exit $LASTEXITCODE }
  $dotaskJsonStart = $dotaskPublishText.IndexOf('{')
  if ($dotaskJsonStart -lt 0) { throw 'MSBuild returned no evaluated publish metadata.' }
  if ($dotaskJsonStart -gt 0) { Write-Output $dotaskPublishText.Substring(0, $dotaskJsonStart).Trim() }
  $dotaskPublish = $dotaskPublishText.Substring($dotaskJsonStart) | ConvertFrom-Json
  if ([IO.Path]::IsPathRooted($dotaskPublish.Properties.PublishDir)) { $dotaskPublished = $dotaskPublish.Properties.PublishDir }
  else { $dotaskPublished = [IO.Path]::GetFullPath((Join-Path (Split-Path $dotaskHostProject) $dotaskPublish.Properties.PublishDir)) }
  New-Item -ItemType Directory -Path (Join-Path $Destination 'sdk/rust/src'), (Join-Path $Destination 'sdk/dotnet') -Force | Out-Null
  Copy-Item -LiteralPath (Join-Path $dotaskCargo.target_directory 'release/dotask.exe') -Destination $Destination
  Copy-Item -LiteralPath (Join-Path $dotaskPublished 'Dotask.dotnet.dll') -Destination (Join-Path $Destination 'sdk/dotnet')
  Copy-Item -LiteralPath 'src/dotask-sdk/Cargo.toml' -Destination (Join-Path $Destination 'sdk/rust')
  Copy-Item -LiteralPath 'src/dotask-sdk/src/lib.rs' -Destination (Join-Path $Destination 'sdk/rust/src')
} catch {
  Write-Error $_
  exit 1
}
