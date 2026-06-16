param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-IsAdministrator)) {
    throw "TIP unregistration writes HKCR/HKLM and must run from an elevated PowerShell."
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$cargoArgs = @("build", "-p", "doubao-tsf-tip", "--bin", "doubao-tip-tool")
if ($Profile -eq "release") {
    $cargoArgs += "--release"
}

Push-Location $repoRoot
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    $toolPath = Join-Path $repoRoot "target\$Profile\doubao-tip-tool.exe"
    & $toolPath unregister
    if ($LASTEXITCODE -ne 0) {
        throw "doubao-tip-tool unregister failed with exit code $LASTEXITCODE"
    }

    & $toolPath status
    if ($LASTEXITCODE -ne 0) {
        throw "doubao-tip-tool status failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
