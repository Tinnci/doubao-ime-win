param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug",

    [string]$DllPath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-IsAdministrator)) {
    throw "TIP registration writes HKCR/HKLM and must run from an elevated PowerShell."
}

$repoRoot = Split-Path -Parent $PSScriptRoot

Push-Location $repoRoot
try {
    $targetDir = Join-Path $repoRoot "target\$Profile"
    if ($DllPath -eq "") {
        $DllPath = Join-Path $targetDir "doubao_tsf_tip.dll"
    }

    if (Test-Path -LiteralPath $DllPath) {
        $toolTargetDir = Join-Path $repoRoot "target\tip-tool-refresh"
        $cargoArgs = @("build", "-p", "doubao-tsf-tip", "--bin", "doubao-tip-tool", "--target-dir", $toolTargetDir)
        if ($Profile -eq "release") {
            $cargoArgs += "--release"
        }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build doubao-tip-tool failed with exit code $LASTEXITCODE"
        }
        $toolPath = Join-Path $toolTargetDir "$Profile\doubao-tip-tool.exe"
    } else {
        $cargoArgs = @("build", "-p", "doubao-tsf-tip")
        if ($Profile -eq "release") {
            $cargoArgs += "--release"
        }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
        $toolPath = Join-Path $targetDir "doubao-tip-tool.exe"
    }

    & $toolPath register --dll-path $DllPath
    if ($LASTEXITCODE -ne 0) {
        throw "doubao-tip-tool register failed with exit code $LASTEXITCODE"
    }

    Start-Sleep -Milliseconds 1000

    & $toolPath status
    if ($LASTEXITCODE -ne 0) {
        throw "doubao-tip-tool status failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
