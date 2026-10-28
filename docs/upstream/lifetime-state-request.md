# Per-instance state on lifetime-only generic owners

Horizon pin: `e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083`.
Mixin pin (PR #2 head): `29a8f8f84b253ea89422c0049ff5e8bfda36f1fb`.

The client is `game::Game<'a>`: its lifetime borrows prepared resources. A native mod needs owned runtime state attached to each client instance, independently of other clients and the integrated server. The intended field is an owned `RefCell<Runtime>` initialized with Default, with no captures or borrowed host references. A thread-local runtime is only an initial integration scaffold and cannot satisfy that ownership contract for multiple sequential/simultaneous Game instances.

A minimal state declaration targeting `horizon_client::game::Game`, with `std::cell::RefCell<u64>` as a dependency-free stand-in, is rejected during planning with `E_BRIDGE_TARGET: generic bridge owner types are unsupported`. The shared owner validator rejects lifetime parameters together with type/const generics. No transformation is applied.

The adjacent `game::chat::UiState` is nongeneric and lives beside each Game, but all its fields are public. It is correctly rejected by the existing construction guard (`E_STATE: state requires a private field or non_exhaustive struct; external construction would break`). Weakening that guard or editing the Horizon checkout is not proposed.

Please support state owners with lifetime parameters only, preserving lifetime parameters on generated impls/accessors and respecting constructor resolution and initialization order. Keep unsupported type/const generics, unresolved cfg/layout attributes, hidden construction and unsafe/public construction fail-closed. Ordinary lifetime-only Game methods already resolve for HEAD hooks; this request concerns owned instance fields and accessors.

Regression coverage should demonstrate two independent instances, initialization once per construction, ordinary moves/drops, multiple declared constructors, struct updates retaining state, and negative coverage for type/const generics and existing construction/layout guards. An actual staged Horizon compile remains necessary after fixture success.

No issue or upstream message has been sent for this request. Local minimal probe manifests/diagnostics are recorded in `docs/evidence/lifecycle-state-probes.json`.
