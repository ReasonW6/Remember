param()

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot

function Get-RememberInstallerPath {
  param([string]$ProjectRoot)

  $configPath = Join-Path $ProjectRoot "src-tauri\tauri.conf.json"
  $config = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
  $installerName = "{0}_{1}_x64-setup.exe" -f $config.productName, $config.version
  Join-Path $ProjectRoot (Join-Path "src-tauri\target\release\bundle\nsis" $installerName)
}

$targets = @(
  Join-Path $root "src-tauri\target\release\remember.exe"
  Get-RememberInstallerPath -ProjectRoot $root
)

foreach ($target in $targets) {
  & (Join-Path $PSScriptRoot "sign-windows-artifact.ps1") -ArtifactPath $target
}
