# BlockOverlay

Status: **design draft only**. It is deliberately unregistered: no module,
settings, mixin manifest entry or native support source is in the shipping
tree. The retained prototype is preserved as
`docs/drafts/block-overlay-options.rs` and
`docs/drafts/block-overlay-native.rs`; these files are documentation artifacts
and have not been compiled or host-validated.

Static reference: `render/hud/BlockOverlayHudModule.java` in the pinned
Vape-v4 tree. It has exactly two retained options: **Overlay Color** (ARGB
`0x5fff0000`, translucent red fill) and **Outline Color** (ARGB
`0xc8ff0000`, red edge stroke). Its line width is a fixed 2 px in both source
render paths, not a user setting. Selected bounds are expanded by 0.002 to
avoid terrain z-fighting.

The options draft preserves these colors, fixed width and expansion. The native
draft `game::fishing::vape_block_overlay::fishing_line` helper would
uses Horizon's own `Game::pick` and private `Game::outline_boxes`, so it draws
the real selected block's context-sensitive outline shape rather than a unit
cube. It appends the twelve edges of every selected outline box to Horizon's
normal depth-tested transparent `gizmos::Line` batch with source ARGB and
2-pixel width.

If resumed, required runtime module ID is `ModuleId::BlockOverlay`. Required
settings are:

- `render.block-overlay.overlay-color`, `Kind::Color`, default `0x5fff0000`.
- `render.block-overlay.outline-color`, `Kind::Color`, default `0xc8ff0000`.

The proposed working outline boundary is a cancellable `HEAD` on
`Game::fishing_line(alpha, size) -> Vec<gizmos::Line>`. When disabled the
helper returns `ControlFlow::Continue(())`, leaving Horizon's original method
untouched. When enabled it reproduces the small pinned fishing-line method and
returns `Break(lines)` after appending the overlay. `Game::render` already
converts that return into its normal world-space, depth-tested line batch
before `gizmos::camera_relative`; no inaccessible render local is required.

Required helper (separate manifest helper file) is only:

```rust
fn fishing_line(
    game: &crate::game::Game<'_>, alpha: f64, size: [u32; 2],
) -> core::ops::ControlFlow<Vec<horizon_client::gizmos::Line>> {
    crate::game::fishing::vape_block_overlay::fishing_line(game, alpha, size)
}
```

To resume it, insert the support module in
`crates/horizon-client/src/game/fishing.rs`, add the helper/manifest entry,
and pin both the replacement body and source hash because it copies the
upstream method exactly. Compile and validate the disabled `Continue` path
and enabled outline path before registering the module.

Horizon currently exposes no transparent world-quad batch at the
`fishing_line` boundary. A concrete fill route exists at
`Game::entity_geometry`'s returned `Vec<Vertex>`: append six
camera-relative quads using the real `horizon/white` atlas sprite, ABGR
Overlay Color, two-sided normals and `TRANSLUCENT_ITEM_OVERLAY` (layer 32).
`EntityBatch` then routes them through `translucent_item_pipeline`, after
terrain with depth testing and depth writes disabled. The current mixin
transform cannot safely modify that large opaque-body function at RETURN/
assignment while retaining `&mut Game`, camera and its returned vector. It
needs an opaque-body-safe RETURN/AFTER_ASSIGN boundary (or a dedicated host
geometry hook) before this fill can be added. This mixin-tool boundary is the
next required implementation step; do not register BlockOverlay before the
fill path is implemented and validated. The draft intentionally does not
pretend a line batch is a filled overlay.

The geometry/tool limitation above is a static design finding. No plan/apply or native compilation has been run for these drafts; the registered module set is unchanged.
