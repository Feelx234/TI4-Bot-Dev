# Removes what ti4-join-install.ps1 put in place: the program folder and the two shortcuts.
param(
    [string]$Destination = (Join-Path $env:LOCALAPPDATA 'Programs\TI4 Join'),
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'

$links = @(
    (Join-Path ([Environment]::GetFolderPath('Programs')) 'TI4 Join.lnk'),
    (Join-Path ([Environment]::GetFolderPath('Desktop')) 'TI4 Join.lnk')
)
# Only ever the folder the installer made: refuse anything that does not hold ti4-join.exe.
if (-not (Test-Path -LiteralPath (Join-Path $Destination 'ti4-join.exe'))) {
    throw "$Destination does not look like a TI4 Join install; nothing removed."
}
if ($DryRun) {
    "remove    $Destination"
    $links | Where-Object { Test-Path -LiteralPath $_ } | ForEach-Object { "remove    $_" }
    return
}
$links | Where-Object { Test-Path -LiteralPath $_ } | ForEach-Object { Remove-Item -LiteralPath $_ -Force }
Remove-Item -LiteralPath $Destination -Recurse -Force
"TI4 Join removed."
