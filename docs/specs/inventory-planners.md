# Inventory transaction planners

The inventory layer receives bounded host snapshots carrying `session`,
`container_id`, and authoritative menu `revision`, plus semantic item metadata
(registry ID/tag fingerprint, stack/durability, armor/category/healing and
host-computed scores). It emits typed pickup/quick-move/throw requests with
the exact identity/revision observed. A request gate permits one in flight;
only a newer matching snapshot acknowledges it. Container/session changes,
screen closure, disable and context interruption clear the gate and queues.

AutoArmor retains Inventory only, Open inventory, Check durability, Drop
equipped and Combat check. It selects a strictly stronger armor item (or equal
score with lower damage when enabled), clears/replaces the equipped slot, and
uses ordinary host requests. Refill retains Both/Pots/Soup, vertical order,
scatter, hotbar-clear and allow-list behavior, opening the player inventory,
filling a suitable hotbar slot, then closing/disabling once exhausted.
ChestSteal retains menu validation, best-only class selection, keep-open,
shuffle, blacklist and harmful-splash filtering; it quick-moves one eligible
chest slot per acknowledged transaction and closes only after the queue drains.

Host integration remains required for exact menu slot mappings, the actual
state-ID/revision packet/click path, open/close inventory input, combat guard,
and authoritative post-click snapshots. No planner treats an emitted request
as accepted before that acknowledgement.
