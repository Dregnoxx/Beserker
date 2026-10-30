//! Détection en lecture seule des interfaces HID utiles :
//! EVision (Berserker d'usine ou autre modèle), bootloader Sonix ISP
//! et interface brute QMK/VIA des claviers reflashés.
//!
//! Ce module ne flashe rien et n'ouvre pas les interfaces détectées.

use crate::protocol;

pub const EVISION_VID: u16 = protocol::VENDOR_ID;
pub const BERSERKER_TKL_PID: u16 = protocol::PRODUCT_ID;
pub const SONIX_VID: u16 = 0x0C45;
pub const QMK_VID: u16 = 0xFEED;
pub const QMK_RAW_USAGE_PAGE: u16 = 0xFF60;
pub const QMK_RAW_USAGE: u16 = 0x61;

pub const SONIX_BOOTLOADER_PIDS: &[(u16, &str)] = &[
    (0x7900, "SN32F22X / SN32F23X / SN32F24X"),
    (0x7040, "SN32F24XB"),
    (0x7160, "SN32F24XC"),
    (0x7010, "SN32F26X"),
    (0x7120, "SN32F28X"),
    (0x7140, "SN32F29X"),
    (0x7210, "SN34F28X"),
    (0x7220, "SN32F24XD"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    /// Berserker BYGG TKL d'usine : protocole EVision V2 complet.
    EvisionTkl,
    /// Interface EVision d'un autre modèle / PID non répertorié.
    EvisionOther,
    /// Bootloader ISP Sonix (mode flash).
    SonixBootloader,
    /// Interface brute QMK/VIA (clavier reflashé).
    QmkRawHid,
}

impl DeviceKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::EvisionTkl => "Berserker BYGG TKL · EVision V2",
            Self::EvisionOther => "EVision · modèle non répertorié",
            Self::SonixBootloader => "Sonix ISP · mode bootloader",
            Self::QmkRawHid => "QMK / VIA · clavier reflashé",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::EvisionTkl => {
                "Firmware d'usine EVision V2. Édition RGB per-touche active (126 zones)."
            }
            Self::EvisionOther => {
                "Interface EVision détectée, mais ce modèle n'a pas de layout/protocole validé dans l'app."
            }
            Self::SonixBootloader => {
                "Bootloader Sonix détecté. SonixFlasherC est un outil séparé : ni inclus, ni lancé par cette app."
            }
            Self::QmkRawHid => {
                "Firmware QMK/VIA détecté. Éclairage global RGB Matrix disponible dans cette page ; per-key non exposé par le firmware."
            }
        }
    }

    pub fn is_factory_evision(self) -> bool {
        matches!(self, Self::EvisionTkl)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub vid: u16,
    pub pid: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub product: Option<String>,
    pub path: String,
    pub kind: DeviceKind,
}

impl DeviceInfo {
    pub fn identity(&self) -> String {
        format!("{:04X}:{:04X}", self.vid, self.pid)
    }

    pub fn sonix_chip(&self) -> Option<&'static str> {
        SONIX_BOOTLOADER_PIDS
            .iter()
            .find(|(pid, _)| *pid == self.pid)
            .map(|(_, name)| *name)
    }

    pub fn summary(&self) -> String {
        let mut text = format!(
            "{} — VID:PID {} · usage {:04X}:{:04X}",
            self.kind.title(),
            self.identity(),
            self.usage_page,
            self.usage
        );
        if let Some(product) = &self.product {
            text.push_str(&format!(" · {product}"));
        }
        if !self.path.is_empty() {
            text.push_str(&format!(" · {}", self.path));
        }
        if let Some(chip) = self.sonix_chip() {
            text.push_str(&format!(" · {chip}"));
        }
        text
    }
}

fn is_qmk_product(product: Option<&str>) -> bool {
    product
        .map(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("qmk")
                || lower
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|word| word == "via" || word == "vial")
        })
        .unwrap_or(false)
}

/// Classification pure, testable sans matériel.
pub fn classify(vid: u16, pid: u16, usage_page: u16, usage: u16) -> Option<DeviceKind> {
    classify_with_product(vid, pid, usage_page, usage, None)
}

