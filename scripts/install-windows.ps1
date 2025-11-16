winparam(
    [string]$Owner = "TheMartynasXS",
    [string]$Repo  = "hash-service",
    [string]$InstallDir = "$env:LOCALAPPDATA\LeagueToolkit\ltk-hash-service"
)

$ErrorActionPreference = 'Stop'

Write-Host "Installing ltk-hash-service..." -ForegroundColor Cyan

if (!(Test-Path -LiteralPath $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# Get latest release metadata
$releaseApi = "https://api.github.com/repos/$Owner/$Repo/releases/latest"
try {
    $release = Invoke-RestMethod -Uri $releaseApi -Headers @{ 'User-Agent' = 'ltk-hash-service-installer' }
} catch {
    throw "Failed to query GitHub releases: $($_.Exception.Message)"
}

$tag = $release.tag_name
# Extract the first semantic version (handles tags like "v0.1.1")
$match = [regex]::Match($tag, '\d+\.\d+\.\d+([\-\+][A-Za-z0-9\.-]+)?')
$version = if ($match.Success) { $match.Value } else { $tag.TrimStart('v') }

# Our release workflow uploads a single Windows asset named ltk-tex-utils-windows.exe
$assetName = "ltk-hash-service-windows.exe"
$asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
if (-not $asset) {
    # Fallback: find any windows exe for this project
    $asset = $release.assets | Where-Object { $_.name -match '^ltk-hash-service-.*windows.*\.exe$' } | Select-Object -First 1
}
if (-not $asset) {
    throw "Could not find a Windows asset in the latest release."
}
$assetName = $asset.name

$exePath = Join-Path $InstallDir 'ltk-hash-service.exe'
$tmpPath = Join-Path $env:TEMP $assetName

Write-Host "Downloading $assetName ($version)..." -ForegroundColor Yellow
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tmpPath -UseBasicParsing

Write-Host "Placing binary into $InstallDir" -ForegroundColor Yellow
Copy-Item -LiteralPath $tmpPath -Destination $exePath -Force

# Create a shim directory so PATH is simple and stable
$binDir = Join-Path $InstallDir 'bin'
if (!(Test-Path -LiteralPath $binDir)) { New-Item -ItemType Directory -Path $binDir | Out-Null }

# Ensure the executable exists
if (!(Test-Path -LiteralPath $exePath)) {
    throw "ltk-hash-service.exe not found after download: $exePath"
}

Write-Host "Installed ltk-hash-service $version to $InstallDir" -ForegroundColor Green