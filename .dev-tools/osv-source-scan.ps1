$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
$gradleLock = Join-Path $repoRoot "crates/envryn-android-clipboard/android/gradle.lockfile"
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$scanDir = Join-Path $tempRoot ("envryn-osv-" + [guid]::NewGuid().ToString("N"))
$runtimeLock = Join-Path $scanDir "gradle.lockfile"

New-Item -ItemType Directory -Path $scanDir | Out-Null
try {
  # Gradle's lock also contains Android Gradle Plugin emulator/test-host
  # configurations. They run on the build machine and are never packaged in
  # Envryn. OSV cannot interpret Gradle configuration reachability itself, so
  # give it a lock containing only the release runtime that ships in the APK.
  $runtimeLines = Get-Content -LiteralPath $gradleLock | Where-Object {
    $_.StartsWith("#") -or
    $_ -eq "empty=" -or
    $_ -match "(?:^|,)releaseRuntimeClasspath(?:,|$)"
  }
  if ($runtimeLines.Count -lt 2) {
    throw "Android release-runtime lock slice is unexpectedly empty"
  }
  [System.IO.File]::WriteAllLines(
    $runtimeLock,
    [string[]]$runtimeLines,
    [System.Text.UTF8Encoding]::new($false)
  )

  Push-Location $repoRoot
  try {
    & osv-scanner scan source `
      --lockfile Cargo.lock `
      --lockfile fuzz/Cargo.lock `
      --lockfile package-lock.json `
      --lockfile $runtimeLock `
      --config osv-scanner.toml
    if ($LASTEXITCODE -ne 0) { throw "OSV dependency scan failed" }
  }
  finally {
    Pop-Location
  }
}
finally {
  $resolvedScanDir = [System.IO.Path]::GetFullPath($scanDir)
  if (-not $resolvedScanDir.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clean an OSV temporary directory outside the system temp path"
  }
  Remove-Item -LiteralPath $resolvedScanDir -Recurse -Force
}
