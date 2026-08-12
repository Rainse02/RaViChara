param(
    [switch]$RequireCleanGit,
    [string]$ExpectedTag = ''
)

$ErrorActionPreference = 'Stop'
$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))

function Read-ProjectText([string]$RelativePath) {
    $path = Join-Path $projectRoot $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required file is missing: $RelativePath"
    }
    Get-Content -LiteralPath $path -Raw -Encoding UTF8
}

function Match-Version([string]$Text, [string]$Pattern, [string]$Label) {
    $match = [regex]::Match($Text, $Pattern)
    if (-not $match.Success) {
        throw "Could not read $Label version."
    }
    $match.Groups['version'].Value
}

$cargoText = Read-ProjectText 'Cargo.toml'
$appVersion = Match-Version $cargoText `
    '(?m)^version\s*=\s*"(?<version>\d+\.\d+\.\d+)"\s*$' `
    'application'
$tauri = Read-ProjectText 'src-tauri\tauri.conf.json' | ConvertFrom-Json
if ($tauri.package.version -ne $appVersion) {
    throw "Version mismatch: Cargo.toml=$appVersion, tauri.conf.json=$($tauri.package.version)."
}
$windowsResource = Read-ProjectText 'resources\windows.rc'
$fileVersionMatch = [regex]::Match(
    $windowsResource,
    '(?m)^FILEVERSION\s+(?<major>\d+),(?<minor>\d+),(?<patch>\d+),(?<revision>\d+)\s*$'
)
if (-not $fileVersionMatch.Success) {
    throw 'Windows VERSIONINFO FILEVERSION is missing.'
}
$windowsFileVersion = @(
    $fileVersionMatch.Groups['major'].Value,
    $fileVersionMatch.Groups['minor'].Value,
    $fileVersionMatch.Groups['patch'].Value
) -join '.'
if (
    $windowsFileVersion -ne $appVersion -or
    $fileVersionMatch.Groups['revision'].Value -ne '0' -or
    $windowsResource -notmatch ('VALUE\s+"ProductVersion",\s*"' + [regex]::Escape($appVersion) + '\\0"')
) {
    throw "Windows VERSIONINFO does not match application version $appVersion."
}

$addonManifest = Read-ProjectText `
    'blender_addons\ravichara_preview_bridge\blender_manifest.toml'
$addonVersion = Match-Version $addonManifest `
    '(?m)^version\s*=\s*"(?<version>\d+\.\d+\.\d+)"\s*$' `
    'Blender add-on'
$addonSource = Read-ProjectText `
    'blender_addons\ravichara_preview_bridge\__init__.py'
if ($addonSource -notmatch ('"version"\s*:\s*"' + [regex]::Escape($addonVersion) + '"')) {
    throw "Blender add-on source does not report version $addonVersion."
}
$backendBlender = Read-ProjectText 'src\blender\mod.rs'
$uiScript = Read-ProjectText 'static\app.js'
if ($backendBlender -notmatch ('"required_addon_version"\s*:\s*"' + [regex]::Escape($addonVersion) + '"')) {
    throw "Backend minimum add-on version does not match $addonVersion."
}
if ($uiScript -notmatch ("versionAtLeast\(addonVersion, '" + [regex]::Escape($addonVersion) + "'\)")) {
    throw "UI minimum add-on version does not match $addonVersion."
}

$protocolMatch = [regex]::Match(
    $addonSource,
    '"protocol_version"\s*:\s*(?<version>\d+)'
)
if (-not $protocolMatch.Success) {
    throw 'Could not read Blender bridge protocol version.'
}
$protocolVersion = $protocolMatch.Groups['version'].Value

if ($ExpectedTag -and $ExpectedTag -ne "v$appVersion") {
    throw "Release tag $ExpectedTag does not match application version v$appVersion."
}

$settings = Read-ProjectText 'config\settings.yaml'
if ($settings -notmatch '(?m)^\s*card:\s*characters/lily\.card\.yaml\s*$') {
    throw 'Public settings must select characters/lily.card.yaml.'
}
foreach ($line in ($settings -split "`r?`n")) {
    $secretField = [regex]::Match($line, '^\s*(?:api_key|token):\s*(?<value>.*?)\s*(?:#.*)?$')
    if ($secretField.Success) {
        $value = $secretField.Groups['value'].Value.Trim()
        if ($value -and $value -notin @('""', "''")) {
            throw 'config/settings.yaml contains a non-empty credential field.'
        }
    }
}

