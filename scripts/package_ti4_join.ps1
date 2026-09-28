# Build the lean online client and zip it for players:
#
#   out\ti4-join-<commit>-windows-x64.zip
#     ti4-join.exe    the client (no libtorch, no data files)
#     Install.cmd     double-click installer (per user, no admin)
#     install.ps1
#     uninstall.ps1
#     README.txt
#
# The client refuses a host on a different commit, so the package is built from committed sources
# only: a dirty client crate would ship code the commit in its name does not describe.
param(
    [switch]$AllowDirty
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$sources = @('Cargo.toml', 'Cargo.lock', 'crates/ti4-replayer', 'crates/ti4-review/src',
    'crates/ti4-review/Cargo.toml', 'crates/ti4-engine', 'crates/ti4-model', 'crates/ti4-content')
$dirty = git status --porcelain --untracked-files=no -- @sources
if ($dirty -and -not $AllowDirty) {
    throw "Client sources have uncommitted changes; commit them or pass -AllowDirty:`n$($dirty -join "`n")"
}
$commit = (git rev-parse HEAD).Trim()
$short = $commit.Substring(0, 8)

cargo build --release -p ti4-replayer --no-default-features --bin ti4-join
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
$exe = Join-Path $root 'target\release\ti4-join.exe'

$protocol = (Select-String -Path 'crates\ti4-replayer\src\net\protocol.rs' -Pattern 'pub const PROTOCOL: &str = "([^"]+)"').Matches[0].Groups[1].Value
$readme = (Get-Content -Raw 'scripts\ti4-join-README.txt').Replace('@COMMIT@', $commit).Replace('@PROTOCOL@', $protocol)
$installCmd = "@echo off`r`npowershell -NoProfile -ExecutionPolicy Bypass -File `"%~dp0install.ps1`" %*`r`npause`r`n"

$outDir = Join-Path $root 'out'
if (-not (Test-Path -LiteralPath $outDir)) { throw "$outDir does not exist; not creating it." }
$zipPath = Join-Path $outDir "ti4-join-$short-windows-x64.zip"
if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }

Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::Open($zipPath, 'Create')
try {
    $level = [System.IO.Compression.CompressionLevel]::Optimal
    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $exe, 'ti4-join.exe', $level)
    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, (Join-Path $root 'scripts\ti4-join-install.ps1'), 'install.ps1', $level)
    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, (Join-Path $root 'scripts\ti4-join-uninstall.ps1'), 'uninstall.ps1', $level)
    foreach ($entry in @(@('README.txt', $readme), @('Install.cmd', $installCmd))) {
        $writer = New-Object System.IO.StreamWriter(($zip.CreateEntry($entry[0], $level)).Open())
        try { $writer.Write($entry[1]) } finally { $writer.Dispose() }
    }
} finally {
    $zip.Dispose()
}

$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath).Hash.ToLower()
$size = [math]::Round((Get-Item -LiteralPath $zipPath).Length / 1MB, 1)
"package  $zipPath"
"size     $size MB"
"sha256   $hash"
"commit   $commit"
