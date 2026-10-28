# NoHurtCam

Static reference: render/hud/NoHurtCameraHudModule.java at 3d600a48106822755cea2d633062e5ab1f383d48. It belongs to the retained Game group in Legit/HUD modules, has no module-specific settings and defaults disabled. Common bindings/activation/preferences still apply. The source temporarily saves and clears hurt time before rendering and restores it afterward; the intended result is removal of damage camera shake, not removal of damage, view bobbing or the death camera effect.

Pinned host: game/fov.rs Game::bob_matrix builds camera::BobState from actual hurt/death and interpolated avatar state. camera::bob_hurt applies the death roll before testing negative hurt time, then applies the damage shake. A cancellable HEAD calls the original shared host camera math with a copied BobState whose hurt_time is negative. Actual health/hurt timers, saved options and physics state are never mutated. View bobbing and death animation remain in that shared math. The disabled helper continues into the untouched original body.

This is a rendering-only interpretation of the source's temporary mutation. It avoids any restoration window while preserving the intended render behavior. There is no queued gameplay action or persistent host override to restore on disconnect or panic. The module preference survives world changes; panic disables it immediately.

Embedded native Rust tests check removal of hurt shake, preservation of nonidentity view bobbing and death animation, and unchanged input snapshot. Isolated native scenarios exercise actual damage/hurt state, binding/GUI toggling, restart and panic. Full retained GUI/icon parity remains a separate unfinished gate; no full port claim follows from this module.
