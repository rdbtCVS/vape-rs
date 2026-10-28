# Retained typed setting editors

The source-sized module sidecar uses the typed setting definition supplied by
the runtime. It does not infer types from setting names or accept arbitrary
JSON from the UI.

- Booleans remain an immediate on/off control.
- Choices open a retained pop-up list and commit only the selected declared
  option.
- Numbers use a bounded, step-quantized drag track. Ranges render both
  endpoints and select the nearer endpoint when the track is pressed; an
  endpoint cannot cross the other one.
- Colors open an A/R/G/B pop-up with independent byte sliders and an opaque
  swatch. The value remains the setting system's `AARRGGBB` integer.
- Text, lists, and manual value edits retain their text buffer until Enter
  parses and submits it. Escape abandons the buffer.

Text fields support click placement and drag selection in addition to the
existing character-indexed keyboard selection/navigation, deletion and
explicit clipboard shortcuts. The painter API intentionally remains unchanged:
caret pointer placement uses the compact six-pixel cell used by the retained
controls, while drawing/scrolling and selection are measured by the host font
painter. This keeps input independent of a native font API; precise
proportional-glyph pointer hit testing is a future painter-metrics enhancement,
not a claim of pixel-perfect text interaction.

All slider/color changes flow through the normal `Action::Set` validation path,
so bounds, range ordering, choice membership and profile persistence stay
owned by typed settings rather than the GUI.
