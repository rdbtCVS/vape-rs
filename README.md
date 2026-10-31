# vape-rs

A from-scratch recreation of the **Vape** Minecraft client as a native Rust mod for the [Horizon](https://github.com/horizon-rust/horizon) client (Minecraft 26.3). It is not Java, not injected and has no accounts or online services: it runs locally as a Horizon native mod, and all settings, profiles, friends and macros stay on your machine.

> **Status: work in progress.** Many modules exist, but only some have been validated in a real game, and visual/behavioral parity with the original is not claimed. See [docs/status.md](docs/status.md) and [docs/progress.md](docs/progress.md).

## What works today

- **HUD:** eleven widgets (incl. Text GUI module list, Scoreboard, Target Info) with an in-game layout editor.
- **Combat:** AimAssist, TriggerBot, KillAura, SilentAura, KeepSprint, Velocity, clickers, WTap, JumpReset, HitBoxes (Reach needs host support).
- **Movement:** Sprint, SafeWalk, Scaffold (Legit), InvWalk, Parkour (experimental), Speed, LongJump, Fly, NoFall, NoSlowdown, Step, AntiFall.
- **Inventory:** Refill, ChestSteal, Throwpot, ThrowDebuff, AutoHeal (Legit), AutoTotem (partial), InventoryManager/InvCleaner/AutoHotbar/ArmorSwitch (untested).
- **World / utility:** AutoTool, FastPlace, AutoFish, WindCharge, Blink, Backtrack, FakeLag, KnockbackDelay, NoJumpDelay, NoClickDelay, Panic.
- **Render:** Fullbright, Clear Water, NoHurtCam, ItemESP, NameTags, StorageESP, Time Changer, Weather, AntiDebuff; ESP, Tracers, Trajectories, Search and others are untested.
- **Client:** click GUI (Right Shift), module settings and key binding, local profiles, friends/enemies, macros.

Modules start disabled. F10 turns every active module off. AI behavior and the online product are intentionally not part of this project.

## Install

Requires Horizon's launcher (0.2.27+) with a Nightly installed for Minecraft 26.3.

```sh
python3 tools/bundle.py                      # builds dist/vape_rs-<version>.hnmod
mkdir -p ~/vape_rs && cd ~/vape_rs && unzip /path/to/vape_rs-0.1.0-dev.hnmod
horizon-launcher native-build ~/vape_rs --trust-native-code
```

The first build downloads Rust and takes 20–30 minutes. Native mods run with your full OS privileges, so read the source first. Only Linux x86-64 has been exercised.

## Building and contributing

See [docs/development.md](docs/development.md). You need Rust 1.99.0 (pinned in `rust-toolchain.toml`, required by Horizon). Never run tests against real player data.

## Disclaimer

This is an independent, unofficial project. It is **not affiliated with or endorsed by** the makers of Vape, Mojang or Microsoft, and contains no code or assets from them. "Minecraft" is a trademark of Mojang AB. Using client modifications on multiplayer servers can get you banned; use it where it is allowed, and at your own risk.

## License

[MIT](LICENSE).
