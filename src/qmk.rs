//! Client VIA minimal pour l'éclairage RGB Matrix des claviers QMK.
//! L'interface utilisée est la collection HID brute FF60:0061.
//! Aucun flash, aucune commande bootloader : uniquement l'éclairage.

use crate::devices::{DeviceInfo, DeviceKind};
use anyhow::{Result, anyhow};

pub const VIA_CMD_GET_PROTOCOL: u8 = 0x01;
pub const VIA_CMD_CUSTOM_SET: u8 = 0x07;
pub const VIA_CMD_CUSTOM_GET: u8 = 0x08;
pub const VIA_CMD_CUSTOM_SAVE: u8 = 0x09;
pub const VIA_CHANNEL_RGB_MATRIX: u8 = 3;

pub const RGB_VALUE_BRIGHTNESS: u8 = 1;
pub const RGB_VALUE_EFFECT: u8 = 2;
pub const RGB_VALUE_SPEED: u8 = 3;
pub const RGB_VALUE_COLOR: u8 = 4;

const REPORT_SIZE: usize = 32;

#[derive(Clone, Copy, Debug)]
pub struct QmkState {
    pub brightness: u8,
    pub effect: u8,
    pub speed: u8,
    pub hue: u8,
    pub saturation: u8,
}

impl Default for QmkState {
    fn default() -> Self {
        Self {
            brightness: 128,
            effect: 1,
            speed: 128,
            hue: 0,
            saturation: 255,
        }
    }
}

pub struct QmkLighting {
    #[cfg(windows)]
    dev: hidapi::HidDevice,
    #[cfg(target_os = "linux")]
    dev: crate::linux_hid::LinuxHid,
    protocol: u16,
}

impl QmkLighting {
    #[cfg(windows)]
    pub fn open(device: &DeviceInfo) -> Result<Self> {
        use hidapi::HidApi;

        if device.kind != DeviceKind::QmkRawHid {
            return Err(anyhow!("interface QMK/VIA attendue"));
        }
        let api = HidApi::new().map_err(|e| anyhow!("initialisation HID : {e}"))?;
        let info = api
            .device_list()
            .find(|info| {
                info.vendor_id() == device.vid
                    && info.product_id() == device.pid
                    && info.usage_page() == crate::devices::QMK_RAW_USAGE_PAGE
                    && info.usage() == crate::devices::QMK_RAW_USAGE
            })
            .ok_or_else(|| anyhow!("interface QMK/VIA introuvable"))?;
        let dev = info
            .open_device(&api)
            .map_err(|e| anyhow!("ouverture QMK/VIA : {e}"))?;
        let mut lighting = Self { dev, protocol: 0 };
        lighting.protocol = lighting.read_protocol()?;
        Ok(lighting)
    }

    #[cfg(target_os = "linux")]
    pub fn open(device: &DeviceInfo) -> Result<Self> {
        use std::path::Path;

        if device.kind != DeviceKind::QmkRawHid {
            return Err(anyhow!("interface QMK/VIA attendue"));
        }
        if device.path.is_empty() {
            return Err(anyhow!("chemin hidraw QMK/VIA absent"));
        }
        let dev = crate::linux_hid::LinuxHid::open(Path::new(&device.path))
            .map_err(|e| anyhow!("ouverture QMK/VIA : {e:#}"))?;
        let mut lighting = Self { dev, protocol: 0 };
        lighting.protocol = lighting.read_protocol()?;
        Ok(lighting)
    }

    pub fn protocol(&self) -> u16 {
        self.protocol
    }

