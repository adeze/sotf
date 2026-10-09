---
title: generate-audio-tests
description: Generate WAV test files for audio validation across signal types, channel counts, sample rates, and bit depths.
---

Generates a battery of WAV test files covering all combinations of signal type, channel
count, sample rate, and bit depth. Each file is accompanied by a JSON sidecar with
signal metadata, and a `manifest.json` index is written at the end.

## Synopsis

```
generate-audio-tests [OPTIONS]
```

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--out-dir` | `data_generated/test-audio` | Output directory |
| `--channels` | `1,2,6,16` | Channel counts (comma-separated; mono, stereo, 5.1, 9.1.6) |
| `--sample-rates` | `44100,48000,96000` | Sample rates in Hz (comma-separated) |
| `--bits` | `16,24` | Bit depths (16 or 24 only) |
| `--signals` | *(all except `full-sti`)* | Signal types to generate (comma-separated; see below) |
| `--duration` | `10.0` | Duration in seconds (per-segment for `full-sti`; fixed at 30 s for `sweep`) |
| `--sti-gap` | `0.0` | Silence gap in seconds between `full-sti` segments (ignored otherwise) |

## Signal Types (`--signals`)

Command-line values use kebab-case; output directories and file stems
use snake_case (`pink-noise` → `pink_noise/…`).

| Value | Description |
|-------|-------------|
| `id` | Per-channel identification tones — each channel gets a unique frequency (300, 600, … 6000 Hz). Useful to verify channel routing. |
| `thd1k` | Single 1 kHz sine at −3 dBFS. Standard signal for Total Harmonic Distortion measurement. |
| `thd100` | Single 100 Hz sine at −3 dBFS. Low-frequency THD measurement. |
| `imd-smpte` | SMPTE IMD test: 60 Hz + 7 kHz at 4:1 amplitude ratio. |
| `imd-ccif` | CCIF IMD test: 19 kHz + 20 kHz at equal amplitudes. |
| `sweep` | Logarithmic frequency sweep 20 Hz → 20 kHz, 30 s fixed duration. Use for impulse response and frequency response measurements. |
| `white-noise` | White noise (flat spectrum). |
| `pink-noise` | Pink noise (1/f spectrum, −3 dB/octave). |
| `m-noise` | M-weighted noise per ITU-R 468. Standard for acoustic measurement. |
| `stipa` | STIPA direct-method test signal per IEC 60268-16: dual-modulated octave-band noise, RMS 0.07. Sample rates below 22050 Hz are skipped. |
| `full-sti` | Full-STI direct-method signal: 98 concatenated single-modulation segments (14 modulations × 7 octave bands) in modulation-major order with `--sti-gap` silence between them. Opt-in only — at 10 s per segment one file already holds 980 s. Sample rates below 22050 Hz are skipped. |

## Output Layout

```
data_generated/test-audio/
  wav/
    id/
      id_ch1_sr44100_b16.wav
      id_ch1_sr44100_b16.wav.json   ← sidecar with signal metadata
      id_ch2_sr48000_b24.wav
      …
    sweep/
      sweep_ch2_sr96000_b24.wav
      …
    pink_noise/
      …
  manifest.json                     ← index of all generated files
```

Files that would violate Nyquist (e.g., a 20 kHz sweep at 22050 Hz sample rate) are
skipped automatically and counted in the summary.

## Examples

Generate everything (default):
```bash
generate-audio-tests
```

Stereo only, 48 kHz, 24-bit, sweep + pink noise:
```bash
generate-audio-tests \
  --channels 2 \
  --sample-rates 48000 \
  --bits 24 \
  --signals sweep,pink-noise
```

Full STI with 4 s segments and 1 s gaps (reference-style measurement set):
```bash
generate-audio-tests \
  --channels 1 \
  --sample-rates 48000 \
  --bits 24 \
  --signals full-sti \
  --duration 4 \
  --sti-gap 1
```

Custom output directory and duration:
```bash
generate-audio-tests --out-dir /tmp/audio-tests --duration 5.0
```

## Sidecar Format

Each WAV file has a companion `.wav.json` with structured metadata:

```json
{
  "format": "wav",
  "channels": 2,
  "sample_rate": 48000,
  "bits": 24,
  "duration": 10.0,
  "signal": {
    "type": "pink_noise",
    "description": "1/f spectrum, -3dB/octave (pink noise)"
  }
}
```

## Use Cases

- **Engine regression tests** — run the full plugin chain on known signals and compare output
- **Room measurement** — use `sweep` files as the test signal when recording room impulse responses
- **Channel routing verification** — use `id` tones to confirm each channel reaches the correct speaker
- **Codec/format testing** — spot-check encoder/decoder fidelity across sample rates and bit depths