foreach ($requiredCharacter in @('lily.card.yaml', 'lily.avatar.svg')) {
    if (-not (Test-Path -LiteralPath (Join-Path $projectRoot "characters\$requiredCharacter"))) {
        throw "Redistributable character asset is missing: $requiredCharacter"
    }
}
$runtimeSource = Read-ProjectText 'src\runtime.rs'
$packageSource = Read-ProjectText 'scripts\package_windows.ps1'
foreach ($source in @($runtimeSource, $packageSource)) {
    if ($source -notmatch 'lily\.card\.yaml' -or $source -notmatch 'lily\.avatar\.svg') {
        throw 'Runtime seeding and packaging must include the Lily distribution assets.'
    }
}
if ($runtimeSource -match 'ganyu' -or $packageSource -match 'ganyu') {
    throw 'Private Ganyu assets must not be embedded or named by the public runtime/package.'
}

$ignore = Read-ProjectText '.gitignore'
foreach ($entry in @(
    '/target/',
    '/dist/',
    '/data/*',
    '/overrides.json',
    '/webview/',
    '/logs/',
    '/artifacts/',
    '/legacy_python/',
    '/characters/ganyu.card.yaml',
    '/characters/ganyu.avatar.png'
)) {
    if (-not ($ignore -split "`r?`n" | Where-Object { $_.Trim() -eq $entry })) {
        throw ".gitignore is missing required entry: $entry"
    }
}

$git = Get-Command git.exe -ErrorAction SilentlyContinue
$isGitRepository = $false
$gitBaseArguments = @(
    '-c',
    "safe.directory=$($projectRoot.Replace('\', '/'))",
    '-C',
    $projectRoot
)
if ($git) {
    & $git.Source @gitBaseArguments rev-parse --is-inside-work-tree 2>$null |
        Out-Null
    $isGitRepository = $LASTEXITCODE -eq 0
}

$trackedFiles = @()
if ($isGitRepository) {
    $trackedFiles = @(& $git.Source @gitBaseArguments ls-files)
    if ($LASTEXITCODE -ne 0) {
        throw 'git ls-files failed.'
    }
    $forbidden = $trackedFiles | Where-Object {
        $_ -match '^(?:target|dist|webview|logs|artifacts|legacy_python|\.vscode)/' -or
        ($_ -match '^data/' -and $_ -ne 'data/.keep') -or
        $_ -match '(?:\.crash\.txt|\.dmp|\.pyc)$' -or
        $_ -match '^characters/ganyu\.(?:card\.yaml|avatar\.png)$' -or
        $_ -match '^blender_addons/.+\.zip$' -or
        $_ -eq 'overrides.json'
    }
    if ($forbidden) {
        throw "Forbidden generated/private files are tracked: $($forbidden -join ', ')"
    }
    $publicCharacters = $trackedFiles | Where-Object { $_ -match '^characters/' }
    $unexpectedCharacters = $publicCharacters | Where-Object {
        $_ -notin @('characters/lily.card.yaml', 'characters/lily.avatar.svg')
    }
    if ($unexpectedCharacters) {
        throw "Unreviewed character assets are tracked: $($unexpectedCharacters -join ', ')"
    }

    $secretPatterns = @(
        '(?i)\bsk-[A-Za-z0-9_-]{16,}\b',
        '(?i)\b(?:api[_-]?key|access[_-]?token|client[_-]?secret)\b\s*[=:]\s*["''][A-Za-z0-9_./+-]{20,}["'']'
    )
    $textExtensions = @(
        '.c', '.css', '.html', '.js', '.json', '.md', '.ps1', '.py', '.rc',
        '.rs', '.svg', '.toml', '.txt', '.yaml', '.yml'
    )
    $suspectSecretFiles = @()
    foreach ($relative in $trackedFiles) {
        $extension = [System.IO.Path]::GetExtension($relative).ToLowerInvariant()
        if ($extension -notin $textExtensions) {
            continue
        }
        $absolute = Join-Path $projectRoot $relative
        if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
            continue
        }
        $content = Get-Content -LiteralPath $absolute -Raw -Encoding UTF8
        if ($secretPatterns | Where-Object { $content -match $_ }) {
            $suspectSecretFiles += $relative
        }
        if ((Get-Item -LiteralPath $absolute).Length -gt 25MB) {
            throw "Tracked file exceeds 25 MiB and needs explicit review: $relative"
        }
    }
    if ($suspectSecretFiles) {
        throw "Possible credential material found in tracked files: $($suspectSecretFiles -join ', ')"
    }

    if ($RequireCleanGit) {
        $status = @(& $git.Source @gitBaseArguments status --porcelain)
        if ($LASTEXITCODE -ne 0) {
            throw 'git status failed.'
        }
        if ($status) {
            throw 'Git worktree is not clean.'
        }
    }
} elseif ($RequireCleanGit) {
    throw 'A Git repository is required for -RequireCleanGit.'
}

[pscustomobject]@{
    Status = 'ready'
    AppVersion = $appVersion
    AddonVersion = $addonVersion
    BridgeProtocol = [int]$protocolVersion
    GitRepository = $isGitRepository
    GitCleanRequired = [bool]$RequireCleanGit
    DistributionCharacter = 'Lily'
} | Format-List
