param(
    [string]$Version = $env:DRATON_VERSION,
    [string]$InstallRoot = $(if ($env:DRATON_INSTALL_ROOT) { $env:DRATON_INSTALL_ROOT } else { Join-Path $env:LOCALAPPDATA "Draton" })
)

$ErrorActionPreference = "Stop"
$repo = if ($env:DRATON_REPO) { $env:DRATON_REPO } else { "draton-lang/draton" }
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()

switch ($arch) {
    "X64" { $artifact = "draton-early-windows-x86_64.zip" }
    "Arm64" {
        Write-Error "Windows aarch64 is not part of the current Draton Early Tooling Preview target set."
    }
    default { Write-Error "Unsupported Windows architecture: $arch" }
}

if ($Version) {
    $url = "https://github.com/$repo/releases/download/$Version/$artifact"
} else {
    $url = "https://github.com/$repo/releases/latest/download/$artifact"
}
$checksumUrl = "$url.sha256"

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("draton-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
    $archive = Join-Path $tmp $artifact
    $checksumFile = "$archive.sha256"
    Invoke-WebRequest -Uri $url -OutFile $archive
    Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumFile
    $expectedHashLine = Get-Content -Path $checksumFile | Select-Object -First 1
    $expectedHash = ($expectedHashLine -split "\s+")[0]
    if (-not $expectedHash) {
        throw "Failed to read SHA256 checksum for $artifact."
    }
    $actualHash = (Get-FileHash -Path $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $expectedHash.ToLowerInvariant()) {
        throw "Checksum verification failed for $artifact."
    }
    Expand-Archive -Path $archive -DestinationPath $tmp -Force

    $root = Get-ChildItem -Path $tmp -Directory | Select-Object -First 1
    if (-not $root) {
        throw "Archive did not contain an installable root directory."
    }

    New-Item -ItemType Directory -Force -Path $InstallRoot | Out-Null
    $finalDir = Join-Path $InstallRoot $root.Name
    if (Test-Path $finalDir) {
        Remove-Item -Recurse -Force $finalDir
    }
    Move-Item -Path $root.FullName -Destination $finalDir

    $currentDir = Join-Path $InstallRoot "current"
    if (Test-Path $currentDir) {
        Remove-Item -Recurse -Force $currentDir
    }
    Copy-Item -Recurse -Force $finalDir $currentDir

    & (Join-Path $currentDir "drat.exe") --version

    Write-Host ""
    Write-Host "Installed Draton Early Tooling Preview to:"
    Write-Host "  $finalDir"
    Write-Host ""
    Write-Host "Add this directory to PATH:"
    Write-Host "  $currentDir"
    Write-Host ""
    Write-Host "Then verify:"
    Write-Host "  drat --version"
    Write-Host "  drat fmt --check $currentDir\\examples\\early-preview\\hello-app\\src"
}
finally {
    if (Test-Path $tmp) {
        Remove-Item -Recurse -Force $tmp
    }
}
