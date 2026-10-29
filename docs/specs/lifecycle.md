# Client lifetime and session boundaries

Host source is the committed Horizon e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083 export, never adjacent working-tree HEAD. A native state field owns one runtime per Game. Server/dedicated instances receive ordinary owned default state but are rejected by mod adapters before runtime use. Runtime settings and profile storage belong to the client instance; they are independent of its currently loaded world.

Session generations are monotonically increasing u64 values, never serialized into profiles. Exhaustion fails instead of wrapping into a previously valid generation. Entity handles contain a generation and numeric entity ID. A handle is usable only for the current generation while the client has a living player in a world. Reusing an entity ID in the next world does not revive an old handle.

The native boundary hooks run before original bodies, do not cancel, and conservatively invalidate even if an attempted transition subsequently fails:

| Hook | Host function | Invalidation |
| --- | --- | --- |
| session-world-open | game::Game::open_world | Local open/create request, including integrated-server selection |
| session-world-close | game::Game::close_world | Save and quit request |

| session-respawn | game::screens::Game::respawn_player | UI respawn request |
| session-connect | game::remote::Game::join_server | External or integrated memory connection |
| session-disconnect | game::remote::Game::leave_server | Connection teardown, including failures |
| session-remote-dimension | game::remote::Game::enter_remote_dimension | Received spawn/respawn dimension replacement |

Client frame and tick HEAD hooks observe presence, alive health, focus, screen and modal context. Entering a ready world, losing the world, and death invalidate once per observed state transition. Repeated identical frames do not advance generations. Both frame and tick observation matter: pause stops gameplay ticks, while packets/GUI transitions continue through frames. Multiple conservative boundaries during one open/respawn may advance more than once; generations are validity tokens, not a world-count statistic.

Invalidation releases chord/held bindings and closes the mod GUI on host transitions, death and panic. Ordinary toggle preferences remain enabled for the next world. Profile replacement invalidates transient state while retaining its open editor; it then applies the selected document. Panic additionally disables every registered module. Focus/screen/modal suspension releases held state without changing the generation every frame.

Current cleanup has real effects on bindings and GUI. Rotation/hotbar/input claims, inventory transactions and packet queues must be attached to this invalidation path as those systems are implemented; the existence of a generation type does not prove those future systems are safe. No host entity references or pending gameplay actions are currently retained by the runtime.

The original Java reference is static behavioral input only. Its module-specific cleanup still needs to be ported individually; this common lifecycle does not replace those specifications.
