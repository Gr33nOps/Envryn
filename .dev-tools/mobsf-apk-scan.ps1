param(
    [Parameter(Mandatory = $true)]
    [string]$ApkPath
)

$ErrorActionPreference = "Stop"
$containerName = "envryn-mobsf-$PID"
$apiKey = [guid]::NewGuid().ToString("N")

try {
    docker run --detach --rm --name $containerName `
        --publish 127.0.0.1:8000:8000 `
        --env "MOBSF_API_KEY=$apiKey" `
        opensecurity/mobile-security-framework-mobsf:latest | Out-Null

    $ready = $false
    # A first run may need to download JADX and initialize the analysis database.
    for ($attempt = 0; $attempt -lt 180; $attempt++) {
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:8000/" -TimeoutSec 2 | Out-Null
            $ready = $true
            break
        } catch {
            Start-Sleep -Seconds 2
        }
    }
    if (-not $ready) { throw "MobSF did not become ready within six minutes." }

    $env:MOBSF_API_KEY = $apiKey
    $env:MOBSF_URL = "http://127.0.0.1:8000"
    node .dev-tools/mobsf-apk-scan.mjs (Resolve-Path -LiteralPath $ApkPath)
    if ($LASTEXITCODE -ne 0) { throw "MobSF scan failed." }
} finally {
    Remove-Item Env:MOBSF_API_KEY -ErrorAction SilentlyContinue
    Remove-Item Env:MOBSF_URL -ErrorAction SilentlyContinue
    docker stop $containerName 2>$null | Out-Null
}
