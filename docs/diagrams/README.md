# Diagrams

Assets used by the root [README](../../README.md).

| File | What |
|---|---|
| [cheap-rtl-sdr.jpg](cheap-rtl-sdr.jpg) | Cheap generic USB RTL-SDR with a whip |
| [signal-path.svg](signal-path.svg) | Antenna → dongle → WinUSB → parked LO → contacts → audio / NV waterfall |
| [lo-window.svg](lo-window.svg) | 2.4 MHz capture, 10% edge crop, V1 enters 840 kHz below the LO, PLL hop at the far edge |

The SVGs are the labeled diagrams. Edit those files when the architecture changes; keep the numbers in sync with `shackhack-dsp` (`EDGE_CROP = 0.10`, `DEFAULT_SAMPLE_RATE = 2_400_000`) and `shackhack-bands` (FM 88–108, default gain 29.6 dB).
