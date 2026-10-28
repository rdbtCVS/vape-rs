# Decisions

- 2026-10-28: Independent native source mod; no changes to either reference checkout. Online product exclusions are exactly those approved in plan.md. Local behavior remains in scope.
- Both reference working trees were clean at start. Vape reference: `3d600a48106822755cea2d633062e5ab1f383d48`. Horizon target: `e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083`, Minecraft 26.3 / protocol 777.
- Mixin tool: `0500ff24aa5933fca95a877ef41125353c4b4fe3` (0.5.0, schema 5). Tool itself builds on its pinned Rust 1.95.0. Mod/host development uses installed Rust 1.99.0; launcher's release toolchain is separately pinned to 1.99.1. Do not claim these are interchangeable release proofs.
- Inventory tooling reads Java as text only. `docs/discovery.json` preserves source hashes and declaration line numbers. `docs/features.csv` is the working ledger. Automatic discovery does not prove complete defaults, inheritance, conditions, submodes or behavior. Helper classes are included to avoid dropping them; classification and cross-cutting decomposition remain open.
- Runtime sources remain modular. A Rust AST packer will inline only conventional module declarations into a self-contained native payload, with module-to-source hashes. Standalone tests must also run on emitted source.
- First vertical slice: client-only tick diagnostic, explicit keyboard toggles, coordinates/FPS HUD. This establishes host integration; it does not establish UI parity or completed feature rows.
- Public release policy: stable-release compatibility with continuous main-source drift detection. Pins never move within an existing release. Publication, registry submission and distribution rights require completed validation/provenance evidence.

- User authorized using upstream horizon-mixins PR #2 before merge. Updated development tool pin to `29a8f8f84b253ea89422c0049ff5e8bfda36f1fb` and moved the input hook to shared `debug_key`. This supersedes the original tool pin and resolves the tooling choice; the Horizon target stays fixed. Validate full native staging/build/scenarios before claiming integration.
