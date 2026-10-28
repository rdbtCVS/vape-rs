# NoJumpDelay and NoClickDelay

Static sources are `render/hud/NoHurtDelayHudModule.java` (whose registered
name is **NoJumpDelay**) and `render/hud/NoClickDelayHudModule.java` in the
pinned Vape-v4 tree. The source names are historical and do not describe hurt
invulnerability behavior.

`NoJumpDelay` clears the local player's actual
`PlayerMotion::no_jump_delay` at the beginning of each client tick. Horizon's
`MotionWorld::jump_phase` sets that field to ten after a ground jump and tests
it before another one, so the normal key/input, collision and jump code remain
host-owned.

`NoClickDelay` clears the actual `Clicks::miss_time` only while `game.left`
(the resolved held attack input) is true. `Clicks::start_attack` sets this
ten-tick timer after a non-creative attack miss. Releasing attack deliberately
does not receive a synthetic reset; Horizon's ordinary release path remains
authoritative.

The adapter must be installed in the existing client `Game::tick` `HEAD`
boundary, before both local and remote player simulation. It is client-only
and does nothing unless focused, alive, playing with no GUI/title/dialog/
server-pack prompt/sleep/spectator state. It also excludes dedicated and
integrated serving mode. Required runtime IDs are `ModuleId::NoJumpDelay` and
`ModuleId::NoClickDelay`; neither module has settings.

The static NoClickDelay source retains an obsolete Forge 1.8.9 version gate.
Horizon has a single modern `Clicks::miss_time` implementation shared by the
relevant paths, so the adapter intentionally applies the same attack-held
condition without reproducing that obsolete compatibility gate.
