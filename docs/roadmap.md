# Roadmap

ShackHack v1 is a Windows desktop spirit box: analog AM/NFM/WFM, parked RTL LO, software VFOs, night-vision HUD. This is the later work, not a promise of dates.

## Now

- Windows GUI via `run.cmd`
- WinUSB on Bulk-In Interface 0, opened at startup when that interface needs it
- `--synth` when no dongle is attached
- GitHub CI on Ubuntu (fmt, test, clippy) so the crates already build on Linux

## Next: Linux desktop

The radio crates do not call WinUSB. `rs-rtl` talks USB through `nusb`, and audio is `cpal`.

1. Add a `packaging/linux/` udev rule for the same VID/PID list the Windows package uses (RTL2832/2838, TerraTec including `0CCD:00D7`, Blog/ezcap). Mode `0666` or a `plugdev` group, and skip Interface 1.
2. Confirm `cargo run -p shackhack -- --synth` on Ubuntu, then a live stick.
3. Document ALSA/PipeWire. No Zadig, no `.sys`.

## Later: Raspberry Pi

Target a Pi 4 or Pi 5 running 64-bit Raspberry Pi OS, with the same cheap RTL-SDR on a powered USB port.

1. Cross-check the Ubuntu desktop build on `aarch64`. The DSP and band crates should move as-is.
2. Prove `--synth` on the Pi desktop, then live IQ at 2.4 MS/s. Watch USB power and dropped buffers. A Pi 3 or an unpowered hub is out of scope.
3. Audio out the 3.5 mm jack or HDMI. Keep the same sweep, gain slider, and four contacts.
4. The HUD uses `eframe` / `wgpu`. If the Pi GPU path is unhappy, switch that binary to the `glow` backend before rewriting the UI.
5. Package it as a script plus the udev rule. A full disk image can wait until the desktop build is boring.

Headless sweep-plus-audio (no window) is optional after the Pi desktop works. It is not a substitute for the HUD.

## Still out

P25, DMR, a squelch that stops the sweep, and HF on a generic R820T without a Blog V4 or upconverter.
