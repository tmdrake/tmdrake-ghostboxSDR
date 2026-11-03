# Drivers

Drake's Box SDR does not ship a kernel driver. The stick is a USB device. The app talks to it from user space with the vendored `rs-rtl` crate (`nusb`). There is no `rtlsdr.dll` to copy and no `.sys` to build.

Windows and Linux disagree about who owns that USB device. These notes are the map for both, and for a later Raspberry Pi port.

## What the stick actually is

A cheap “RTL-SDR” is a USB DVB-T TV tuner built around an RTL2832U. The tuner chip in front of it is not always an R820T.

This machine’s stick, after the WinUSB bind:

| Node | Id | Driver | Role |
|---|---|---|---|
| Composite parent | `USB\VID_0CCD&PID_00D7\…` | `usbccgp` | Leave it. Windows splits the functions. |
| Bulk-In Interface 0 | `…&MI_00` | **WinUSB** | The radio. IQ samples. |
| Interface 1 | `…&MI_01` | not WinUSB | IR receiver. Leave it. |

The USB product string on that unit was `RTL2838UHIDIR`. The tuner behind it is an **Elonics E4000**, not an R820T. `rs-rtl` 0.5.1 only opened `0bda:2832` and `0bda:2838`, so the stick looked missing until the vendored ID table and `vendor/rs-rtl/src/e4k.rs` were added.

E4000 check: I2C address `0xc8`, register `0x02` reads `0x40`. It is zero-IF. The app clamps it to about **52 MHz–1700 MHz**. FM broadcast is in range. HF and the generic R820T floor of ~24 MHz are not.

Manual gain steps are **−1.0 to 42.0 dB** (LNA plus mixer). The six IF stages stay at a fixed moderate gain and are not on the slider. AGC is forced off. A local FM station was clean near **11.5 dB**; the default **29.6 dB** is for a weak signal and overloads that same station. Each hop writes the PLL, then the band bits and the RF filter. The IF filters are set once, to the sample rate. The **Full range** control sweeps this whole clamp, and on a span wider than 30 MHz the LO retunes every dwell.

R820T / R820T2 stays the usual cheap stick (`0bda:2838`). R828D is the Blog V4 path and is the one that can do HF. Do not treat every `0bda` device as a radio. Realtek Wi-Fi uses the same vendor id and other product ids.

## Two ID lists that must stay in step

The Windows package and the Rust opener do not use the same list yet.

Windows match strings in `packaging/windows/install-winusb.ps1` (`$RtlVidPid`):

`0BDA:2831`, `0BDA:2832`, `0BDA:2838`, `0CCD:00A9`, `0CCD:00B3`, `0CCD:00D3`, `0CCD:00D7`, `0CCD:00E0`, `1D50:6044`, `1D50:6089`, `1F4D:B803`, `1F4D:C803`, `1209:2832`.

Rust `KNOWN_RTL_IDS` in `vendor/rs-rtl/src/rtlsdr.rs` also includes the other Terratec ids `00B4`, `00B5`, `00B7`, `00B8`, `00B9`, `00C0`, `00C6`.

A Linux udev rule should follow the Rust list, or a stick the app can open will still be permission-denied. When one list grows, update the other and this file.

## Windows

Windows already has `winusb.sys`. The stock DVB-T driver, or no driver at all, sits on Bulk-In Interface 0 and blocks `nusb`. The package only binds the inbox WinUSB driver to that interface.

Run it from the **repo root**, not from `target\release`:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\install-winusb.ps1
powershell -ExecutionPolicy Bypass -File packaging\windows\install-winusb.ps1 -Status
powershell -ExecutionPolicy Bypass -File packaging\windows\install-winusb.ps1 -Check
```

From a built binary, `--list` and `--setup-driver` walk up from the exe and find the same script.

`-Check` prints one token and is what startup uses:

| Token | Exit | Meaning |
|---|---|---|
| `READY` | 0 | Interface 0 already has WinUSB, or no radio interface needs it. |
| `NEED` | 2 | An RTL-like radio interface is present and is not WinUSB. |
| `ABSENT` | 0 | No matching dongle. |

`-Status` is for humans. The composite parent and Interface 1 can print `NEED WINUSB` even when `-Check` says `READY`. Trust `-Check` for the radio.

On `NEED`, startup downloads official Zadig 2.9 if it is not cached and opens it. The file is SHA-256 `4ECAA95DF3DA3621486A043AEF8B3050B8BAFE7C901402871E816229EF82039B`, from `https://github.com/pbatard/libwdi/releases/download/v1.5.1/zadig-2.9.exe`, cached at `%LOCALAPPDATA%\ShackHack\tools\zadig.exe`. A `zadig.ini` next to it selects WinUSB and, when Interface 0 is visible, presets that VID/PID and `MI = 0x00`.

In Zadig: **Bulk-In, Interface (Interface 0)** → target **WinUSB** → **Replace Driver**. Then unplug and replug.

Windows still asks for Administrator. Choosing **No** is not a failed script. Zadig opens anyway, and **Replace Driver** asks again. Do not bind the composite parent, Interface 1, or a Wi-Fi adapter.

PowerShell 5.1 is enough (`#Requires -Version 5.1`). A missing-file error on `-File` means the current directory is not the repo root.

`--synth` does not touch USB and does not open Zadig.

## Linux and Raspberry Pi

No Zadig and no WinUSB. The same `nusb` open claims **USB interface 0** on `/dev/bus/usb`. Two things stop that:

1. The kernel DVB module `dvb_usb_rtl28xxu` grabs the stick as a TV tuner. Unload it and blacklist it (`rtl2832` comes along with that stack). After a blacklist, replug the stick.
2. The device node is root-only until a udev rule matches the vendor and product ids. `MODE="0666"` or `GROUP="plugdev"` on those ids is the whole “driver package” on Linux.

Interface 1 does not need a rule of its own if the rule is on the USB device. Do not write a rule that matches every `0bda` device.

Audio on the Pi is ALSA through `cpal`: the headphone jack or HDMI, not a Windows WASAPI device. A dead default device looks like a silent scan, not a USB failure.

The HUD is `eframe` / `wgpu`. The window asks for at least 960×600 and opens at 1280×800. If `wgpu` fails on the Pi GPU, switch that binary to the `glow` backend before rewriting the UI.

### Bench hardware that is not the port target

A 2014 board (Model B or Model B+, one 700 MHz ARM11, 512 MB, no NEON) plus a Waveshare screen stacked on the GPIO header is not enough for this app. Those panels are usually SPI, 320×240 or 480×320, and they share the only core with the display. A 32 GB card is plenty of storage and is not the limit. The dongle plus that panel also wants a 5 V supply of 2 A or more.

The port target is a **Pi 4 or Pi 5**, 64-bit Raspberry Pi OS, a powered USB port, and a display at least about 1024×600. Prove `--synth` there before a live stick. Live IQ is 2.4 MS/s, about 4.8 MB/s of samples, plus the FFT and eight VFOs. Watch dropped USB buffers. A Pi 3 and an unpowered hub stay out of scope.

Order of work when the port starts: Linux desktop udev and the DVB blacklist, then the same image on a Pi 4/5, then audio, then the GPU backend if `wgpu` misbehaves. The E4000 and R820T code is already in the crate. It does not get a second Windows-style installer. The sweep is the same contract on every OS. It is written up in [sweep.md](sweep.md), and the port does not grow a second copy of it.

## What we still do not build

A custom kernel driver, a signed catalog, `rtlsdr.dll`, P25/DMR, or a squelch that stops the sweep.