fn classify_with_product(
    vid: u16,
    pid: u16,
    usage_page: u16,
    usage: u16,
    product: Option<&str>,
) -> Option<DeviceKind> {
    if vid == SONIX_VID && SONIX_BOOTLOADER_PIDS.iter().any(|(p, _)| *p == pid) {
        return Some(DeviceKind::SonixBootloader);
    }
    let qmk_product = is_qmk_product(product) && matches!(vid, EVISION_VID | SONIX_VID | QMK_VID);
    if vid == QMK_VID || (usage_page == QMK_RAW_USAGE_PAGE && usage == QMK_RAW_USAGE) || qmk_product
    {
        return Some(DeviceKind::QmkRawHid);
    }
    if vid == EVISION_VID {
        if pid == BERSERKER_TKL_PID {
            // Une même interface clavier expose souvent une collection
            // « clavier » standard en plus de la collection vendor EVision.
            // Seule la collection vendor (ou un descripteur absent) représente
            // le Berserker d'usine ; l'autre est ignorée.
            return if usage_page == 0
                || (usage_page == protocol::USAGE_PAGE && usage == protocol::USAGE)
            {
                Some(DeviceKind::EvisionTkl)
            } else {
                None
            };
        }
        return Some(DeviceKind::EvisionOther);
    }
    None
}

fn push_unique(devices: &mut Vec<DeviceInfo>, device: DeviceInfo) {
    if devices.iter().any(|existing| {
        existing.vid == device.vid
            && existing.pid == device.pid
            && existing.kind == device.kind
            && existing.product == device.product
    }) {
        return;
    }
    if device.kind == DeviceKind::QmkRawHid {
        // Une même interface QMK expose aussi une collection EVision/clavier
        // standard : ne pas la présenter comme un second modèle EVision.
        devices.retain(|existing| {
            !(existing.kind == DeviceKind::EvisionOther
                && existing.vid == device.vid
                && existing.pid == device.pid)
        });
    }
    if device.kind == DeviceKind::EvisionOther
        && devices.iter().any(|existing| {
            existing.kind == DeviceKind::QmkRawHid
                && existing.vid == device.vid
                && existing.pid == device.pid
        })
    {
        return;
    }
    devices.push(device);
}

#[cfg(windows)]
pub fn scan() -> Vec<DeviceInfo> {
    use hidapi::HidApi;

    let Ok(api) = HidApi::new() else {
        return Vec::new();
    };
    let mut devices = Vec::new();
    for info in api.device_list() {
        let vid = info.vendor_id();
        let pid = info.product_id();
        let usage_page = info.usage_page();
        let usage = info.usage();
        let product = info.product_string().map(ToOwned::to_owned);
        let Some(kind) = classify_with_product(vid, pid, usage_page, usage, product.as_deref())
        else {
            continue;
        };
        push_unique(
            &mut devices,
            DeviceInfo {
                vid,
                pid,
                usage_page,
                usage,
                product,
                path: info.path().to_string_lossy().to_string(),
                kind,
            },
        );
    }
    devices
}

#[cfg(target_os = "linux")]
fn descriptor_has_usage(bytes: &[u8], page: u16, usage: u16) -> bool {
    let page_bytes = [0x06, (page & 0xFF) as u8, (page >> 8) as u8];
    let usage_bytes = [0x09, (usage & 0xFF) as u8];
    bytes.windows(page_bytes.len()).any(|w| w == page_bytes)
        && bytes.windows(usage_bytes.len()).any(|w| w == usage_bytes)
}

