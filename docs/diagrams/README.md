# Diagrams

Assets used by the root [README](../../README.md).

| File | What |
|---|---|
| [icon.png](icon.png) | App icon — phosphor stick figure + cheap dongle |
| [cheap-rtl-sdr.jpg](cheap-rtl-sdr.jpg) | Cheap generic USB RTL-SDR with a whip |
| [signal-path.svg](signal-path.svg) | Antenna → dongle → WinUSB → parked LO → contacts → audio / NV waterfall |
| [lo-window.svg](lo-window.svg) | 2.4 MHz capture, 10% edge crop, software VFO walk, PLL hop at the edge |

The SVGs are the labeled diagrams. Edit those files when the architecture changes; keep the numbers in sync with `shackhack-dsp` (`EDGE_CROP = 0.10`, `DEFAULT_SAMPLE_RATE = 2_400_000`) and `shackhack-bands` (FM 88–108, default gain 29.6 dB).
