# Local macros

Macros are profile-local data and never contact an account, cloud profile, or
backend. The static reference creates three action classes from a macro name:
names starting with `/` send a normal in-game command; other names locate
the matching hotbar item and use it; `fishing rod` is the item form with its
hook-ready shortcut for the optional second click.

The retained engine stores an ordered, bounded set of at most 256 macros. Each
has a unique case-insensitive name, one to three physical-key bindings, a
1–1000 ms delay range, and optional double-click delay range. It serializes to
a `macros` JSON array suitable for the existing local profile document. It
does not import legacy encoded names or old numeric key codes: the Horizon
runtime supplies named physical keys.

Only physical press edges start an action after its complete chord is held;
repeat events and engine-emitted effects never re-trigger macros. A command
emits a typed chat effect on the following tick. An item action emits ordinary
host hotbar selection and use-down/use-up effects, waits for its configured
delay, and restores the prior selected slot. Replacing an active item action
inherits its original slot like the reference.

The host must apply effects through its usual chat, selected-hotbar and use-key
paths, and must not route those synthetic effects back into macro input. It
must call `cancel` on disable, panic, GUI/context loss, profile replacement,
world/session transitions and client teardown; cancellation clears held input,
releases use if owned, and restores the saved hotbar slot. This adds safe
cleanup where the legacy action's bare cancellation could leave state behind.
