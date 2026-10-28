# Native inventory host bridge

`game::remote::vape_transactions` snapshots only Horizon's live player menu:
hotbar (0–8), main inventory (9–35), armor (500–503), and chest/container
slots (300+). It carries the runtime session generation, remote container ID
and its server state ID. In single player, where no packet state ID exists,
the revision is a bounded stable content fingerprint of every slot ID/count/
damage/component fingerprint plus the carried cursor; it changes only after
the ordinary local menu engine changed content.

`submit` re-snapshots and rejects any session/container/revision/slot or
carried-cursor fingerprint mismatch. Accepted requests call Horizon's existing `menu_ops`, not direct
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
planner dispatch. Potion decisions read Horizon's decoded
`minecraft:potion_contents`: Refill accepts splash potions containing instant
health or regeneration, and ChestSteal treats splash poison, instant damage,
slowness, weakness and blindness as harmful. Unknown/missing components are
neither healing nor harmful; no item-ID suffix classification is used.
