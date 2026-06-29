param(
  [Parameter(Mandatory = $true)]
  [string]$ArtifactPath
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $ArtifactPath)) {
  throw "Artifact not found: $ArtifactPath"
}

$signtool = if ($env:SIGNTOOL_EXE) {
  $env:SIGNTOOL_EXE
} else {
  (Get-Command signtool.exe -ErrorAction Stop).Source
}

$timestampUrl = if ($env:REMEMBER_TIMESTAMP_URL) {
  $env:REMEMBER_TIMESTAMP_URL
} else {
  "http://timestamp.digicert.com"
}

$args = @("sign", "/fd", "SHA256", "/tr", $timestampUrl, "/td", "SHA256")

if ($env:REMEMBER_SIGN_CERT_THUMBPRINT) {
  $args += @("/sha1", $env:REMEMBER_SIGN_CERT_THUMBPRINT)
} elseif ($env:REMEMBER_SIGN_PFX_PATH) {
  $args += @("/f", $env:REMEMBER_SIGN_PFX_PATH)
  if ($env:REMEMBER_SIGN_PFX_PASSWORD) {
    $args += @("/p", $env:REMEMBER_SIGN_PFX_PASSWORD)
  }
} else {
  throw "Set REMEMBER_SIGN_CERT_THUMBPRINT or REMEMBER_SIGN_PFX_PATH before signing."
}

$args += $ArtifactPath

& $signtool @args
if ($LASTEXITCODE -ne 0) {
  throw "signtool failed with exit code $LASTEXITCODE"
}

$signature = Get-AuthenticodeSignature -LiteralPath $ArtifactPath
if ($signature.Status -ne "Valid") {
  throw "Signed artifact did not validate: $ArtifactPath :: $($signature.Status)"
}

Write-Host "$ArtifactPath :: Valid"
