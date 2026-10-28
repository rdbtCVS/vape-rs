# Coordinate, FPS and compass HUD layouts

Static behavioral reference: Vape-v4 commit 3d600a48106822755cea2d633062e5ab1f383d48, render/hud module classes and ui/click/frame/impl/hud frame classes. No Java code was executed. The native host remains e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083.

The registry owns one enabled/binding state per actual implemented module. It no longer has separate coordinate/FPS booleans and binding fields. Adding a registry entry still requires a real implementation; no undispatched module catalogue is exposed as working toggles.

## Coordinates

Display Type defaults to Horizontal; Vertical remains selectable. Java Math.round behavior is retained for supported game coordinates, including negative halfway values. Horizontal height is 35; content width follows measured values with 10 left padding, small labels, larger values, two directional arrows and dividers. The second row shows BIOME and the current biome name. Vertical is 70 by 90 with four evenly spaced rows, X/Y/Z values and a biome row, three dividers, and X/Z arrows at the right.

Heading is Minecraft yaw normalized then shifted by 180 degrees. Positive X is heading strictly between 0 and 180; positive Z is strictly between 90 and 270. Arrow backgrounds/triangles are authored geometry. The biome is sampled from the actual loaded client chunk at the player's block coordinates, including current Y. Modern 3D biome lookup at player height replaces the old reference's version-dependent 2D/Y-zero query. Missing chunks yield an empty biome rather than invented data. Current biome display names are readable registry IDs; full localization remains open.

## FPS

Fixed 50 by 20 frame. The actual host FPS is formatted with its FPS suffix and centered. Background/shadow and scale follow local preferences. Exact reference smooth-font metrics remain open.

## Compass

A 308 by 30 strip shows a 140-degree span around the live heading. Minor ticks occur every five degrees, direction labels every 45 degrees and numeric labels every 15 degrees between cardinals. North/east/south/west headings correspond to yaw -180/-90/0/90. Wrapping is continuous around north. A center arrow and current numeric heading are drawn above/in the strip. Tick spacing retains the source's width-plus-four calculation. Reference image resources are not copied; current ticks/arrows use original geometry. Full icon/typography fidelity remains an open visual gate.

Coordinates, FPS and Compass all use the same native painter for gameplay and disabled editor previews. Only enabled panels appear during gameplay; previews have reduced alpha and do not change enabled state. Layout is pure Rust with host font measurement supplied at draw time. The host-specific painter uses only the current frame's data and retains no GPU/world references.

The snapshot contains actual player position, yaw, current client-chunk biome and host FPS. F1, missing world and host screen context suppress normal HUDs. Local binding, scale, placement, color/background and enabled preferences use the existing atomic profile system.

This is implemented functionality, not complete reference parity. Fonts, image resources, localization, global HUD styling and all remaining widgets still require completion and native visual evidence.