    #[cfg(windows)]
    fn transfer(&self, payload: &[u8; REPORT_SIZE]) -> Result<[u8; REPORT_SIZE]> {
        let mut output = [0u8; REPORT_SIZE + 1];
        output[1..].copy_from_slice(payload);
        self.dev
            .write(&output)
            .map_err(|e| anyhow!("écriture QMK/VIA : {e}"))?;
        let mut input = [0u8; REPORT_SIZE + 1];
        let read = self
            .dev
            .read_timeout(&mut input, 800)
            .map_err(|e| anyhow!("lecture QMK/VIA : {e}"))?;
        if read == 0 {
            return Err(anyhow!("pas de réponse QMK/VIA"));
        }
        let mut response = [0u8; REPORT_SIZE];
        if read > REPORT_SIZE && input[0] == 0 {
            response.copy_from_slice(&input[1..1 + REPORT_SIZE]);
        } else {
            let take = read.min(REPORT_SIZE);
            response[..take].copy_from_slice(&input[..take]);
        }
        Ok(response)
    }

    #[cfg(target_os = "linux")]
    fn transfer(&self, payload: &[u8; REPORT_SIZE]) -> Result<[u8; REPORT_SIZE]> {
        // hidraw peut attendre le rapport brut de 32 octets ou, selon le
        // descripteur, un octet de Report ID 0 en tête. Essayer les deux
        // évite de dépendre d'un détail de la pile HID du noyau.
        match self.transfer_raw(payload, false) {
            Ok(response) => Ok(response),
            Err(first) => self
                .transfer_raw(payload, true)
                .map_err(|second| anyhow!("{first}; variante Report ID : {second}")),
        }
    }

    #[cfg(target_os = "linux")]
    fn transfer_raw(
        &self,
        payload: &[u8; REPORT_SIZE],
        report_id: bool,
    ) -> Result<[u8; REPORT_SIZE]> {
        if report_id {
            let mut output = [0u8; REPORT_SIZE + 1];
            output[1..].copy_from_slice(payload);
            self.dev
                .write(&output)
                .map_err(|e| anyhow!("écriture QMK/VIA : {e}"))?;
        } else {
            self.dev
                .write(payload)
                .map_err(|e| anyhow!("écriture QMK/VIA : {e}"))?;
        }
        let mut input = [0u8; REPORT_SIZE + 1];
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(800);
        loop {
            let remaining = deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_millis()
                .min(200) as i32;
            if remaining == 0 {
                break;
            }
            let read = self
                .dev
                .read_timeout(&mut input, remaining)
                .map_err(|e| anyhow!("lecture QMK/VIA : {e}"))?;
            if read == 0 {
                continue;
            }
            let mut response = [0u8; REPORT_SIZE];
            if read > REPORT_SIZE && input[0] == 0 {
                response.copy_from_slice(&input[1..1 + REPORT_SIZE]);
            } else {
                let take = read.min(REPORT_SIZE);
                response[..take].copy_from_slice(&input[..take]);
            }
            return Ok(response);
        }
        Err(anyhow!("pas de réponse QMK/VIA (timeout)"))
    }

    fn read_protocol(&self) -> Result<u16> {
        let mut request = [0u8; REPORT_SIZE];
        request[0] = VIA_CMD_GET_PROTOCOL;
        let response = self.transfer(&request)?;
        if response[0] != VIA_CMD_GET_PROTOCOL {
            return Err(anyhow!(
                "réponse VIA inattendue (commande {:#04x})",
                response[0]
            ));
        }
        Ok(u16::from_be_bytes([response[1], response[2]]))
    }

    fn get_value(&self, value: u8) -> Result<[u8; 4]> {
        let mut request = [0u8; REPORT_SIZE];
        request[0] = VIA_CMD_CUSTOM_GET;
        request[1] = VIA_CHANNEL_RGB_MATRIX;
        request[2] = value;
        let response = self.transfer(&request)?;
        if response[0] == 0xFF {
            return Err(anyhow!("commande VIA non gérée par le firmware"));
        }
        if response[0] != VIA_CMD_CUSTOM_GET
            || response[1] != VIA_CHANNEL_RGB_MATRIX
            || response[2] != value
        {
            return Err(anyhow!("réponse VIA incohérente"));
        }
        Ok([response[3], response[4], response[5], response[6]])
    }

