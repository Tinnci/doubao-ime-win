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
$cargoArgs = @("build", "-p", "doubao-tsf-tip")
if ($Profile -eq "release") {
    $cargoArgs += "--release"
}

Push-Location $repoRoot
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    $targetDir = Join-Path $repoRoot "target\$Profile"
    $toolPath = Join-Path $targetDir "doubao-tip-tool.exe"
    if ($DllPath -eq "") {
        $DllPath = Join-Path $targetDir "doubao_tsf_tip.dll"
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
