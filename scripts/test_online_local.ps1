# Try online play on this one machine: a hosting replayer and some ti4-join windows, over loopback.
#
#   powershell -ExecutionPolicy Bypass -File scripts\test_online_local.ps1 [-Clients 2] [-Port 47474] [-DebugBuild]
#
# 1. Builds the replayer (host) and the lean ti4-join client (--no-default-features, as shipped).
# 2. Opens the replayer with --host, so it starts hosting by itself once you start a table.
# 3. Waits for the host to listen, then opens one ti4-join window per client, seated at seat1,
#    seat2, ... so seat0 is left for you in the host window if you want to play it.
#
# Nothing is installed and no folder is created. Close the windows to finish.
param(
    [ValidateRange(1, 5)][int]$Clients = 2,
    [int]$Port = 47474,
    [switch]$DebugBuild,
    [int]$WaitMinutes = 15
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$profileArgs = if ($DebugBuild) { @() } else { @('--release') }
$bin = Join-Path $root ('target\' + $(if ($DebugBuild) { 'debug' } else { 'release' }))

"Building the host (ti4-replayer)..."
cargo build @profileArgs -p ti4-replayer --bin ti4-replayer
if ($LASTEXITCODE -ne 0) { throw 'host build failed' }
"Building the lean client (ti4-join)..."
cargo build @profileArgs -p ti4-replayer --no-default-features --bin ti4-join
if ($LASTEXITCODE -ne 0) { throw 'client build failed' }

# A fresh code per run, as the real host would make; 32 hex characters.
$bytes = New-Object byte[] 16
[System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
$code = ($bytes | ForEach-Object { $_.ToString('x2') }) -join ''

# The replayer reads its inputs and settings relative to the repository root.
$host_ = Start-Process -FilePath (Join-Path $bin 'ti4-replayer.exe') -WorkingDirectory $root `
    -ArgumentList @('--host', $Port, '--code', $code) -PassThru
""
"Host window is open. In it:"
"  1. Start a table (Start a table... -> Start). Optionally set seat0 to Manual to play it yourself."
"  2. It starts hosting on port $Port by itself; the top bar shows the code."
"Waiting for the host to listen on 127.0.0.1:$Port ..."

function Test-Listening([int]$port) {
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $client.ConnectAsync('127.0.0.1', $port).Wait(300) -and $client.Connected
    } catch {
        $false
    } finally {
        $client.Dispose()
    }
}

$deadline = (Get-Date).AddMinutes($WaitMinutes)
while (-not (Test-Listening $Port)) {
    if ($host_.HasExited) { throw 'The host window was closed before it started hosting.' }
    if ((Get-Date) -gt $deadline) { throw "The host did not start hosting within $WaitMinutes minutes." }
    Start-Sleep -Milliseconds 500
}

"Host is listening. Opening $Clients client window(s)..."
$joined = @()
for ($i = 1; $i -le $Clients; $i++) {
    $joined += Start-Process -FilePath (Join-Path $bin 'ti4-join.exe') -PassThru -ArgumentList @(
        "127.0.0.1:$Port", '--code', $code, '--seat', "seat$i", '--name', "Tester$i")
}
""
"Clients are seated at seat1..seat$Clients; those seats are now Manual on the host."
"Press Run (or End of game) in the host window; each client gets its own choices."
"Things to check:"
"  - a client's players panel shows other hands as counts, never card names"
"  - a choice appears only in the window of the seat being asked"
"  - the host's choice panel says 'Tester1 is playing this seat online' for a client seat"
"  - close a client, then Reconnect it from its join form: it gets the same seat back"
""
"Join code for manual joins: $code"
"Waiting for the host window to close..."
$host_.WaitForExit()
$joined | Where-Object { -not $_.HasExited } | ForEach-Object { Stop-Process -Id $_.Id -Confirm:$false }
"Done."