#[cfg(target_os = "linux")]
pub fn scan() -> Vec<DeviceInfo> {
    use std::fs;

    let mut devices = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return devices;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let raw_path = format!("/dev/{}", name.to_string_lossy());
        let uevent = entry.path().join("device/uevent");
        let Ok(text) = fs::read_to_string(&uevent) else {
            continue;
        };
        let mut vid = 0u16;
        let mut pid = 0u16;
        let mut product = None;
        for line in text.lines() {
            if let Some(id) = line.strip_prefix("HID_ID=") {
                let parts: Vec<&str> = id.split(':').collect();
                if parts.len() == 3 {
                    vid = u16::from_str_radix(parts[1].trim_start_matches('0'), 16).unwrap_or(0);
                    pid = u16::from_str_radix(parts[2].trim_start_matches('0'), 16).unwrap_or(0);
                }
            } else if let Some(name) = line.strip_prefix("HID_NAME=") {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    product = Some(trimmed.to_string());
                }
            }
        }
        let descriptor =
            fs::read(entry.path().join("device/report_descriptor")).unwrap_or_default();
        let (usage_page, usage) =
            if descriptor_has_usage(&descriptor, protocol::USAGE_PAGE, protocol::USAGE) {
                (protocol::USAGE_PAGE, protocol::USAGE)
            } else if descriptor_has_usage(&descriptor, QMK_RAW_USAGE_PAGE, QMK_RAW_USAGE) {
                (QMK_RAW_USAGE_PAGE, QMK_RAW_USAGE)
            } else {
                (0, 0)
            };
        let Some(kind) = classify_with_product(vid, pid, usage_page, usage, product.as_deref())
        else {
            continue;
        };
        push_unique(
            &mut devices,
            DeviceInfo {
                vid,
                pid,
                usage_page,
                usage,
                product,
                path: raw_path,
                kind,
            },
        );
    }
    devices
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn scan() -> Vec<DeviceInfo> {
    Vec::new()
}

/// Message court quand aucun contrôleur EVision TKL n'est disponible.
pub fn detection_hint(devices: &[DeviceInfo]) -> Option<String> {
    if devices
        .iter()
        .any(|device| device.kind == DeviceKind::QmkRawHid)
    {
        return Some(
            "Clavier QMK/VIA détecté · éclairage global disponible dans Claviers.".to_string(),
        );
    }
    if devices
        .iter()
        .any(|device| device.kind == DeviceKind::SonixBootloader)
    {
        return Some(
            "Clavier en mode bootloader Sonix · utiliser SonixFlasherC séparément.".to_string(),
        );
    }
    if devices
        .iter()
        .any(|device| device.kind == DeviceKind::EvisionOther)
    {
        return Some("Modèle EVision détecté · layout non répertorié.".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifie_les_interfaces_connues() {
        assert_eq!(
            classify(
                EVISION_VID,
                BERSERKER_TKL_PID,
                protocol::USAGE_PAGE,
                protocol::USAGE
            ),
            Some(DeviceKind::EvisionTkl)
        );
        assert_eq!(
            classify(EVISION_VID, 0x1234, protocol::USAGE_PAGE, protocol::USAGE),
            Some(DeviceKind::EvisionOther)
        );
        assert_eq!(
            classify(EVISION_VID, BERSERKER_TKL_PID, 0x0006, 0x0001),
            None
        );
        assert_eq!(
            classify(SONIX_VID, 0x7040, 0, 0),
            Some(DeviceKind::SonixBootloader)
        );
        assert_eq!(
            classify(0xFEED, 0x0000, QMK_RAW_USAGE_PAGE, QMK_RAW_USAGE),
            Some(DeviceKind::QmkRawHid)
        );
        assert_eq!(classify(0x1234, 0x5678, 0x0006, 0x0001), None);
    }

    #[test]
    fn un_evision_reflashe_qmk_reste_qmk() {
        assert_eq!(
            classify(
                EVISION_VID,
                BERSERKER_TKL_PID,
                QMK_RAW_USAGE_PAGE,
                QMK_RAW_USAGE
            ),
            Some(DeviceKind::QmkRawHid)
        );
    }

    #[test]
    fn qmk_par_vid_ou_nom_produit() {
        assert_eq!(
            classify(QMK_VID, 0x0000, 0x0006, 0x0001),
            Some(DeviceKind::QmkRawHid)
        );
        assert_eq!(
            classify_with_product(
                EVISION_VID,
                BERSERKER_TKL_PID,
                0x0006,
                0x0001,
                Some("Berserker VIA")
            ),
            Some(DeviceKind::QmkRawHid)
        );
        assert_eq!(
            classify_with_product(0x1102, 0x0001, 0x0006, 0x0001, Some("VIA HD Audio")),
            None
        );
    }
}
