# ShackHack

RTL-SDR analog **ghost box** / shack-hack sweeper. Fast software-VFO scan across a band, always-on audio (no squelch), SDR#-style waterfall, and four VFOs inside the live 2.4 MHz window.

Public repo: [tmdrake/tmdrake-ghostboxSDR](https://github.com/tmdrake/tmdrake-ghostboxSDR)

## What it is

A Radio Shack-style spirit box races through FM and never stops. ShackHack does that on a cheap RTL-SDR, but the tuner stays parked on a 2.4 MHz chunk and a **software VFO** walks the channels. The PLL only hops when the sweep walks off the window, so tuning looks smooth and the audio click is short.

Default shack-hack: FM broadcast **88–108 MHz**, **WFM**, **150 kHz** steps, **30 ms** dwell, **bounce**.

## Hardware floor (no upconverter)

| Stick | Range |
|---|---|
| Generic R820T / R820T2 / Nooelec | **~24 MHz – 1766 MHz** |
| RTL-SDR Blog V4 | **~500 kHz – 1766 MHz** (built-in HF) |

AM broadcast and HF ham need HF (V4 or a V3 in direct sampling). Lowest analog **voice** on a generic stick is **VHF-low 30–50 MHz**.

Instantaneous bandwidth is ~2.4 MHz. Multi-VFO is several software slices of that same window (SDR# Sharp Slicer is Airspy-only).

## Gain

The R820T has about **0–49.6 dB** of stepped RF gain. ShackHack exposes it as a **Gain (dB)** slider on the top bar and snaps to the tuner's real steps.

- Start around **25–30 dB**.
- If the spectrum goes solid yellow/red or you see **OVERLOAD**, drop gain.
- Tuner AGC is not used on FM broadcast; strong stations will slam the ADC.

## Windows driver (Zadig)

1. Plug in the dongle.
2. Run [Zadig](https://zadig.akeo.ie/) as Administrator.
3. **Options → List All Devices**.
4. Select **Bulk-In, Interface (Interface 0)** (not the composite parent).
5. Target driver: **WinUSB** → Replace Driver.

See [docs/zadig.md](docs/zadig.md).

## Build

Needs Rust (`rustup`) and, on Windows, the MSVC build tools (for `eframe` / `wgpu`).

```powershell
cargo run --release -p shackhack
```

No dongle? Fake stations:

```powershell
cargo run --release -p shackhack -- --synth
```

## Controls

- **Sweep / Stop** — ghost-box motion vs listen
- **Band** — FM, airband, VHF, NOAA, GMRS, 70 cm, …
- **Mode** — bounce, forward, backward, random
- **Dwell / Step** — shack-hack speed
- **Gain** — RF gain in dB
- Click the waterfall while sweeping to **park VFO B** on a signal and mix it in
- Stop the sweep, then click to tune **VFO A**

## Analog voice presets (US)

See [docs/bands.md](docs/bands.md).

## Disclaimer

Receive only. No digital decode, no encryption. Ghost-box / ITC use is entertainment. Check local law before monitoring public-safety radio.

## License

MIT
