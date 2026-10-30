//! Protocole EVision V2 pour clavier Berserker 320F:5041.
//! Écritures de configuration uniquement : mode, couleur, luminosité,
//! vitesse, direction et couleurs per-touche. Aucune commande
//! firmware/bootloader n'est implémentée.

use anyhow::{Context, Result, anyhow};
#[cfg(windows)]
use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

pub const VENDOR_ID: u16 = 0x320F;
pub const PRODUCT_ID: u16 = 0x5041;
pub const USAGE_PAGE: u16 = 0xFF1C;
pub const USAGE: u16 = 0x92;

const PACKET_SIZE: usize = 64;
const REPORT_ID: u8 = 4;
const ACK_TIMEOUT_MS: u64 = 800;
const QUERY_RETRIES: usize = 2;

const CMD_BEGIN_CONFIGURE: u8 = 0x01;
const CMD_END_CONFIGURE: u8 = 0x02;
const CMD_READ_CAPABILITIES: u8 = 0x03;
const CMD_READ_CONFIG: u8 = 0x05;
const CMD_WRITE_CONFIG: u8 = 0x06;
const CMD_READ_CUSTOM_COLORS: u8 = 0x0A;
const CMD_WRITE_CUSTOM_COLORS: u8 = 0x0B;
const CMD_DIRECT: u8 = 0x12;
const CMD_DIRECT_END: u8 = 0x13;
const CMD_READ_KEY_MAP: u8 = 0x1B;

pub const MODE_STATIC: u8 = 0x06;
pub const MODE_CUSTOM: u8 = 0x14;
pub const LED_COUNT: usize = 126;
pub const CUSTOM_COLORSET_SIZE: usize = LED_COUNT * 3;
pub const CUSTOM_COLORSET_COUNT: u8 = 10;
const CUSTOM_AREA_STRIDE: u16 = 512;

/// Modes EVision V2 utilisables via la configuration.
pub const MODES: &[(&str, u8)] = &[
    ("Statique", 0x06),
    ("Respiration", 0x05),
    ("Vortex", 0x03),
    ("Aura", 0x04),
    ("Vagues", 0x08),
    ("Scintillements", 0x0A),
    ("Mosaïques", 0x0B),
    ("Cascade", 0x0C),
    ("Oscillation", 0x0D),
    ("Pluie", 0x0F),
    ("Balayage", 0x10),
    ("Radiation", 0x12),
];

/// Modes dont la vitesse est réellement pilotable par le firmware 5041.
/// Le logiciel d'usine grise le curseur « Vitesse » pour Statique,
/// Balayage et Radiation ; pour les autres effets l'octet vitesse agit.
pub fn mode_supports_speed(mode: u8) -> bool {
    !matches!(mode, MODE_CUSTOM | MODE_STATIC | 0x10 | 0x12)
}

pub const SPEED_UI_MAX: u8 = 4;

/// L'octet vitesse du firmware est inversé : 0 = animation la plus rapide,
/// 4 = la plus lente (mesuré à la webcam sur Respiration). L'interface
/// expose l'inverse, 0 = lent et 4 = rapide, comme le logiciel d'usine.
pub fn firmware_speed(ui_speed: u8) -> u8 {
    SPEED_UI_MAX.saturating_sub(ui_speed.min(SPEED_UI_MAX))
}

/// Convertit l'octet vitesse lu sur le clavier vers l'échelle de l'interface.
pub fn ui_speed(firmware_speed: u8) -> u8 {
    SPEED_UI_MAX.saturating_sub(firmware_speed.min(SPEED_UI_MAX))
}

/// Ancienne liste (V1.1) pour relire les profils enregistrés avant la refonte.
pub const LEGACY_MODE_INDEX: &[u8] = &[
    0,  // 0 = personnalisé
    1,  // Statique
    2,  // Respiration
    4,  // Spectre -> Aura
    5,  // Vague courte -> Vagues
    5,  // Vague longue -> Vagues
    3,  // Roue chromatique -> Vortex
    4,  // Réactif -> Aura
    4,  // Réactif ondulation -> Aura
    4,  // Réactif ligne -> Aura
    6,  // Étoiles rapides -> Scintillements
    7,  // Floraison -> Mosaïques
    4,  // Vague arc-en-ciel -> Aura
    9,  // Ouragan -> Oscillation
    7,  // Accumulation -> Mosaïques
    6,  // Étoiles lentes -> Scintillements
    11, // Visière -> Balayage
    12, // Surmount -> Radiation
    3,  // Cercle arc-en-ciel -> Vortex
];

