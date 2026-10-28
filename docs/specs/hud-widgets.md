# Retained HUD widgets

Clock, Keystrokes, Armor Status and Potion Status are real runtime modules with
normal activation, bindings, profile settings and drag/scale editor placement.
All use the shared native painter and actual client snapshots. Disabled widgets
appear only as editor previews; these never mutate the player's inventory or
status effects.

Clock retains analog/digital, date and 24-hour options. Local wall time comes
from the host's already-declared chrono dependency; analog uses weekday/date,
ticks and hour/minute hands, while digital uses 12/24-hour text and optional
date. The America timezone date convention is detected from TZ or Unix system
timezone configuration. Windows locale/date convention still needs validation.

Keystrokes retains keyboard/arrow, button/icon, spacebar and CPS-only choices.
Its snapshot reads the host's mapped movement keys and mouse state; physical
mouse press edges count in a bounded one-second CPS window. Synthetic click
registration is exposed for future combat producers. Layout dimensions follow
the retained reference. Authored key/icon geometry and host typography still
need full visual comparison.

Armor Status renders actual host item models, including their resource-pack
appearance, with durability percentages/bars, compact/detailed layout and a
four-second broken warning when a nearly broken piece disappears. Editor-only
empty previews use render-only diamond stacks. Potion Status reads actual
active effects, filters positive/negative effects, tracks maximum durations for
progress rings and shows names/amplifiers/time (including infinite duration).
Names currently use readable registry IDs rather than the full localization
pipeline. No effect is added or removed by these widgets.

This batch has compile evidence only. Actual gameplay, visual comparison,
localization and full parity gates remain pending until the implementation
batch is ready for the user's requested later validation pass.
