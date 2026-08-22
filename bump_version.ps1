# Usage: .\bump_version.ps1 0.3.0
# Updates the version in both `release` and `Cargo.toml`, and creates a new git tag.

param(
    [Parameter(Mandatory=$false, Position=0)]
    [string]$NewVersion
)

if (-not $NewVersion) {
    Write-Host "Usage: .\bump_version.ps1 <new_version>"
    Write-Host "Example: .\bump_version.ps1 0.3.0"
    exit 1
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

$Version = $NewVersion.TrimStart('v')
$Tag = "v$Version"

# Update the release file
Set-Content -Path (Join-Path $ScriptDir "release") -Value $Version -NoNewline

# Update Cargo.toml version field
$CargoPath = Join-Path $ScriptDir "Cargo.toml"
$Content = Get-Content $CargoPath -Raw
$Content = $Content -replace '(?m)^version = ".*"', "version = `"$Version`""
Set-Content -Path $CargoPath -Value $Content -NoNewline

Write-Host "Version updated to $Version in both release and Cargo.toml"

# Set new git tag
try {
    $existingTag = git tag -l $Tag
    if ($existingTag) {
        Write-Host "Git tag $Tag already exists."
    } else {
        git tag -a $Tag -m "Release $Tag"
        Write-Host "Created git tag $Tag"
    }
} catch {
    Write-Warning "Failed to create git tag $Tag: $_"
}

