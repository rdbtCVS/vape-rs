# Fullbright

Retains the source's Night Vision (default) and Gamma modes and Gamma-only
Fade option (default false). Night Vision overrides only the native renderer's
night-vision intensity to 1.0. It does not create a gameplay potion or remove a
real effect when disabled. This is the modern client-only interpretation of the
legacy visual potion workaround; the player's real status effects remain intact.

Gamma owns the live renderer brightness while active and restores the captured
brightness on disable/mode change. Saved game options are never changed. Fade
uses the reference's 0.4 increments at intervals of at least 10 ms toward 10.0
or the captured brightness. World/profile/panic cleanup restores immediately;
ordinary disable may finish its fade. Module activation, binds and options use
the common registry/profile/GUI path. No brightness override runs on a server.

The native lightmap night-vision boundary uses one cancellable HEAD. Disabled
execution goes through the original method. Gamma updates on the frame callback
and uses the host's existing lightmap gamma implementation. Shader-pack-specific
brightness uniforms, actual visual parity and cross-platform validation remain
pending. Compile evidence alone does not validate these modes.
