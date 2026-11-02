# Roadmap

Drake's Box SDR v1 is a Windows desktop ghost box: analog AM/NFM/WFM, parked RTL LO, software VFOs, dark phosphor HUD. Driver facts for a port are in [drivers.md](drivers.md). This is the later work, not a promise of dates.

## Now

- Windows GUI via `Start.vbs` / `run.cmd`. The sweep starts on open.
- Controls persist in the settings file (dwell, gain, band, full range, VFOs).
- Listen parks V1 on a typed frequency. Full range sweeps the stick's real limits, hopping the LO each step.
- Spectrum is an averaged 45 dB window with peak-hold, so weak carriers stay visible.
- WinUSB on Bulk-In Interface 0, opened at startup when `-Check` prints `NEED`
- Terratec `0CCD:00D7` is an E4000 (`vendor/rs-rtl/src/e4k.rs`). The slider is the LNA plus mixer, −1.0 to 42.0 dB. A hop writes the PLL, the band, and the RF filter.
- `--synth` when no dongle is attached
- GitHub CI on Ubuntu (fmt, test, clippy) so the crates already build on Linux

## Next: Linux desktop, then a Pi 4 or Pi 5

The radio crates do not call WinUSB. `rs-rtl` talks USB through `nusb`, and audio is `cpal`.

1. Add `packaging/linux/`: a udev rule for every id in `KNOWN_RTL_IDS`, plus a modprobe blacklist for `dvb_usb_rtl28xxu`. See [drivers.md](drivers.md). No Zadig and no `.sys`.
2. Confirm `cargo run -p shackhack -- --synth` on Ubuntu, then a live stick (R820T and the E4000).
3. Move that same userspace build to 64-bit Raspberry Pi OS on a Pi 4 or Pi 5. Audio out the headphone jack or HDMI.
4. If `wgpu` fails on the Pi GPU, switch that binary to `glow` before rewriting the UI.
5. A disk image can wait until that desktop build is boring.

The bench board from 2014 (Model B / B+, 512 MB, one ARM11) with a Waveshare GPIO screen is recorded in [drivers.md](drivers.md). It is not this port. A Pi 3 and an unpowered hub stay out of scope.

Headless sweep-plus-audio is optional after the Pi desktop works.

## Still out

P25, DMR, a squelch that stops the sweep, and HF on a generic R820T without a Blog V4 or upconverter.
