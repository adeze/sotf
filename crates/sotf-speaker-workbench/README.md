# Speaker Workbench

An offline SOTF workbench embedding Sonium's measured passive crossover editor.
Sonium retains circuit, driver and enclosure numerical ownership. The separate
active preview runs SOTF's existing `CrossoverPlugin`; it does not translate
passive component values into DSP settings or alter the player's processing.

Use the populated Adeze `all_of_sotf` checkout, including its `sonium` and
`gpui-toolkit` source dependencies. From the umbrella root:

```sh
rustup run stable cargo run --manifest-path sotf/Cargo.toml -p sotf-gpui --bin sotf-desktop --features speaker-workbench
```

The player feature adds **View → Speaker Workbench** and a Studio navigation
entry, embedded in the player's main content view. Its editor entity survives
navigation between screens.
The feature is opt-in to keep Sonium's numerical dependencies out of normal
player builds. This crate provides a reusable view, not a separate application.

Import woofer/tweeter FRD and ZMA files in Sonium's editor to evaluate its passive
network. The active LR24 preview needs both complex FRD responses, shared
measurement references and overlapping frequency support below Nyquist. It uses
the same driver delay/polarity and imports, with its own cutoff and sample rate.
Changing the editor or active settings invalidates the active chart; recalculate
explicitly. Invalid or incomplete data produces an error, not synthetic fill.

The response chart combines measured complex driver responses with a finite
impulse measurement of the actual two-output plugin. Ideal voltage-source
amplifiers are assumed. This does not predict active-system input impedance,
distortion, protection, room behavior or safe driver routing. No audio is played.

**Copy crossover parameters** copies only the sample rate and native electrical
plugin parameters. It does not apply them or include driver delay, polarity,
output routing, measurement data or a complete deployment preset. Sonium's
project save/load retains passive design/imports; active settings are currently
session-local. Sonium's existing unsaved-change-close limitation also applies.

```sh
rustup run stable cargo test --manifest-path sotf/Cargo.toml -p sotf-speaker-workbench --lib
rustup run stable cargo build --manifest-path sotf/Cargo.toml -p sotf-speaker-workbench --features desktop
rustup run stable cargo check --manifest-path sotf/Cargo.toml -p sotf-gpui --bin sotf-desktop --features speaker-workbench
```
