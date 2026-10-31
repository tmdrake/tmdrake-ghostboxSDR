# Windows WinUSB package

ShackHack does **not** build a kernel driver. Windows already includes `winusb.sys`. Cheap RTL-SDR sticks ship as DVB-T TV tuners, so Windows binds the Realtek TV driver (or nothing) to the bulk endpoint. SDR apps need **WinUSB** on **Bulk-In Interface 0** (`MI_00`).

This folder is the package that does that bind.

## Install

Plug the dongle in, then from the repo root:

```powershell
powershell -ExecutionPolicy Bypass -File packaging/windows/install-winusb.ps1
```

Or from a built binary:

```powershell
.\shackhack.exe --list
.\shackhack.exe --setup-driver
```

The script:

1. Lists RTL-like USB devices and whether `WinUSB` is already the service on interface 0
2. Downloads official **Zadig 2.9** (SHA-256 pinned) into `%LOCALAPPDATA%\ShackHack\tools\`
3. Writes a `zadig.ini` that defaults to WinUSB and “list all devices”
4. Launches Zadig elevated

In Zadig: **Bulk-In, Interface (Interface 0)** → **WinUSB** → Replace Driver. Unplug and replug. Then run ShackHack.

The app runs the same check at startup (`-Check`). `NEED` opens Zadig elevated and, when Interface 0 is visible, presets its VID/PID. `READY` and `ABSENT` do not. Windows will not install a driver with no confirmation.

Interface 1 is the IR receiver. Leave it.

## What this is not

- Not `rtlsdr.dll` / osmocom `librtlsdr` — ShackHack talks USB itself (`rs-rtl` / `nusb`)
- Not a custom `.sys` you compile with the WDK
- Not a bind for Realtek **wifi** sticks (same vendor id `0BDA`, different PID). The package only matches RTL2832/2838, TerraTec DVB (`0CCD`, including `00D7`), and a few Blog/ezcap IDs.

## If Zadig is already installed

```powershell
powershell -ExecutionPolicy Bypass -File packaging/windows/install-winusb.ps1 -Status
```

READY on the radio interface means you can skip Zadig and start the app.
