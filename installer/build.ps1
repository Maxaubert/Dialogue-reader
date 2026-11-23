# Builds the DialogReader installer: target\installer\DialogReader-Setup-x64-<version>.exe (Inno Setup 6).
#
#   pwsh installer\build.ps1                 # ship build, files next to target\ship\DialogReader.exe, then ISCC
#   pwsh installer\build.ps1 -Fetch          # CI: download ONNX Runtime and the Kokoro files from their releases
#   pwsh installer\build.ps1 -NoBuild        # use the DialogReader.exe that target\ship already has
#
# What ships (docs/app-plan.md, M5, "Kokoro in the installer"): DialogReader.exe, onnxruntime.dll with its licence
# and notices, the four Visual C++ runtime DLLs it imports (14.40 or newer, app-local), kokoro\kokoro-v1.0.onnx
# and kokoro\voices-v1.0.bin, and THIRD_PARTY_NOTICES.md. Every downloaded or copied file with a pinned SHA-256 is
# checked here; a wrong file stops the build.
#
# Builds use CARGO_TARGET_DIR when it is set (this repo's rule: the repo's own target folder).

[CmdletBinding()]
param(
    [switch]$Fetch,
    [switch]$NoBuild,
    # where the runtime files come from without -Fetch
    [string]$From
)

$ErrorActionPreference = 'Stop'
$Repo = Split-Path -Parent $PSScriptRoot
$Target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Repo 'target' }
if (-not $From) { $From = Join-Path $Target 'ship' }

# ONNX Runtime 1.28.2 CPU build for Windows x64 (Microsoft, MIT). The zip and the DLL inside it are pinned.
$OrtVersion = '1.28.2'
$OrtUrl = "https://github.com/microsoft/onnxruntime/releases/download/v$OrtVersion/onnxruntime-win-x64-$OrtVersion.zip"
$OrtZipSha = 'c4eedd29489d5feca21866d054638416f3655bf6b18851b3b6b85c8313e95c35'
# Kokoro v1.0 from kokoro-onnx's release model-files-v1.0 (the same pins as Sonara's download.rs)
$KokoroBase = 'https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0'
$Pinned = [ordered]@{
    'onnxruntime.dll'          = '1becbd71adbf49609d33195e29c9214969db3c3de69c81425bebe5a4b69aef97'
    'kokoro\kokoro-v1.0.onnx'  = '7d5df8ecf7d4b1878015a32686053fd0eebe2bc377234608764cc0ef3636a6c5'
    'kokoro\voices-v1.0.bin'   = 'bca610b8308e8d99f32e6fe4197e7ec01679264efed0cac9140fe9c29f1fbf7d'
}
$VcDlls = 'msvcp140.dll', 'msvcp140_1.dll', 'vcruntime140.dll', 'vcruntime140_1.dll'
# The oldest Visual C++ runtime ONNX Runtime 1.28.2 runs with (VS 2022 17.10)
$VcMin = [version]'14.40.33810.0'

function Get-Sha([string]$Path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant() }

function Assert-Sha([string]$Path, [string]$Want) {
    $got = Get-Sha $Path
    if ($got -ne $Want) { throw "$Path has SHA-256 $got, expected $Want" }
}

function Get-WorkspaceVersion {
    $inPkg = $false
    foreach ($line in Get-Content (Join-Path $Repo 'Cargo.toml')) {
        $l = $line.Trim()
        if ($l.StartsWith('[')) { $inPkg = ($l -eq '[workspace.package]'); continue }
        if ($inPkg -and $l -match '^version\s*=\s*"([^"]+)"') { return $Matches[1] }
    }
    throw 'no [workspace.package] version in Cargo.toml'
}

function Find-VcRedist {
    $candidates = @()
    if ($env:VCToolsRedistDir) { $candidates += $env:VCToolsRedistDir }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        foreach ($vs in (& $vswhere -latest -products * -property installationPath)) {
            $msvc = Join-Path $vs 'VC\Redist\MSVC'
            if (Test-Path $msvc) {
                $candidates += Get-ChildItem $msvc -Directory | Sort-Object Name -Descending | ForEach-Object FullName
            }
        }
    }
    foreach ($c in $candidates) {
        $crt = Get-ChildItem (Join-Path $c 'x64') -Directory -Filter 'Microsoft.VC14*.CRT' -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending | Select-Object -First 1
        if ($crt -and -not ($VcDlls | Where-Object { -not (Test-Path (Join-Path $crt.FullName $_)) })) { return $crt.FullName }
    }
    throw 'no Visual C++ redistributable folder found (install Visual Studio Build Tools)'
}

