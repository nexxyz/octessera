# Updates

With Wi-Fi on, **System > Maintenance > Updates > Check** tells you whether a
newer Octessera is out for your board. **Apply** downloads it, checks that the
download arrived intact (if it didn't, nothing changes), and restarts the
instrument on the new version. Give it a minute. If the new version doesn't
come up properly, the board quietly switches back to the one you had, so a
bad update costs you a restart, not an evening.

**Rollback** goes back to the version you had before the last update, and
pressing it again goes forward again. The board keeps those two versions and
tidies away older ones. Each time the version changes, the board saves a copy
of `system.json` (your settings) and `default.patch.json` (the startup patch)
for the version you're leaving, so the switch also swaps those two files:

| Action | Program | Settings and startup patch | Presets, samples, recordings |
|---|---|---|---|
| Update | Switches to the newer version | Carried over as they are | Untouched |
| Rollback | Switches back to the previous version | Replaced by the copy saved when you left that version (if it has one); what you had on the newer version is kept aside | Untouched |
| Rollback again (forward) | Switches to the newer version again | Replaced by the copy kept aside, but only if you haven't changed anything since the rollback; otherwise the changes you made on the older version stay | Untouched |

So changes made while you're on the older version survive going forward, and
settings you saved on the newer version come back only when you didn't touch
the older version's. Presets you saved on the newer version stay on the card;
the older one just won't open them. If a new version doesn't start, the board
puts the old settings and startup patch back along with the old program.

One honest caveat: pull the power during that minute of restarting and the
board may wake up on the new version without having checked it. If that version
then misbehaves, Rollback (or a fresh image) puts things right.

Updates only replace the Octessera program itself and, as above, those two
files. Your presets, samples and recordings stay where they are, and the
operating system, kernel and boot setup only change when you flash a new image.
