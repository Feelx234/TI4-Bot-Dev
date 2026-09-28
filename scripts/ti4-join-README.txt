TI4 Join - a seat at an online TI4 table
=========================================

One program, ti4-join.exe. No libtorch, no game data files, nothing else to install.

Install
-------
Double-click Install.cmd. It copies the program to
%LOCALAPPDATA%\Programs\TI4 Join and adds "TI4 Join" to the Start menu and the desktop.
No administrator rights are needed. Or skip installing and just run ti4-join.exe from this folder.

Windows may warn that the program is from an unknown publisher (it is not code-signed):
choose "More info" -> "Run anyway".

Join a game
-----------
The host starts a table in the replayer and presses "Host online". They send you:
  - their address and port, e.g. 100.64.12.7:47474
  - the join code (32 letters and digits)

Open TI4 Join, fill those in, optionally pick a seat (seat0 .. seat5) and your name, and press Join.
From a terminal you can also run:
  ti4-join 100.64.12.7:47474 --code <code> --seat seat2 --name Alex

If your connection drops, press Reconnect: the same seat is kept for you.

Version
-------
Host and players must run the same build. This package was built from commit @COMMIT@
(protocol @PROTOCOL@). A player on a different build is refused when they join.

Network
-------
The connection is not encrypted. Over the internet, the host should either use a VPN everyone
joins (Tailscale is the easy one) or forward the port on their router.

Uninstall
---------
Run uninstall.ps1 from %LOCALAPPDATA%\Programs\TI4 Join, or delete that folder and the two shortcuts.
