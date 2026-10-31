# Windows WinUSB (Zadig)

The stock Windows DVB-T driver claims the RTL2832U. User-space SDR needs **WinUSB** on the bulk interface.

1. Download Zadig from https://zadig.akeo.ie/
2. Plug in the RTL-SDR.
3. Run Zadig **as Administrator**.
4. **Options → List All Devices**.
5. Pick **Bulk-In, Interface (Interface 0)**. VID is usually `0BDA`, PID `2832` or `2838`.
6. Target: **WinUSB**.
7. **Replace Driver** / **Install Driver**.
8. Unplug and replug the stick.
9. Run ShackHack.

Do not bind WinUSB to a random USB device. If the list is empty, the dongle is not enumerated — try another port, avoid unpowered hubs.

ShackHack talks to the stick with `rs-rtl` (pure Rust, `nusb`). There is no `rtlsdr.dll` to copy. Blog V4 boards are detected by USB strings (`RTLSDRBlog` / `Blog V4`) and get HF down to ~500 kHz.
