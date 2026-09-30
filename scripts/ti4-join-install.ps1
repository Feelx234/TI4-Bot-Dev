# Installs ti4-join for the current user: no administrator rights, nothing outside the user profile.
#
#   Program:     %LOCALAPPDATA%\Programs\TI4 Join\ti4-join.exe
#   Start menu:  TI4 Join
#   Desktop:     TI4 Join            (skip with -NoDesktopShortcut)
#
# Run it from the unzipped folder, or double-click Install.cmd. -DryRun prints what would happen.
param(
    [string]$Destination = (Join-Path $env:LOCALAPPDATA 'Programs\TI4 Join'),
    [switch]$NoDesktopShortcut,
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'

$source = Join-Path $PSScriptRoot 'ti4-join.exe'
if (-not (Test-Path -LiteralPath $source)) {
    throw "ti4-join.exe is not next to this script ($PSScriptRoot). Unzip the whole package first."
}
$uninstaller = Join-Path $PSScriptRoot 'uninstall.ps1'
$programs = [Environment]::GetFolderPath('Programs')
$desktop = [Environment]::GetFolderPath('Desktop')
$links = @(Join-Path $programs 'TI4 Join.lnk')
if (-not $NoDesktopShortcut) { $links += Join-Path $desktop 'TI4 Join.lnk' }

if ($DryRun) {
    "copy      $source -> $Destination\ti4-join.exe"
    "copy      $uninstaller -> $Destination\uninstall.ps1"
    $links | ForEach-Object { "shortcut  $_" }
    return
}

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Copy-Item -LiteralPath $source -Destination (Join-Path $Destination 'ti4-join.exe') -Force
if (Test-Path -LiteralPath $uninstaller) {
    Copy-Item -LiteralPath $uninstaller -Destination (Join-Path $Destination 'uninstall.ps1') -Force
}
$shell = New-Object -ComObject WScript.Shell
foreach ($path in $links) {
    $link = $shell.CreateShortcut($path)
    $link.TargetPath = Join-Path $Destination 'ti4-join.exe'
    $link.WorkingDirectory = $Destination
    $link.Description = 'Join an online TI4 table'
    $link.Save()
}
"Installed to $Destination. Start it from the Start menu: TI4 Join."
