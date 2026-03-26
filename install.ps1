param(
    [string]$Version = $env:DRATON_VERSION,
    [string]$InstallRoot = $(if ($env:DRATON_INSTALL_ROOT) { $env:DRATON_INSTALL_ROOT } else { Join-Path $env:LOCALAPPDATA "Draton" })
)
$ErrorActionPreference = "Stop"
$repo = if ($env:DRATON_REPO) { $env:DRATON_REPO } else { "draton-lang/draton" }
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    "AMD64" { $artifact = "draton-early-windows-x86_64.zip" }
    "ARM64" {
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

function Download-File {
    param([string]$Uri, [string]$OutFile)
    $client = [System.Net.Http.HttpClient]::new()
    $client.DefaultRequestHeaders.Add("User-Agent", "Draton-Installer")
    try {
        $response = $client.GetAsync($Uri, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        $response.EnsureSuccessStatusCode()
        $stream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $fs = [System.IO.FileStream]::new($OutFile, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None, 81920)
        try { $stream.CopyTo($fs) }
        finally { $fs.Dispose(); $stream.Dispose() }
    } finally { $client.Dispose() }
}

try {
    $archive = Join-Path $tmp $artifact
    $checksumFile = "$archive.sha256"
    Download-File -Uri $url -OutFile $archive
    Download-File -Uri $checksumUrl -OutFile $checksumFile
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
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathEntries = @()
    if ($userPath) {
        $pathEntries = $userPath -split ';' | Where-Object { $_ }
    }
    $pathUpdated = $false
    if (-not ($pathEntries | Where-Object { $_.TrimEnd('\') -eq $currentDir.TrimEnd('\') })) {
        $newUserPath = if ($userPath) { "$userPath;$currentDir" } else { $currentDir }
        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
        $env:Path = "$env:Path;$currentDir"
        $pathUpdated = $true
    }
    Write-Host ""
    Write-Host "Installed Draton Early Tooling Preview to:"
    Write-Host "  $finalDir"
    Write-Host ""
    if ($pathUpdated) {
        Write-Host "Added this directory to your user PATH:"
    } else {
        Write-Host "This directory is already on your user PATH:"
    }
    Write-Host "  $currentDir"
    Write-Host ""
    Write-Host "Then verify:"
    Write-Host "  drat --version"
    Write-Host "  drat fmt --check $currentDir\examples\early-preview\hello-app\src"
}
finally {
    if (Test-Path $tmp) {
        Remove-Item -Recurse -Force $tmp
    }
}
