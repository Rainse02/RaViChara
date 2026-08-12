param(
    [switch]$IncludeUserData,
    [switch]$SkipBuild,
    [switch]$AllowActiveRuntime
)

$ErrorActionPreference = 'Stop'
$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$distRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot 'dist'))
$cargoManifest = Get-Content -LiteralPath (Join-Path $projectRoot 'Cargo.toml') `
    -Raw -Encoding UTF8
$versionMatch = [regex]::Match(
    $cargoManifest,
    '(?m)^version\s*=\s*"(?<version>\d+\.\d+\.\d+)"\s*$'
)
if (-not $versionMatch.Success) {
    throw 'Could not determine the package version from Cargo.toml.'
}
$packageVersion = $versionMatch.Groups['version'].Value
$addonManifestPath = Join-Path $projectRoot `
    'blender_addons\ravichara_preview_bridge\blender_manifest.toml'
$addonManifest = Get-Content -LiteralPath $addonManifestPath -Raw -Encoding UTF8
$addonVersionMatch = [regex]::Match(
    $addonManifest,
    '(?m)^version\s*=\s*"(?<version>\d+\.\d+\.\d+)"\s*$'
)
if (-not $addonVersionMatch.Success) {
    throw 'Could not determine the Blender add-on version from blender_manifest.toml.'
}
$addonVersion = $addonVersionMatch.Groups['version'].Value
$outputRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $distRoot "RaViChara-$packageVersion")
)
$archivePath = [System.IO.Path]::GetFullPath(
    (Join-Path $distRoot "RaViChara-$packageVersion-Windows-x64-portable.zip")
)

if (-not $outputRoot.StartsWith(
    $projectRoot + [System.IO.Path]::DirectorySeparatorChar,
    [System.StringComparison]::OrdinalIgnoreCase
)) {
    throw "Refusing to package outside the project: $outputRoot"
}

$listener = Get-NetTCPConnection -LocalPort 8760 -State Listen -ErrorAction SilentlyContinue
$runtimeActive = [bool]$listener
if (-not $runtimeActive) {
    $probe = [System.Net.Sockets.TcpClient]::new()
    try {
        $connect = $probe.ConnectAsync('127.0.0.1', 8760)
        $runtimeActive = $connect.Wait(500) -and $probe.Connected
    } catch {
        $runtimeActive = $false
    } finally {
        $probe.Dispose()
    }
}
if ($runtimeActive -and -not $AllowActiveRuntime) {
    throw 'Port 8760 is active. Close RaViChara before packaging persistent SQLite data.'
}
if ($AllowActiveRuntime -and $IncludeUserData) {
    throw 'Cannot package mutable user data while port 8760 is active.'
}
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path -LiteralPath $cargo)) {
    $cargoCommand = Get-Command cargo.exe -ErrorAction Stop
    $cargo = $cargoCommand.Source
}

if (-not $SkipBuild) {
    & $cargo build --release --locked --offline
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo release build failed with exit code $LASTEXITCODE"
    }
}

$releaseExe = Join-Path $projectRoot 'target\release\RaViChara.exe'
if (-not (Test-Path -LiteralPath $releaseExe)) {
    throw "Release executable not found: $releaseExe"
}

if (Test-Path -LiteralPath $outputRoot) {
    Remove-Item -LiteralPath $outputRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null

Copy-Item -LiteralPath $releaseExe -Destination (Join-Path $outputRoot 'RaViChara.exe')
New-Item -ItemType File -Path (Join-Path $outputRoot 'RaViChara.portable') -Force |
    Out-Null

$outputConfig = Join-Path $outputRoot 'config'
New-Item -ItemType Directory -Path $outputConfig -Force | Out-Null
$settings = Get-Content -LiteralPath (Join-Path $projectRoot 'config\settings.yaml') -Raw -Encoding UTF8
$settings = [regex]::Replace(
    $settings,
    '(?m)^(\s*(?:api_key|token):\s*).*$',
    '$1""'
)
$cardPattern = [regex]::new('(?m)^(\s*card:\s*).*$')
$settings = $cardPattern.Replace(
    $settings,
    '$1characters/lily.card.yaml',
    1
)
[System.IO.File]::WriteAllText(
    (Join-Path $outputConfig 'settings.yaml'),
    $settings,
    [System.Text.UTF8Encoding]::new($false)
)

$outputCharacters = Join-Path $outputRoot 'characters'
New-Item -ItemType Directory -Path $outputCharacters -Force | Out-Null
foreach ($characterAsset in @('lily.card.yaml', 'lily.avatar.svg')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot "characters\$characterAsset") `
        -Destination $outputCharacters
}

if ($IncludeUserData) {
    # This mode is explicitly private: keep locally installed character assets
    # so the saved active-card override continues to resolve.
    Copy-Item -Path (Join-Path $projectRoot 'characters\*') `
        -Destination $outputCharacters -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'data') `
        -Destination (Join-Path $outputRoot 'data') -Recurse
    $outputOverrides = Join-Path $outputRoot 'data\overrides.json'
    if (Test-Path -LiteralPath $outputOverrides) {
        $runtimeOverrides = Get-Content -LiteralPath $outputOverrides -Raw -Encoding UTF8 |
            ConvertFrom-Json
        if ($runtimeOverrides.PSObject.Properties.Name -contains 'llm_api_key') {
            $runtimeOverrides.llm_api_key = ''
        }
        if ($runtimeOverrides.PSObject.Properties.Name -contains 'tts_api_key') {
            $runtimeOverrides.tts_api_key = ''
        }
        [System.IO.File]::WriteAllText(
            $outputOverrides,
            ($runtimeOverrides | ConvertTo-Json -Depth 32),
            [System.Text.UTF8Encoding]::new($false)
        )
    }
} else {
    New-Item -ItemType Directory `
        -Path (Join-Path $outputRoot 'data\memories') -Force | Out-Null
}