function Find-Iscc {
    $cmd = Get-Command iscc.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($p in @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
                     "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe")) {
        if (Test-Path $p) { return $p }
    }
    throw 'Inno Setup 6 (ISCC.exe) not found: winget install JRSoftware.InnoSetup'
}

$Version = Get-WorkspaceVersion
Write-Host "DialogReader $Version"

if (-not $NoBuild) {
    Push-Location $Repo
    try {
        cargo build --profile ship -p dr-app --bin DialogReader
        if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    } finally { Pop-Location }
}
$Exe = Join-Path $Target 'ship\DialogReader.exe'
$ExeVersion = (Get-Item $Exe).VersionInfo.ProductVersion
if ($ExeVersion -ne $Version) { throw "$Exe is version $ExeVersion, Cargo.toml says $Version (build it again)" }

$Stage = Join-Path $Target 'installer\stage'
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force (Join-Path $Stage 'kokoro') | Out-Null
Copy-Item $Exe $Stage
Copy-Item (Join-Path $Repo 'THIRD_PARTY_NOTICES.md') $Stage

if ($Fetch) {
    $Cache = Join-Path $Target 'installer\cache'
    New-Item -ItemType Directory -Force $Cache | Out-Null
    $zip = Join-Path $Cache "onnxruntime-win-x64-$OrtVersion.zip"
    if (-not (Test-Path $zip) -or (Get-Sha $zip) -ne $OrtZipSha) {
        Write-Host "downloading $OrtUrl"
        Invoke-WebRequest -Uri $OrtUrl -OutFile $zip
    }
    Assert-Sha $zip $OrtZipSha
    $ortDir = Join-Path $Cache 'ort'
    if (Test-Path $ortDir) { Remove-Item -Recurse -Force $ortDir }
    Expand-Archive -LiteralPath $zip -DestinationPath $ortDir
    $root = Join-Path $ortDir "onnxruntime-win-x64-$OrtVersion"
    Copy-Item (Join-Path $root 'lib\onnxruntime.dll') $Stage
    Copy-Item (Join-Path $root 'LICENSE') (Join-Path $Stage 'onnxruntime-LICENSE.txt')
    Copy-Item (Join-Path $root 'ThirdPartyNotices.txt') (Join-Path $Stage 'onnxruntime-ThirdPartyNotices.txt')
    foreach ($name in 'kokoro-v1.0.onnx', 'voices-v1.0.bin') {
        $file = Join-Path $Cache $name
        if (-not (Test-Path $file) -or (Get-Sha $file) -ne $Pinned["kokoro\$name"]) {
            Write-Host "downloading $KokoroBase/$name"
            Invoke-WebRequest -Uri "$KokoroBase/$name" -OutFile $file
        }
        Copy-Item $file (Join-Path $Stage 'kokoro')
    }
    $crt = Find-VcRedist
    foreach ($d in $VcDlls) { Copy-Item (Join-Path $crt $d) $Stage }
} else {
    foreach ($f in @('onnxruntime.dll', 'onnxruntime-LICENSE.txt', 'onnxruntime-ThirdPartyNotices.txt') + $VcDlls) {
        Copy-Item (Join-Path $From $f) $Stage
    }
    foreach ($name in 'kokoro-v1.0.onnx', 'voices-v1.0.bin') {
        Copy-Item (Join-Path $From "kokoro\$name") (Join-Path $Stage 'kokoro')
    }
}

foreach ($k in $Pinned.Keys) { Assert-Sha (Join-Path $Stage $k) $Pinned[$k] }
foreach ($d in $VcDlls) {
    $v = [version](Get-Item (Join-Path $Stage $d)).VersionInfo.FileVersion.Split(' ')[0]
    if ($v -lt $VcMin) { throw "$d $v is older than $VcMin, the oldest VC++ runtime ONNX Runtime $OrtVersion runs with" }
    Write-Host "$d $v"
}
Write-Host 'SHA-256 of the pinned files: ok'

$iscc = Find-Iscc
& $iscc "/DStage=$Stage" "/O$(Join-Path $Target 'installer')" (Join-Path $PSScriptRoot 'DialogReader.iss')
if ($LASTEXITCODE -ne 0) { throw 'ISCC failed' }
$out = Join-Path $Target "installer\DialogReader-Setup-x64-$Version.exe"
if (-not (Test-Path $out)) { throw "$out was not made" }
Write-Host "installer: $out"
