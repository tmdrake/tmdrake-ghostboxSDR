#Requires -Version 5.1
# ShackHack WinUSB package for cheap RTL-SDR dongles.
# Windows already ships winusb.sys. This binds it to Bulk-In Interface 0 via official Zadig.
# Interface 0 (MI_00) is the radio. Interface 1 is IR.

[CmdletBinding()]
param(
    [switch] $Status,
    [switch] $Install,
    [switch] $Help
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ZadigVersion = "2.9"
$ZadigUrl = "https://github.com/pbatard/libwdi/releases/download/v1.5.1/zadig-2.9.exe"
$ZadigSha256 = "4ECAA95DF3DA3621486A043AEF8B3050B8BAFE7C901402871E816229EF82039B"
$ToolsDir = Join-Path $env:LOCALAPPDATA "ShackHack\tools"
$ZadigPath = Join-Path $ToolsDir "zadig.exe"

# Realtek wifi (RTL8188 etc.) shares VID 0BDA. Only match RTL2832-class PIDs.
$RtlVidPid = @(
    "VID_0BDA&PID_2832",
    "VID_0BDA&PID_2838",
    "VID_0BDA&PID_2831",
    "VID_0CCD&PID_00A9",
    "VID_0CCD&PID_00B3",
    "VID_0CCD&PID_00D3",
    "VID_0CCD&PID_00D7",
    "VID_0CCD&PID_00E0",
    "VID_1D50&PID_6044",
    "VID_1D50&PID_6089",
    "VID_1F4D&PID_B803",
    "VID_1F4D&PID_C803",
    "VID_1209&PID_2832"
)

function Show-Help {
    Write-Host "ShackHack WinUSB package"
    Write-Host "  Binds Microsoft WinUSB to the RTL-SDR. No kernel driver to compile."
    Write-Host ""
    Write-Host "  powershell -ExecutionPolicy Bypass -File packaging/windows/install-winusb.ps1"
    Write-Host "  powershell -ExecutionPolicy Bypass -File packaging/windows/install-winusb.ps1 -Status"
    Write-Host ""
    Write-Host "  In Zadig: Options -> List All Devices"
    Write-Host "            Bulk-In, Interface (Interface 0)   (MI_00)"
    Write-Host "            target WinUSB -> Replace Driver"
    Write-Host "            unplug and replug the stick"
}

function Test-IsRtlHardwareId {
    param([string] $Id)
    foreach ($token in $RtlVidPid) {
        if ($Id -like ("*" + $token + "*")) {
            return $true
        }
    }
    return $false
}

function Get-RtlUsbDevices {
    $present = @(Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue)
    if ($present.Count -eq 0) {
        return @()
    }
    $out = @()
    foreach ($dev in $present) {
        $id = [string] $dev.InstanceId
        $name = [string] $dev.FriendlyName
        $match = $false
        if (Test-IsRtlHardwareId -Id $id) { $match = $true }
        if ($name -match "RTL2832|RTL2838|RTL-SDR|RTLSDR") { $match = $true }
        if ($name -match "Bulk-In" -and $id -match "VID_0CCD|VID_0BDA&PID_283|VID_1D50|VID_1F4D") {
            $match = $true
        }
        if ($match) { $out += $dev }
    }
    return $out
}

function Get-DeviceService {
    param([string] $InstanceId)
    try {
        $p = Get-PnpDeviceProperty -InstanceId $InstanceId -KeyName "DEVPKEY_Device_Service" -ErrorAction Stop
        return [string] $p.Data
    }
    catch {
        return ""
    }
}

function Get-Prop {
    param($Object, [string] $Name, [string] $Fallback = "")
    $v = $Object.PSObject.Properties[$Name]
    if ($null -eq $v -or $null -eq $v.Value -or [string]::IsNullOrWhiteSpace([string]$v.Value)) {
        return $Fallback
    }
    return [string] $v.Value
}

function Write-DeviceStatus {
    Write-Host "ShackHack WinUSB package - device status"
    Write-Host "Target: inbox WinUSB on Bulk-In Interface 0. No .sys to compile."
    Write-Host ""
    $devs = @(Get-RtlUsbDevices)
    if ($devs.Count -eq 0) {
        Write-Host "No RTL-SDR-like USB device is present."
        Write-Host "Plug the dongle into a powered USB port, then rerun -Status."
        return $false
    }
    $ready = $false
    foreach ($d in $devs) {
        $id = Get-Prop $d "InstanceId"
        $svc = Get-DeviceService -InstanceId $id
        $iface = "parent"
        if ($id -match "MI_00") { $iface = "radio (interface 0)" }
        if ($id -match "MI_01") { $iface = "IR (interface 1) - leave this" }
        $ok = $false
        if ($svc -eq "WinUSB" -and $id -match "MI_00") { $ok = $true }
        if ($svc -eq "WinUSB" -and $id -notmatch "MI_") { $ok = $true }
        if ($ok) { $ready = $true }
        $mark = "NEED WINUSB"
        if ($ok) { $mark = "READY" }
        $svcLabel = $svc
        if ([string]::IsNullOrWhiteSpace($svcLabel)) { $svcLabel = "(none)" }
        Write-Host ("[{0}] {1}" -f $mark, (Get-Prop $d "FriendlyName" "RTL-SDR"))
        Write-Host ("       status={0}  class={1}  service={2}" -f (Get-Prop $d "Status" "?"), (Get-Prop $d "Class" "(none)"), $svcLabel)
        Write-Host ("       {0}" -f $id)
        Write-Host ("       {0}" -f $iface)
        Write-Host ""
    }
    if ($ready) {
        Write-Host "Radio interface already has WinUSB. Run: cargo run --release -p shackhack"
    }
    else {
        Write-Host "Radio interface is not on WinUSB yet. Rerun this script without -Status (or pass -Install)."
    }
    return $ready
}

function Get-Zadig {
    New-Item -ItemType Directory -Force -Path $ToolsDir | Out-Null
    if (Test-Path $ZadigPath) {
        $hash = (Get-FileHash -Path $ZadigPath -Algorithm SHA256).Hash
        if ($hash -eq $ZadigSha256) {
            return $ZadigPath
        }
        Write-Host "Cached Zadig hash mismatch - downloading again."
        Remove-Item -Force $ZadigPath
    }
    Write-Host ("Downloading official Zadig {0} (Akeo / libwdi)..." -f $ZadigVersion)
    Write-Host $ZadigUrl
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $ZadigUrl -OutFile $ZadigPath -UseBasicParsing
    $hash = (Get-FileHash -Path $ZadigPath -Algorithm SHA256).Hash
    if ($hash -ne $ZadigSha256) {
        Remove-Item -Force $ZadigPath -ErrorAction SilentlyContinue
        throw ("Zadig SHA-256 mismatch. Expected {0} got {1}" -f $ZadigSha256, $hash)
    }
    $iniLines = @(
        "[general]",
        "  advanced_mode = false",
        "  exit_on_success = false",
        "",
        "[device]",
        "  list_all = true",
        "  include_hubs = false",
        "  trim_whitespaces = true",
        "",
        "[driver]",
        "  default_driver = 0"
    )
    Set-Content -Path (Join-Path $ToolsDir "zadig.ini") -Value $iniLines -Encoding ASCII
    return $ZadigPath
}

function Install-WinUsb {
    $null = Write-DeviceStatus
    Write-Host ""
    $zadig = Get-Zadig
    Write-Host "Launching Zadig as Administrator."
    Write-Host "Select Bulk-In, Interface (Interface 0). Target WinUSB. Replace Driver."
    Write-Host "Do not pick the composite parent or Interface 1."
    Start-Process -FilePath $zadig -WorkingDirectory $ToolsDir -Verb RunAs | Out-Null
}

if ($Help) {
    Show-Help
    exit 0
}

if ($Status -and -not $Install) {
    $null = Write-DeviceStatus
    exit 0
}

Install-WinUsb
