# FastPlace

Static reference: `module/world/FastPlace.java` at the pinned Vape-v4 revision. It has `Held Item` modes **All** (default), **Blocks**, and **Projectiles**, and integer `Delay` 0–4 (default 1). Each pre-tick, unless a macro item-use claim owns the path, it shortens the host right-click delay when either hand matches the selected item mode. Blocks have a block component; Projectiles are snowballs and eggs.

Horizon uses `Game::use_delay`, decrements it during local and remote ticks, and repeats held use only at zero. The helper must run at a `HEAD` hook on `horizon_client::game::Game<'_>::tick`, before that boundary. It changes only the ordinary repeat delay: item cooldowns, use-in-progress blocking, placement validation and remote packets remain host-owned.

Runtime settings: `world.fast-place.held-item` choice `all|blocks|projectiles`, default `all`; `world.fast-place.delay` number 0–4 step 1, default 1. The adapter uses `min(current_delay, configured_delay)`, matching the reference's `current > configured` condition: it never lengthens a pending repeat. It guards dedicated/server mode, no player, unfocused/non-Playing screens, title/dialog/server-pack prompts, sleep, spectator, and active macro item use. It never synthesizes clicks.

Required integration manifest block:

```toml
[[module]]
id = "fast-place"
file = "crates/horizon-client/src/game.rs"
name = "vape_fast_place"
source = "fast_place.rs"

[[injection]]
id = "fast-place-tick"
target = "horizon_client::game::Game<'_>::tick"
kind = "HEAD"
helper = "adapters/fast_place.rs" # delegates to crate::game::vape_fast_place::fast_place(game)
bindings = [{ name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" }]
```

The native source hash for `mixins/vape-rs/fast_place.rs` must be included by the existing support-module audit before an emitted bundle is accepted.
