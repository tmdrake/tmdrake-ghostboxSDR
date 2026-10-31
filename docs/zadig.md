# Windows WinUSB package

You do **not** compile a kernel driver for these sticks. Windows already ships `winusb.sys`. The stock DVB-T driver (or a missing driver) is what blocks ShackHack. Bind WinUSB to **Bulk-In Interface 0** (`MI_00`) only.

ShackHack talks to the stick with `rs-rtl` (pure Rust, `nusb`). There is no `rtlsdr.dll` to copy.

## Package (preferred)

From the repo:

```powershell
powershell -ExecutionPolicy Bypass -File packaging/windows/install-winusb.ps1
```

```powershell
cargo run --release -p shackhack -- --list
cargo run --release -p shackhack -- --setup-driver
```

The package lists RTL-like USB devices, downloads official [Zadig 2.9](https://github.com/pbatard/libwdi/releases/download/v1.5.1/zadig-2.9.exe) (SHA-256 pinned) into `%LOCALAPPDATA%\ShackHack\tools\`, and launches it elevated. Details: [packaging/windows/README.md](../packaging/windows/README.md).

In the GUI, a missing driver shows **Install WinUSB package…**. Startup does the same check: if the radio interface needs WinUSB, Zadig opens on its own. You still confirm the admin prompt and click **Replace Driver**.

## What to click in Zadig

1. Plug in the dongle.
2. **Options → List All Devices**.
3. Pick **Bulk-In, Interface (Interface 0)**. VID is often `0BDA` / PID `2832` or `2838`. TerraTec-style sticks use `0CCD` (this repo has seen `0CCD:00D7`).
4. Target: **WinUSB**.
5. **Replace Driver** / **Install Driver**.
6. Unplug and replug.
7. `shackhack --list` should show READY on the radio interface.

Do not bind WinUSB to a random USB device, the composite parent, or Interface 1 (IR). Do not bind Realtek **wifi** adapters (same vendor `0BDA`, different PID).

If the list is empty, the dongle is not enumerated — try another port, avoid unpowered hubs.

Blog V4 boards are detected later by USB strings (`RTLSDRBlog` / `Blog V4`) and get HF down to ~500 kHz.
