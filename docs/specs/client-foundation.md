# Shared client behavior specification

Reference pin: `3d600a48106822755cea2d633062e5ab1f383d48`. These are traced contracts, not a claim that all client behavior is specified or ported.

## Settings

`value/BooleanValue.java` preserves the raw preference but returns false when hidden. `ValueCondition` combines equality/inequality/boolean requirements with AND. `ModeValue` owns choices and dependent value branches. `NumberValue` rounds edits to increments about zero; its raw-load path differs from normal edits. `RandomValue` has ordered endpoints and clamped endpoint editing. Rust uses typed definitions and rejects invalid types/choices, clamps finite numeric edits to supported limits, reports adjusted/invalid imports, and preserves unknown keys. This deliberately avoids propagating malformed raw Java values into gameplay.

Current native HUD definitions expose Display Type (Coords; default horizontal), Render background (default true), position, scale, ARGB color, binding chord and activation. Color is converted to the host's ABGR vertex format at the adapter. Position is clamped to the current screen for drawing while preserving the configured preference. The reference's full anchored layout, blur, font selection and composite editors remain open.

## Bindings

`input/BindActivationMode.java` defines TOGGLE and HELD. `unmap/Bendable.activateIfMatched` requires all inputs of a multi-input binding to be down. `ModBendable.handleInput` enables a held module only when the chord is complete and disables it when any bound member is released. `Mod.u` gates activation on no open Minecraft screen and cancels handled key events. Mouse input uses the corresponding shared dispatcher and negative legacy codes; Rust uses named physical keys/buttons and removes the legacy encoding.

Current core supports chords, repeat suppression and cleanup on focus/screen loss. Editing bindings releases active held claims using the previous activation mode. Restoring a profile never activates a held module without a physical press. Release-edge machinery exists for event/macros work, but the module activation editor exposes the two actual reference modes. Complete keyboard/mouse capture and host input cancellation still require UI/integration work.

## Local profiles

Keep local save/load/rename/delete/import/export, enabled states, module settings, binds, friends/enemies and UI layout. Remove remote ID reservations, upload/cloud/public-profile/review actions and their online controls. The reference's profile creation incidental remote dependency is replaced by an ordinary local file operation.

Rust schema 1 stores `modules`, `settings`, `friends`, `enemies`, `ui` and unknown extension data. Named profiles use bounded safe filenames below the mod-owned `profiles/` directory; current preferences use `current.json`. Imports are data-only and force module enabled flags false. Explicit profile loading may restore enabled toggle modules. Invalid current files are preserved rather than overwritten; valid fields/unknown data round-trip. Writes use a private temporary file, fsync and atomic rename; no recursive application cleanup or broad deletion is performed. File size is bounded to 1 MiB. Local profile UI remains unfinished.

## Reference UI geometry and styling

`ui/click/frame/impl/main/ClickGuiMainFrame.java`: 400×240 frame, 40 header, 136 frame overlay and 120 content overlay widths. Sections are Modules, Friends and Profiles (`ClickGuiSection`). Modules page has category/favorites selection, module/legit search, cards and settings sidecars. `ModuleComponent` row is 110×20; floating category header is 110×18. HUD overview width is 137.5, body height 50, header 20. Source theme uses background RGB(26,25,26), secondary RGB(37,36,38), foreground RGB(209,209,209), muted RGB(89,88,89). The accent is `ThemeColors.z()` = `ClientSettings.getAccentColor()`, the ClientSettings "Gui Color" (default RGB(5,134,105)): toggles, section-tab underlines, module cards, macro icons and Text GUI "Match GUI color" follow it. `ThemeColors.O` RGB(6,161,126) is fixed: HUD editor guide lines (`HudModuleFrameBase`) and SUCCESS/FRIENDINVITESENT notifications.

Recreate retained local controls in this structure; do not replace the interface with stock Horizon settings widgets. Do not reproduce online-only tabs/controls as placeholders. Real reference captures, final font/icon provenance and pixel/interaction parity remain unverified.

## Click GUI over play

`ClientSettings.openGui` opens a natively created screen; `GuiScreenNativeCallbackBridge` forwards only `drawScreen` and `mouseClicked`, so the screen's pause flag is not visible in the Java text. `module/blatant/InvWalk.shouldHandleCurrentScreen` (lines 58-69) keeps movement in any non-chat screen except while `ClientSettings.activeComponent` (a focused click GUI text field) is set, which only makes sense if the world keeps ticking under the click GUI. Rust treats the click GUI as non-pausing (an inference, not a capture), keeps the vanilla HUD under it without a dim, and draws the Text GUI, Target Info and notifications over its frames. The HUD editor keeps its dim.

## Text GUI clicks

`TextGuiSettings` "Click disable" (default off) acts only while the click GUI is open: a hovered row's colour blends toward white, and a press on a row toggles its module. A module switched off this way stays listed, with a grey `| Disabled` suffix, until the GUI closes. The hit rectangle is the row's background box from the previous drawn frame, in GUI pixels. "Match GUI color" paints rows with the Gui Color accent.

## Tooltips

`GuiComponent.bindValue` gives a value-bound component a `ToolTips` built from the value description (font scale 0.75); headers carry fixed texts such as "Edit hidden modules". While hovered with "Show tooltips" on, the first child with a tooltip claims it; otherwise `updateToolTipHover` accumulates `tooltipHoverMillis += now - tooltipHoverStartMillis` per frame and shows the tooltip at the mouse once it reaches 2000. `hideToolTips` resets both counters. `ToolTips` draws last: box at mouse x+8 (flipped left past the screen half-width minus 8) and mouse y (flipped up past the bottom), size (text width + 6, lines × (line height + 1) + 2.5), fill RGB(20,20,20) with a RGB(26,25,26) shadow, RGB(35,35,35) outline, text RGB(163,163,163) at (+3, +2).