    pub fn read_state(&self) -> Result<QmkState> {
        let brightness = self.get_value(RGB_VALUE_BRIGHTNESS)?[0];
        let effect = self.get_value(RGB_VALUE_EFFECT)?[0];
        let speed = self.get_value(RGB_VALUE_SPEED)?[0];
        let color = self.get_value(RGB_VALUE_COLOR)?;
        Ok(QmkState {
            brightness,
            effect,
            speed,
            hue: color[0],
            saturation: color[1],
        })
    }

    fn set_value(&self, value: u8, data: &[u8]) -> Result<()> {
        if data.len() > REPORT_SIZE - 3 {
            return Err(anyhow!("charge VIA trop longue"));
        }
        let mut request = [0u8; REPORT_SIZE];
        request[0] = VIA_CMD_CUSTOM_SET;
        request[1] = VIA_CHANNEL_RGB_MATRIX;
        request[2] = value;
        request[3..3 + data.len()].copy_from_slice(data);
        let response = self.transfer(&request)?;
        if response[0] == 0xFF {
            return Err(anyhow!("commande VIA non gérée par le firmware"));
        }
        if response[0] != VIA_CMD_CUSTOM_SET
            || response[1] != VIA_CHANNEL_RGB_MATRIX
            || response[2] != value
        {
            return Err(anyhow!("réponse VIA incohérente"));
        }
        Ok(())
    }

    pub fn apply(&self, state: QmkState) -> Result<()> {
        self.set_value(RGB_VALUE_EFFECT, &[state.effect])?;
        self.set_value(RGB_VALUE_BRIGHTNESS, &[state.brightness])?;
        self.set_value(RGB_VALUE_SPEED, &[state.speed])?;
        self.set_value(RGB_VALUE_COLOR, &[state.hue, state.saturation])?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let mut request = [0u8; REPORT_SIZE];
        request[0] = VIA_CMD_CUSTOM_SAVE;
        let response = self.transfer(&request)?;
        if response[0] == 0xFF {
            return Err(anyhow!("sauvegarde VIA non gérée par le firmware"));
        }
        Ok(())
    }
}

/// Effets RGB Matrix standard QMK (IDs du firmware SonixQMK).
/// Les IDs peuvent varier si le firmware active un sous-ensemble différent.
pub const EFFECTS: &[(u8, &str)] = &[
    (0, "Désactivé"),
    (1, "Couleur unie"),
    (2, "Alphas + modificateurs"),
    (3, "Dégradé haut/bas"),
    (4, "Dégradé gauche/droite"),
    (5, "Respiration"),
    (6, "Bande saturation"),
    (7, "Bande valeur"),
    (8, "Bande pinwheel saturation"),
    (9, "Bande pinwheel valeur"),
    (10, "Bande spirale saturation"),
    (11, "Bande spirale valeur"),
    (12, "Cycle complet"),
    (13, "Cycle gauche/droite"),
    (14, "Cycle haut/bas"),
    (15, "Chevron arc-en-ciel"),
    (16, "Cycle extérieur/intérieur"),
    (17, "Cycle extérieur/intérieur double"),
    (18, "Cycle pinwheel"),
    (19, "Cycle spirale"),
    (20, "Double balise"),
    (21, "Balise arc-en-ciel"),
    (22, "Pinwheels arc-en-ciel"),
    (23, "Gouttes de pluie"),
    (24, "Gouttes jellybean"),
    (25, "Respiration teinte"),
    (26, "Pendule teinte"),
    (27, "Vague teinte"),
    (28, "Pluie de pixels"),
    (29, "Flux de pixels"),
    (30, "Fractale de pixels"),
    (31, "Carte de chaleur de frappe"),
    (32, "Pluie numérique"),
    (33, "Réactif simple"),
    (34, "Réactif"),
    (35, "Réactif large"),
    (36, "Réactif croix"),
    (37, "Réactif nexus"),
    (38, "Splash"),
    (39, "Solid splash"),
];

pub fn effect_label(id: u8) -> &'static str {
    EFFECTS
        .iter()
        .find(|(value, _)| *value == id)
        .map(|(_, label)| *label)
        .unwrap_or("Effet QMK (ID personnalisé)")
}
