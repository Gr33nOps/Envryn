param(
  [switch]$SkipDependencyNetwork
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
  $required = @("gitleaks", "semgrep", "cargo-audit", "cargo-deny")
  if (-not $SkipDependencyNetwork) { $required += "osv-scanner" }
  foreach ($command in $required) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
      throw "Required free security tool is not installed: $command"
    }
  }

  node .dev-tools/verify-security-invariants.mjs
  if ($LASTEXITCODE -ne 0) { throw "security invariants failed" }

  gitleaks git --config .gitleaks.toml --redact --no-banner
  if ($LASTEXITCODE -ne 0) { throw "secret scan failed" }

  semgrep --config .semgrep/ crates/ src-tauri/ apps/ui/src --error
  if ($LASTEXITCODE -ne 0) { throw "static analysis failed" }

  cargo deny check
  if ($LASTEXITCODE -ne 0) { throw "Rust policy scan failed" }
  cargo audit
  if ($LASTEXITCODE -ne 0) { throw "Rust vulnerability scan failed" }

  if (-not $SkipDependencyNetwork) {
    osv-scanner scan source -r . --config osv-scanner.toml
    if ($LASTEXITCODE -ne 0) { throw "OSV dependency scan failed" }
    npm audit --omit=dev --audit-level=high
    if ($LASTEXITCODE -ne 0) { throw "npm production dependency audit failed" }
  }
} finally {
  Pop-Location
}
