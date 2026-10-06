# Winnu Commander Alliance Route

Status: implementation present; focused test added, not yet run. Scope: extend only the Winnu commander combat bonus to the canonical commander-grant routes supplied by `promissory::has_commander_ability`.

The bonus now checks whether the rolling player has `winnucommander`, rather than requiring that player's own `winnucommander` status to be unlocked. The existing system filter remains unchanged: Mecatol Rex, the rolling player's own home system, and any system containing a legendary planet receive +2. Thus a borrowed Winnu commander uses the recipient's home system, not the Winnu owner's.

`an_alliance_commander_grant_uses_the_recipient_home_system` seats no Winnu faction, grants `winnucommander` directly to Sol using the public durable commander-grant API, and asserts +2 for Sol's home and zero for another player's roll. Existing native commander unlock/bonus tests remain in place. Ordinary Alliance ownership, faceup state, exact-owner-unlock validation, and durable grants remain governed by `promissory::has_commander_ability`; this package does not change unrelated Alliance effects, including Hacan's vote bonus.

Validation pending: parent-coordinated shared low-memory build. No Cargo command was run by this scoped task.