pub struct Controller {
    #[cfg(windows)]
    dev: HidDevice,
    #[cfg(target_os = "linux")]
    dev: crate::linux_hid::LinuxHid,
    profile: u8,
}

impl Controller {
    #[cfg(windows)]
    pub fn open() -> Result<Self> {
        let api = HidApi::new().context("initialisation hidapi")?;
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == VENDOR_ID
                    && d.product_id() == PRODUCT_ID
                    && d.usage_page() == USAGE_PAGE
                    && d.usage() == USAGE
            })
            .ok_or_else(|| {
                anyhow!(
                    "clavier Berserker {VENDOR_ID:04X}:{PRODUCT_ID:04X} usage FF1C:0092 introuvable"
                )
            })?;
        let dev = info.open_device(&api).context("ouverture HID")?;

        let mut controller = Self { dev, profile: 0 };
        controller.profile = controller.read_current_profile().unwrap_or(0);
        Ok(controller)
    }

    #[cfg(target_os = "linux")]
    pub fn open() -> Result<Self> {
        let paths = crate::linux_hid::candidate_paths();
        if paths.is_empty() {
            return Err(anyhow!(
                "clavier Berserker {VENDOR_ID:04X}:{PRODUCT_ID:04X} introuvable dans /dev/hidraw"
            ));
        }
        let mut last_error = None;
        for path in paths {
            match crate::linux_hid::LinuxHid::open(&path) {
                Ok(dev) => {
                    let mut controller = Self { dev, profile: 0 };
                    match controller.read_capabilities() {
                        Ok(caps) if caps.len() >= 6 && caps[5] as usize == LED_COUNT => {
                            controller.profile = controller.read_current_profile().unwrap_or(0);
                            return Ok(controller);
                        }
                        Ok(_) => {
                            last_error = Some(anyhow!(
                                "interface {} ne répond pas au protocole EVision",
                                path.display()
                            ));
                        }
                        Err(error) => last_error = Some(error),
                    }
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            anyhow!("aucune interface EVision accessible (règle udev ou permissions /dev/hidraw ?)")
        }))
    }

    fn build_packet(cmd: u8, offset: u16, data: &[u8], size: u8) -> [u8; PACKET_SIZE] {
        let mut packet = [0u8; PACKET_SIZE];
        packet[0] = REPORT_ID;
        packet[3] = cmd;
        packet[4] = size;
        packet[5] = (offset & 0xFF) as u8;
        packet[6] = ((offset >> 8) & 0xFF) as u8;
        let copy_len = data.len().min(PACKET_SIZE - 8).min(size as usize);
        packet[8..8 + copy_len].copy_from_slice(&data[..copy_len]);

        let checksum: u16 = packet[3..].iter().map(|&b| b as u16).sum();
        packet[1] = (checksum & 0xFF) as u8;
        packet[2] = ((checksum >> 8) & 0xFF) as u8;
        packet
    }

    /// Vide les échos en attente. Un autre logiciel (Berserker.exe, OpenRGB)
    /// peut écrire sur la même collection HID : on ignore ses réponses.
    fn drain_input(&self) {
        let mut buf = [0u8; PACKET_SIZE];
        for _ in 0..8 {
            match self.dev.read_timeout(&mut buf, 0) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }

    fn read_matching_ack(&self, packet: &[u8; PACKET_SIZE], cmd: u8) -> Result<Vec<u8>> {
        let deadline = Instant::now() + Duration::from_millis(ACK_TIMEOUT_MS);
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(anyhow!("pas d'acquittement du clavier"));
            }
            let remaining = (deadline - now).as_millis().clamp(1, 200) as i32;
            let mut ack = [0u8; PACKET_SIZE];
            let read = self
                .dev
                .read_timeout(&mut ack, remaining)
                .context("lecture de l'acquittement HID")?;
            if read == 0 {
                continue;
            }
            if ack[0] != REPORT_ID {
                continue;
            }
            // Réponse d'un autre programme : mauvais checksum, commande ou offset.
            if ack[3] != cmd
                || ack[5] != packet[5]
                || ack[6] != packet[6]
                || ack[1] != packet[1]
                || ack[2] != packet[2]
            {
                continue;
            }
            if ack[7] != 0 {
                return Err(anyhow!(
                    "le clavier a refusé la commande (statut {})",
                    ack[7]
                ));
            }
            let len = (ack[4] as usize).min(PACKET_SIZE - 8);
            return Ok(ack[8..8 + len].to_vec());
        }
    }

    fn query(&self, cmd: u8, offset: u16, data: &[u8], size: u8) -> Result<Vec<u8>> {
        let packet = Self::build_packet(cmd, offset, data, size);
        let mut last_error = anyhow!("échec de la commande {cmd:#04x}");
        for attempt in 0..=QUERY_RETRIES {
            self.drain_input();
            let written = self.dev.write(&packet).context("écriture HID")?;
            if written != PACKET_SIZE {
                last_error = anyhow!("écriture HID partielle ({written}/{PACKET_SIZE})");
                continue;
            }
            match self.read_matching_ack(&packet, cmd) {
                Ok(data) => return Ok(data),
                Err(error) => {
                    last_error = error;
                    if attempt < QUERY_RETRIES {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
            }
        }
        Err(last_error)
    }

    fn begin(&self) -> Result<()> {
        self.query(CMD_BEGIN_CONFIGURE, 0, &[], 0).map(|_| ())
    }

    fn end(&self) -> Result<()> {
        self.query(CMD_END_CONFIGURE, 0, &[], 0).map(|_| ())
    }

    /// Lecture brute des capacités (map_size, taille macros).
    pub fn read_capabilities(&self) -> Result<Vec<u8>> {
        self.query(CMD_READ_CAPABILITIES, 0, &[], 7)
    }

    /// Lit une plage de la configuration EVision.
    pub fn read_config(&self, offset: u16, size: u8) -> Result<Vec<u8>> {
        self.query(CMD_READ_CONFIG, offset, &[], size)
    }

    fn read_chunks(&self, cmd: u8, offset: u16, size: usize) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(size);
        let mut offset = offset;
        while result.len() < size {
            let chunk = (size - result.len()).min(PACKET_SIZE - 8) as u8;
            let data = self.query(cmd, offset, &[], chunk)?;
            if data.is_empty() {
                break;
            }
            offset = offset.wrapping_add(data.len() as u16);
            result.extend_from_slice(&data);
        }
        Ok(result)
    }

    fn write_chunks(&self, cmd: u8, offset: u16, data: &[u8]) -> Result<()> {
        let mut offset = offset;
        for chunk in data.chunks(PACKET_SIZE - 8) {
            self.query(cmd, offset, chunk, chunk.len() as u8)?;
            offset = offset.wrapping_add(chunk.len() as u16);
        }
        Ok(())
    }

    fn check_colorset(colorset: u8) -> Result<u16> {
        if colorset >= CUSTOM_COLORSET_COUNT {
            return Err(anyhow!("jeu de couleurs invalide"));
        }
        Ok(CUSTOM_AREA_STRIDE * colorset as u16)
    }

    /// Lit le jeu de couleurs per-touche d'un profil Custom (0..=9).
    pub fn read_custom_colors(&self, colorset: u8, size: usize) -> Result<Vec<u8>> {
        let offset = Self::check_colorset(colorset)?;
        self.read_chunks(CMD_READ_CUSTOM_COLORS, offset, size)
    }

    /// Lit la table firmware index logique -> position LED (commande 0x1B).
    pub fn read_key_map(&self) -> Result<Vec<u8>> {
        let map = self.read_chunks(CMD_READ_KEY_MAP, 0, LED_COUNT)?;
        if map.len() != LED_COUNT {
            return Err(anyhow!(
                "table de correspondance incomplète ({} octets)",
                map.len()
            ));
        }
        Ok(map)
    }

    /// Écrit le jeu de couleurs per-touche d'un profil Custom (0..=9),
    /// puis le relit pour confirmer que le firmware l'a bien stocké.
    pub fn write_custom_colors(&self, colorset: u8, data: &[u8]) -> Result<()> {
        if data.len() != CUSTOM_COLORSET_SIZE {
            return Err(anyhow!(
                "taille de couleurs inattendue : {} (attendu {})",
                data.len(),
                CUSTOM_COLORSET_SIZE
            ));
        }
        let offset = Self::check_colorset(colorset)?;
        self.begin()?;
        let result = self.write_chunks(CMD_WRITE_CUSTOM_COLORS, offset, data);
        let end_result = self.end();
        result?;
        end_result?;
        self.verify_custom_colors(colorset, data)
    }

    /// Relit les couleurs et compare avec ce qui a été envoyé.
    pub fn verify_custom_colors(&self, colorset: u8, expected: &[u8]) -> Result<()> {
        let actual = self.read_custom_colors(colorset, expected.len())?;
        if actual.len() != expected.len() {
            return Err(anyhow!(
                "relecture des couleurs incomplète ({} / {} octets)",
                actual.len(),
                expected.len()
            ));
        }
        if actual != expected {
            let mut first = None;
            for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
                if a != e {
                    first = Some((i, *e, *a));
                    break;
                }
            }
            if let Some((i, expected, actual)) = first {
                return Err(anyhow!(
                    "relecture des couleurs différente à l'octet {i} (attendu {expected:#04x}, lu {actual:#04x})"
                ));
            }
            return Err(anyhow!("relecture des couleurs différente"));
        }
        Ok(())
    }

    fn read_current_profile(&self) -> Result<u8> {
        let data = self.query(CMD_READ_CONFIG, 0, &[], 1)?;
        let profile = data.first().copied().unwrap_or(0);
        Ok(if profile <= 2 { profile } else { 0 })
    }

    fn verify_config_fields(&self, expected: &[u8], include_color: bool) -> Result<()> {
        let offset = (self.profile as u16) * 0x40 + 1;
        let actual = self.read_config(offset, expected.len() as u8)?;
        if actual.len() < expected.len() {
            return Err(anyhow!(
                "relecture de configuration incomplète ({} / {} octets)",
                actual.len(),
                expected.len()
            ));
        }
        let check_len = if include_color { 8 } else { 5 };
        for i in 0..check_len.min(expected.len()) {
            if actual[i] != expected[i] {
                return Err(anyhow!(
                    "relecture de configuration différente à l'octet {i} (attendu {:#04x}, lu {:#04x})",
                    expected[i],
                    actual[i]
                ));
            }
        }
        Ok(())
    }

    /// Applique un mode EVision V2 avec couleur, luminosité, vitesse et direction,
    /// puis relit la configuration pour confirmer.
    pub fn set_mode(
        &self,
        mode: u8,
        rgb: [u8; 3],
        brightness: u8,
        speed: u8,
        direction: u8,
    ) -> Result<()> {
        let mut config = [0u8; 18];
        config[0] = mode;
        config[1] = brightness;
        config[2] = speed;
        config[3] = direction;
        config[5] = rgb[0];
        config[6] = rgb[1];
        config[7] = rgb[2];

        self.begin()?;
        let offset = (self.profile as u16) * 0x40 + 1;
        let result = self.query(CMD_WRITE_CONFIG, offset, &config, config.len() as u8);
        let end_result = self.end();
        result?;
        end_result?;
        self.verify_config_fields(&config, true)
    }

    pub fn set_static_color(&self, rgb: [u8; 3]) -> Result<()> {
        self.set_mode(MODE_STATIC, rgb, 255, 0, 0)
    }

    /// Active le mode Custom (0x14) et sélectionne le jeu de couleurs per-touche,
    /// puis relit la configuration et le numéro de jeu pour confirmer.
    pub fn set_custom_mode(
        &self,
        colorset: u8,
        brightness: u8,
        speed: u8,
        direction: u8,
    ) -> Result<()> {
        let _ = Self::check_colorset(colorset)?;
        let mut config = [0u8; 18];
        config[0] = MODE_CUSTOM;
        config[1] = brightness;
        config[2] = speed;
        config[3] = direction;

        // Octet 18 du bloc (offset profile*0x40 + 0x12) : variante / jeu de
        // couleurs per-touche. Le firmware 5041 affiche toujours le jeu 0,
        // mais on conserve une valeur valide (0..3) pour ne rien casser.
        config[17] = colorset.min(3);

        self.begin()?;
        let base = (self.profile as u16) * 0x40;
        let result = self.query(CMD_WRITE_CONFIG, base + 1, &config, config.len() as u8);
        let end_result = self.end();
        result?;
        end_result?;
        self.verify_config_fields(&config, false)?;
        Ok(())
    }

    /// Lit un bloc de configuration (ex. profil complet, réglages système).
    pub fn read_config_block(&self, offset: u16, size: usize) -> Result<Vec<u8>> {
        self.read_chunks(CMD_READ_CONFIG, offset, size)
    }

    /// Écrit un petit bloc de configuration à un offset donné, puis relit
    /// les octets pour confirmer. Utilisé pour la fréquence USB et le debounce.
    pub fn write_config_bytes(&self, offset: u16, data: &[u8]) -> Result<()> {
        self.begin()?;
        let result = self.write_chunks(CMD_WRITE_CONFIG, offset, data);
        let end_result = self.end();
        result?;
        end_result?;
        let actual = self.read_config(offset, data.len() as u8)?;
        if actual.len() < data.len() || &actual[..data.len()] != data {
            return Err(anyhow!(
                "relecture de configuration différente à l'offset {offset:#x} (attendu {:02X?}, lu {:02X?})",
                data,
                &actual[..actual.len().min(data.len())]
            ));
        }
        Ok(())
    }

    /// Applique les réglages système de l'outil d'usine dans le bloc du profil
    /// courant : Mode flèche (0x13), Anti-ghosting (0x14), Mode Jeu (0x15),
    /// index de fréquence USB (0x16) et index de debounce (0x19).
    /// Valeurs relevées sur le logiciel Berserker officiel : fréquence
    /// 0=1000 Hz, 1=500 Hz, 2=250 Hz, 3=125 Hz ; debounce 0=5 ms, 1=10 ms,
    /// 2=15 ms, 3=20 ms.
    pub fn apply_system_settings(
        &self,
        polling_index: u8,
        debounce_index: u8,
        win_lock: bool,
        arrow_mode: bool,
        anti_ghosting: bool,
    ) -> Result<()> {
        if polling_index > 3 {
            return Err(anyhow!("index de fréquence USB invalide"));
        }
        if debounce_index > 3 {
            return Err(anyhow!("index de debounce invalide"));
        }
        let flags = [
            (0x13u16, arrow_mode as u8),
            (0x14, anti_ghosting as u8),
            (0x15, win_lock as u8),
        ];
        self.begin()?;
        let mut result: Result<()> = Ok(());
        for (offset, value) in flags {
            result = self
                .query(CMD_WRITE_CONFIG, offset, &[value], 1)
                .map(|_| ());
            if result.is_err() {
                break;
            }
        }
        if result.is_ok() {
            result = self
                .query(CMD_WRITE_CONFIG, 0x16, &[polling_index], 1)
                .map(|_| ());
        }
        if result.is_ok() {
            result = self
                .query(CMD_WRITE_CONFIG, 0x19, &[debounce_index], 1)
                .map(|_| ());
        }
        let end_result = self.end();
        result?;
        end_result?;
        for (offset, value) in flags {
            let actual = self.read_config(offset, 1)?;
            if actual.first().copied() != Some(value) {
                return Err(anyhow!(
                    "relecture du réglage système à l'offset {offset:#x} différente"
                ));
            }
        }
        let poll = self.read_config(0x16, 1)?;
        if poll.first().copied() != Some(polling_index) {
            return Err(anyhow!("relecture de la fréquence USB différente"));
        }
        let deb = self.read_config(0x19, 1)?;
        if deb.first().copied() != Some(debounce_index) {
            return Err(anyhow!("relecture du debounce différente"));
        }
        Ok(())
    }

    /// Lit les réglages système du profil courant.
    pub fn read_system_settings(&self) -> Result<(u8, u8, bool, bool, bool)> {
        let data = self.read_config(0x13, 7)?;
        if data.len() < 7 {
            return Err(anyhow!("bloc de réglages système incomplet"));
        }
        Ok((
            data[3],      // 0x16 : index fréquence USB
            data[6],      // 0x19 : index debounce
            data[2] != 0, // 0x15 : Mode Jeu
            data[0] != 0, // 0x13 : Mode flèche
            data[1] != 0, // 0x14 : Anti-ghosting
        ))
    }

    /// Envoie une frame directe (0x12), non persistante. La taille doit être
    /// celle du jeu de couleurs (378 octets pour ce clavier).
    pub fn send_direct_frame(&self, data: &[u8]) -> Result<()> {
        if data.len() != CUSTOM_COLORSET_SIZE {
            return Err(anyhow!(
                "frame directe de {} octets (attendu {})",
                data.len(),
                CUSTOM_COLORSET_SIZE
            ));
        }
        self.write_chunks(CMD_DIRECT, 0, data)
    }

    /// Termine le mode direct (0x13) et rend la main à l'effet stocké.
    pub fn end_direct(&self) -> Result<()> {
        self.query(CMD_DIRECT_END, 0, &[], 0).map(|_| ())
    }
}
