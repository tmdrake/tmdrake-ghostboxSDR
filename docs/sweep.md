# Sweep

The ghost-box scan. Frank's Box leaves the audio open and walks a band. This file is the contract for that walk. A Linux or Pi port keeps this behavior. It does not get a second sweeper.

The math is in `crates/shackhack-dsp/src/sweep.rs` and `crates/shackhack-dsp/src/window.rs`. The radio thread in `crates/shackhack-sdr/src/engine.rs` is what hops the stick. The HUD in `crates/shackhack/src/app.rs` only draws the mark and the **up** / **down** label. None of that calls WinUSB. `cargo test -p shackhack-dsp -- sweep::` locks it, and that test does not need a dongle or Windows.

## What a pass does

V1 is the heard cursor. Each dwell it moves by **Step**, then waits **Dwell** (10–500 ms). The big digits glide across that one step. A jump bigger than four steps, and at least 5 MHz, snaps instead of gliding, so a wrap does not smear the readout across the whole span for one dwell.

The range bar is the configured min–max. The bright mark on it is V1, not the 2.4 MHz LO window. On a full-range bar that window is about a pixel wide, which is why the mark exists.

| Mode | Start of a pass | At the far end | Label |
|---|---|---|---|
| Bounce | Bottom, going up | Sits on the top for one dwell, then comes back down. Sits on the bottom, then goes up again. | **up** or **down** |
| Forward | Bottom, going up | Sits on the top for one dwell, then wraps to the bottom. | **up** |
| Backward | Top, going down | Sits on the bottom for one dwell, then wraps to the top. | **down** |
| Random | Bottom | Jumps inside the span. Does not walk. | hidden |

"Sits on" means the readout equals the configured end, even when the step does not divide the span. A 150 kHz step on 52–1700 MHz still lands on 1700.000 MHz. The next dwell is the wrap or the turnaround.

Switching mode sets the direction immediately. Bounce keeps the direction it already had. On a short band the LO is re-parked for the new direction. On a span wider than 30 MHz the LO stays on the current step.

## Two ways the LO moves

The stick captures 2.4 MHz. Each edge crops 10%, so a station has to stay inside **LO ± 960 kHz**.

- **Span of 30 MHz or less** (FM, air, marine, NOAA). The LO stays parked. Sweeping up, a hop places it **840 kHz above** V1. Sweeping down mirrors that. The next hop happens when V1 leaves the usable middle. Eight software VFOs walk inside the window.
- **Span wider than 30 MHz**, including **Full range**. The LO hops onto every step, centered on that step, then clamped so the whole 2.4 MHz window stays inside the tuner (`clamp_lo`: half a sample-rate in from each tuner edge). The sweep frequency still lands on the configured ends. The center of the window cannot, because the tuner will not tune a window that hangs off the end.

A hop drops 1–2 USB buffers and fades the audio for a few milliseconds. Retuning every 150 kHz on a short band is what clicks. That is why short bands park.

## Full range

**Full range** is not a preset row. It copies the stick's own limits into the sweep:

| Tuner | Low | High |
|---|---|---|
| E4000 (this Terratec `0CCD:00D7`) | 52 MHz | 1700 MHz |
| Generic R820T | 24 MHz | 1766 MHz |
| Blog V4 | 500 kHz | 1766 MHz |

`Sweeper::restart` runs when that range is applied. Forward and bounce start at the low end going up. Backward starts at the high end going down.

The step is resized so one direction takes about **90 seconds**. The hop stays between **250 kHz** and **2 MHz** (one RTL window, so neighboring slices still overlap) and is rounded to 50 kHz. A step the user already set is kept when it is at least 100 kHz and that pass already finishes in under three minutes. Dwell and demod are not changed.

A 150 kHz step at 90 ms dwell is about sixteen minutes one way on 52–1700 MHz. The scan is moving, but it looks stuck, and bounce does not look like it turns around. That is why Full range raises the step. Dragging **Step** back down is allowed. The ends are still hit. The pass just takes longer.

## Bugs a port must not bring back

These all shipped once. The tests above fail if they come back.

- Casting a downward overshoot through `u64` wraps it to a huge frequency and snaps the scan to the top.
- Forward that wraps as soon as the next step would pass the top never sits on the top when the step does not divide the span.
- Backward that wraps when `freq <= min + step` turns around one step early and never sits on the bottom.
- A wide span that parks the LO for the whole 2.4 MHz window looks like the radio is not tuning. Hop every step above 30 MHz.
- Treating the LO readout as the sweep end. `clamp_lo` holds the LO half a sample-rate inside the tuner. V1 is the end.

## What the Linux port reuses

Keep `shackhack-dsp` and `shackhack-sdr` as they are. The Pi build is the same userspace binary once USB and the window open. Settings already load from `~/.local/share/ShackHack/settings.txt`, and `mode` is `bounce`, `forward`, `backward`, or `random`.

Do not reimplement the walk in the UI, and do not add a squelch that stops it. USB, udev, and the DVB blacklist are [drivers.md](drivers.md). The order of the port is [roadmap.md](roadmap.md).
