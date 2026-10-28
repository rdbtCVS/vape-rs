# Clear Water

The retained source class is `render/hud/NoFogHudModule.java`, registered as
Clear Water. It changes underwater fog only; there are no module-specific
options. It uses the normal module registry/binds/profile/GUI path.

Minecraft 26.3 renders linear fog rather than the old density event. The
native adapter keeps the actual host frame fog computation and color, then
moves the underwater environmental span to the normal terrain visibility
limit. Render-distance caps, atmospheric/lava/powder-snow fog and blindness /
darkness effects remain native. This is the explicit modern interpretation of
clear underwater visibility.

A cancellable HEAD runs only for an enabled underwater player. Its helper
preserves the pinned `frame_fog` input assembly and calls the normal native
`setup_fog`; the original method remains intact for every other case. The
source hash is pinned. RETURN injection was tried first but the native planner
rejects the source method's closures; no tool or Horizon checkout was changed
to bypass that guard.

Actual resource-backed underwater captures and complete visual parity remain
pending. This implementation batch runs compilation checks only.
