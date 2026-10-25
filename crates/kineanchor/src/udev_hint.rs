// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::net::{Ipv4Addr, UdpSocket};
use std::path::Path;

const TETHER_DRIVERS: &[&str] = &["rndis_host", "cdc_ncm", "cdc_ether", "ipheth"];

/// Known Android OEM USB Vendor IDs (Google, Samsung, Xiaomi, OnePlus/OPPO, Motorola, Sony, ASUS, Fairphone, Nothing)
const ANDROID_USB_VIDS: &[&str] = &[
    "18d1", "04e8", "2717", "22d9", "22b8", "0fce", "0b05", "2ae5", "2b4c", "1004", "2a70",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CableHint {
    TetherActive {
        iface: String,
        gateway: Option<Ipv4Addr>,
    },
    UsbPluggedNoTether {
        product: String,
    },
    NoUsbPhone,
}

impl CableHint {
    pub fn label(&self) -> String {
        match self {
            Self::TetherActive { iface, gateway } => match gateway {
                Some(gw) => format!("USB tether active on {iface} (phone {gw})"),
                None => format!("USB tether interface {iface} detected (waiting for IP)"),
            },
            Self::UsbPluggedNoTether { product } => {
                format!("{product} plugged in — turn on USB tethering on phone")
            }
            Self::NoUsbPhone => {
                "No USB phone detected (if plugged in, cable may be charge-only)".to_string()
            }
        }
    }
}

/// Checks `/sys/class/net` for USB tether interfaces by kernel driver name.
pub fn find_tether_interfaces() -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return out;
    };
    for entry in entries.flatten() {
        let iface = entry.file_name().to_string_lossy().to_string();
        if iface == "lo" {
            continue;
        }
        let driver_link = entry.path().join("device/driver");
        if let Ok(target) = fs::read_link(&driver_link) {
            if let Some(driver_name) = target.file_name().and_then(|s| s.to_str()) {
                if TETHER_DRIVERS.contains(&driver_name) {
                    out.push(iface);
                    continue;
                }
            }
        }
        if iface.starts_with("usb") {
            out.push(iface);
        }
    }
    out
}

/// Parses `/proc/net/route` to find a gateway IPv4 address for the given interface.
pub fn gateway_for_iface(iface: &str) -> Option<Ipv4Addr> {
    let content = fs::read_to_string("/proc/net/route").ok()?;
    for line in content.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 3 && cols[0] == iface {
            let gw_hex = cols[2];
            if gw_hex != "00000000" {
                if let Ok(raw) = u32::from_str_radix(gw_hex, 16) {
                    // /proc/net/route stores IPv4 in host (little-endian on x86/ARM) byte order
                    return Some(Ipv4Addr::from(raw.to_be()));
                }
            }
        }
    }
    None
}

/// Scans `/sys/bus/usb/devices` for connected Android phones when tether is not yet enabled.
fn detect_usb_android_phone() -> Option<String> {
    let entries = fs::read_dir("/sys/bus/usb/devices").ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let vid = fs::read_to_string(path.join("idVendor"))
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())?;
        if ANDROID_USB_VIDS.iter().any(|v| *v == vid) {
            let prod = fs::read_to_string(path.join("product"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Android phone".to_string());
            return Some(prod);
        }
    }
    None
}

pub fn probe_cable_hint() -> CableHint {
    let ifaces = find_tether_interfaces();
    if let Some(iface) = ifaces.into_iter().next() {
        let gateway = gateway_for_iface(&iface);
        return CableHint::TetherActive { iface, gateway };
    }
    if let Some(product) = detect_usb_android_phone() {
        return CableHint::UsbPluggedNoTether { product };
    }
    CableHint::NoUsbPhone
}

/// Finds the best local IPv4 address to embed in the pairing QR code.
/// Prefers the local address on the USB tether subnet if active, otherwise the default LAN route IP.
pub fn detect_local_ip() -> String {
    if let CableHint::TetherActive {
        gateway: Some(gw), ..
    } = probe_cable_hint()
    {
        if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
            if sock.connect((gw, 47474)).is_ok() {
                if let Ok(local) = sock.local_addr() {
                    return local.ip().to_string();
                }
            }
        }
    }

    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        // Does not transmit packets; queries kernel routing table for preferred source IP
        if sock.connect("192.0.2.1:80").is_ok() {
            if let Ok(local) = sock.local_addr() {
                return local.ip().to_string();
            }
        }
    }

    let _ = Path::new("/sys/class/net");
    "127.0.0.1".to_string()
}
