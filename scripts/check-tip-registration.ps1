param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug",

    [switch]$NoBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot

Push-Location $repoRoot
try {
    $toolTargetDir = Join-Path $repoRoot "target\tip-tool-refresh"
    if (-not $NoBuild) {
        $cargoArgs = @("build", "-p", "doubao-tsf-tip", "--bin", "doubao-tip-tool", "--target-dir", $toolTargetDir)
        if ($Profile -eq "release") {
            $cargoArgs += "--release"
        }

        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
    }

    $toolPath = Join-Path $toolTargetDir "$Profile\doubao-tip-tool.exe"
    if ($NoBuild -and -not (Test-Path -LiteralPath $toolPath)) {
        $toolPath = Join-Path $repoRoot "target\$Profile\doubao-tip-tool.exe"
    }
    & $toolPath status
    if ($LASTEXITCODE -ne 0) {
        throw "doubao-tip-tool status failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
