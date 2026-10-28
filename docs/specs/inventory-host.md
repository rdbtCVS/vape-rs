# Native inventory host bridge

`game::remote::vape_transactions` snapshots only Horizon's live player menu:
hotbar (0–8), main inventory (9–35), armor (500–503), and chest/container
slots (300+). It carries the runtime session generation, remote container ID
and its server state ID. In single player, where no packet state ID exists,
the revision is a content fingerprint of the host menu copy; it advances only
after the ordinary local menu engine changed content.

`submit` re-snapshots and rejects any session/container/revision/fingerprint
mismatch. Accepted requests call Horizon's existing `menu_ops`, not direct
inventory mutation: local menus use the verified menu engine, while remote
menus use `remote_menu_ops`, its mapping/prediction bookkeeping and normal
`ContainerClick` packet state/changed-slot/carried-stack fields. Its boolean
means the host accepted the request for processing/sending; it is never a
server acknowledgement. Planners wait for newer state/content snapshots.

Required manifest addition:

```toml
[[module]]
id = "transactions"
file = "crates/horizon-client/src/game/remote.rs"
name = "vape_transactions"
source = "transactions.rs"
```

The runtime must export `inventory` and invoke snapshot/submit from its own
planner dispatch. Potion harmfulness is deliberately not guessed from an item
identifier: the current bridge reports it false until a host component adapter
supplies exact effect metadata, so a blacklist policy cannot silently discard
potions based on a fake classification.
