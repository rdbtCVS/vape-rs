# StorageESP baseline

The initial host integration enumerates loaded Horizon block entities and adds
world-space outlines for `Chest` and `Furnace` entries to the renderer's
existing depth-tested fishing-line batch. The module is off by default and
uses the normal registry, settings, binding, profile and panic mechanisms.
Chest and furnace toggles and colors use the reference defaults. The hook
leaves the original host method untouched when disabled; when enabled it
reproduces the pinned fishing-line body before adding outlines. The pinned
`game/fishing.rs` SHA-256 is
`0b45a5abf73f1124168741ce94fd89d46ecea55fee9f1a0a2af4fdd020b39006`.

This is not reference parity. The line batch is depth tested; the reference
renders through walls and also draws filled geometry. Bounds are unit cubes,
not model-specific bounds. Trapped and ordinary chests are not distinguished;
open-chest coloring, double chest merging, ender chests, hoppers, dispensers,
droppers and shulkers are not implemented. Their controls are deliberately
absent until their behavior is connected. ESP, Tracers, Nametags, the other
requested modules, world text, quads, camera/projection snapshots and collision
queries remain open. No shared targeting adapter was added.