$addonOutput = Join-Path $outputRoot 'blender_addons'
New-Item -ItemType Directory -Path $addonOutput -Force | Out-Null
$addonSource = Join-Path $projectRoot 'blender_addons\ravichara_preview_bridge'
$addonZip = Join-Path $projectRoot "blender_addons\ravichara_preview_bridge-$addonVersion.zip"
if (Test-Path -LiteralPath $addonZip) {
    Remove-Item -LiteralPath $addonZip -Force
}
Compress-Archive -LiteralPath @(
    (Join-Path $addonSource '__init__.py'),
    (Join-Path $addonSource 'blender_manifest.toml'),
    (Join-Path $addonSource 'README.md')
) -DestinationPath $addonZip -CompressionLevel Optimal
$addonHash = Get-FileHash -LiteralPath $addonZip -Algorithm SHA256
$addonHashPath = "$addonZip.sha256"
[System.IO.File]::WriteAllText(
    $addonHashPath,
    "$($addonHash.Hash.ToLowerInvariant())  $([System.IO.Path]::GetFileName($addonZip))`r`n",
    [System.Text.Encoding]::ASCII
)
Copy-Item -LiteralPath $addonZip -Destination $addonOutput
Copy-Item -LiteralPath $addonHashPath -Destination $addonOutput
Copy-Item -LiteralPath (Join-Path $projectRoot 'blender_addons\ravichara_preview_bridge\README.md') `
    -Destination $addonOutput

$docsOutput = Join-Path $outputRoot 'docs'
New-Item -ItemType Directory -Path $docsOutput -Force | Out-Null
foreach ($document in @(
    'BLENDER_SETUP.md',
    'TTS_SETUP.md',
    'DESKTOP_BUILD.md',
    'CHARACTER_CARDS.md',
    'VERSIONING.md'
)) {
    $source = Join-Path $projectRoot "docs\$document"
    if (Test-Path -LiteralPath $source) {
        Copy-Item -LiteralPath $source -Destination $docsOutput
    }
}
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') `
    -Destination $outputRoot
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') `
    -Destination $outputRoot
Copy-Item -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md') `
    -Destination $outputRoot

$instructions = @(
    'RaViChara Windows x64 portable build',
    '',
    '1. Double-click RaViChara.exe.',
    '2. Configuration, chat history and memory are stored under data/.',
    '3. The public build starts with the original Lily example character.',
    '   The default LM Studio endpoint is http://127.0.0.1:1234/v1.',
    "4. Install Blender companion extension $addonVersion from blender_addons for semantic arm/IK control, A/T-pose idle calibration, and stable Blender preview.",
    '   Keep the Virtual_c service listening on 127.0.0.1:9876.',
    '5. API keys entered in the UI are stored locally in data/overrides.json.',
    '   Keys are scrubbed from portable archives even when user data is included.',
    '6. Runtime logs are written to logs/ravichara.log.',
    '',
    'Requires Windows 10/11 x64 and Microsoft Edge WebView2 Runtime.'
) -join [System.Environment]::NewLine
[System.IO.File]::WriteAllText(
    (Join-Path $outputRoot 'START_HERE.txt'),
    $instructions,
    [System.Text.UTF8Encoding]::new($false)
)

$hash = Get-FileHash -LiteralPath (Join-Path $outputRoot 'RaViChara.exe') -Algorithm SHA256
[System.IO.File]::WriteAllText(
    (Join-Path $outputRoot 'RaViChara.exe.sha256'),
    "$($hash.Hash.ToLowerInvariant())  RaViChara.exe`r`n",
    [System.Text.UTF8Encoding]::new($false)
)

if (Test-Path -LiteralPath $archivePath) {
    Remove-Item -LiteralPath $archivePath -Force
}
Compress-Archive -LiteralPath $outputRoot -DestinationPath $archivePath `
    -CompressionLevel Optimal
$archiveHash = Get-FileHash -LiteralPath $archivePath -Algorithm SHA256
$archiveHashPath = "$archivePath.sha256"
[System.IO.File]::WriteAllText(
    $archiveHashPath,
    "$($archiveHash.Hash.ToLowerInvariant())  $([System.IO.Path]::GetFileName($archivePath))`r`n",
    [System.Text.Encoding]::ASCII
)

$exe = Get-Item -LiteralPath (Join-Path $outputRoot 'RaViChara.exe')
$archive = Get-Item -LiteralPath $archivePath
[pscustomobject]@{
    AppVersion = $packageVersion
    AddonVersion = $addonVersion
    Executable = $exe.FullName
    ExecutableBytes = $exe.Length
    Sha256 = $hash.Hash.ToLowerInvariant()
    Archive = $archive.FullName
    ArchiveBytes = $archive.Length
    ArchiveSha256 = $archiveHash.Hash.ToLowerInvariant()
    UserDataIncluded = [bool]$IncludeUserData
} | Format-List
