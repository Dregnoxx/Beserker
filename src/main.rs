#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod layout;
#[cfg(target_os = "linux")]
mod linux_hid;
mod protocol;

use anyhow::{Result, anyhow};
use eframe::egui;
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Layout, Pos2, Rect,
    RichText, Sense, Stroke, StrokeKind, Vec2, pos2,
};
use protocol::{Controller, MODES};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIGHT_BG: Color32 = Color32::from_rgb(242, 242, 242);
const CARD: Color32 = Color32::from_rgb(255, 255, 255);
const SIDEBAR: Color32 = Color32::from_rgb(43, 43, 43);
const SIDEBAR_ITEM: Color32 = Color32::from_rgb(58, 58, 58);
const PANEL: Color32 = Color32::from_rgb(43, 43, 43);
const ACCENT: Color32 = Color32::from_rgb(255, 158, 44);
const TEXT: Color32 = Color32::from_rgb(232, 232, 232);
const MUTED: Color32 = Color32::from_rgb(150, 150, 150);
const OK: Color32 = Color32::from_rgb(88, 200, 120);
const ERR: Color32 = Color32::from_rgb(240, 110, 110);

const PALETTE: [[u8; 3]; 16] = [
    [255, 0, 0],
    [255, 96, 0],
    [255, 220, 0],
    [128, 255, 0],
    [0, 128, 255],
    [0, 64, 255],
    [128, 0, 255],
    [255, 0, 200],
    [255, 0, 128],
    [255, 96, 160],
    [255, 255, 0],
    [0, 255, 255],
    [0, 160, 255],
    [0, 80, 160],
    [160, 0, 0],
    [255, 255, 255],
];

/// Pastille de couleur de la palette. Le contour de sélection est composé
/// d'un anneau sombre extérieur et d'un anneau clair intérieur : il reste
/// lisible aussi bien sur le blanc que sur les couleurs très sombres.
fn paint_color_swatch(
    painter: &egui::Painter,
    rect: Rect,
    radius: u8,
    rgb: [u8; 3],
    selected: bool,
) {
    painter.rect_filled(
        rect,
        CornerRadius::same(radius),
        Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(radius),
        Stroke::new(1.0, Color32::from_rgb(190, 190, 190)),
        StrokeKind::Inside,
    );
    if selected {
        painter.rect_stroke(
            rect.expand(1.5),
            CornerRadius::same(radius.saturating_add(1)),
            Stroke::new(1.5, Color32::from_rgb(35, 35, 35)),
            StrokeKind::Inside,
        );
        painter.rect_stroke(
            rect.shrink(1.0),
            CornerRadius::same(radius.saturating_sub(1)),
            Stroke::new(1.2, Color32::WHITE),
            StrokeKind::Inside,
        );
    }
}

/// Bouton « couleur libre » : ouvre le sélecteur RGB/HSV (roue chromatique,
/// carré saturation/valeur, saisie numérique) proposé par egui.
fn color_well(ui: &mut egui::Ui, rgb: &mut [u8; 3], custom_selected: bool) -> egui::Response {
    let previous = ui.spacing().interact_size;
    ui.spacing_mut().interact_size = Vec2::new(30.0, 22.0);
    let response = egui::color_picker::color_edit_button_srgb(ui, rgb);
    ui.spacing_mut().interact_size = previous;
    ui.painter().rect_stroke(
        response.rect,
        CornerRadius::same(4),
        Stroke::new(1.0, Color32::from_rgb(150, 150, 150)),
        StrokeKind::Inside,
    );
    if custom_selected {
        ui.painter().rect_stroke(
            response.rect.expand(1.5),
            CornerRadius::same(5),
            Stroke::new(1.5, Color32::from_rgb(35, 35, 35)),
            StrokeKind::Inside,
        );
    }
    response.on_hover_text("Couleur personnalisée : roue chromatique et saisie RGB")
}

/// Petit bouton flèche dessiné à la main : la police par défaut du GUI ne
/// contient pas ↑/↓, ce qui affichait des carrés vides.
fn arrow_button(ui: &mut egui::Ui, up: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter()
        .rect_filled(rect, CornerRadius::same(4), visuals.weak_bg_fill);
    let center = rect.center();
    let half_w = 3.6;
    let half_h = 4.2;
    let tip = if up {
        pos2(center.x, center.y - half_h)
    } else {
        pos2(center.x, center.y + half_h)
    };
    let base_y = if up {
        center.y + half_h * 0.6
    } else {
        center.y - half_h * 0.6
    };
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            tip,
            pos2(center.x - half_w, base_y),
            pos2(center.x + half_w, base_y),
        ],
        Color32::from_rgb(55, 55, 55),
        Stroke::NONE,
    ));
    response.on_hover_text(if up {
        "Monter dans la liste des calques"
    } else {
        "Descendre dans la liste des calques"
    })
}

const HISTORY_LIMIT: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Lighting,
    Effects,
    Macros,
    Settings,
}

impl Page {
    fn from_env() -> Self {
        match std::env::var("BERSERKER_PAGE").ok().as_deref() {
            Some("effects") | Some("effets") => Self::Effects,
            Some("macros") => Self::Macros,
            Some("settings") | Some("reglages") => Self::Settings,
            _ => Self::Lighting,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EffectKind {
    Solid,
    Breathing,
    Wave,
    Cycle,
    Gradient,
    Ripple,
    Sparkle,
    Rain,
    Comet,
    Ember,
    Explosion,
}

impl EffectKind {
    const ALL: [Self; 11] = [
        Self::Solid,
        Self::Breathing,
        Self::Wave,
        Self::Cycle,
        Self::Gradient,
        Self::Ripple,
        Self::Sparkle,
        Self::Rain,
        Self::Comet,
        Self::Ember,
        Self::Explosion,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Solid => "Solide",
            Self::Breathing => "Respiration",
            Self::Wave => "Vague",
            Self::Cycle => "Cycle",
            Self::Gradient => "Dégradé",
            Self::Ripple => "Onde",
            Self::Sparkle => "Scintillement",
            Self::Rain => "Pluie",
            Self::Comet => "Comète",
            Self::Ember => "Braise",
            Self::Explosion => "Explosion",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Solid => "Couleur fixe sur la zone",
            Self::Breathing => "Pulsation douce",
            Self::Wave => "Vague qui traverse la zone",
            Self::Cycle => "Cycle de teintes (couleur ignorée)",
            Self::Gradient => "Dégradé animé",
            Self::Ripple => "Ondes concentriques",
            Self::Sparkle => "Scintillements aléatoires",
            Self::Rain => "Gouttes qui tombent",
            Self::Comet => "Comète qui balaie la zone",
            Self::Ember => "Braises vacillantes",
            Self::Explosion => "Ondes radiales périodiques",
        }
    }

    fn id(self) -> u8 {
        match self {
            Self::Solid => 0,
            Self::Breathing => 1,
            Self::Wave => 2,
            Self::Cycle => 3,
            Self::Gradient => 4,
            Self::Ripple => 5,
            Self::Sparkle => 6,
            Self::Rain => 7,
            Self::Comet => 8,
            Self::Ember => 9,
            Self::Explosion => 10,
        }
    }

    fn from_id(id: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlendMode {
    Add,
    Replace,
}

impl BlendMode {
    fn label(self) -> &'static str {
        match self {
            Self::Add => "Ajouter",
            Self::Replace => "Remplacer",
        }
    }

    fn id(self) -> u8 {
        match self {
            Self::Add => 0,
            Self::Replace => 1,
        }
    }

    fn from_id(id: u8) -> Self {
        if id == 1 { Self::Replace } else { Self::Add }
    }
}

const MAX_LAYERS: usize = 8;

#[derive(Clone, Debug)]
struct EffectLayer {
    kind: EffectKind,
    rgb: [u8; 3],
    speed: u8,
    reverse: bool,
    blend: BlendMode,
    enabled: bool,
    mask: [bool; protocol::LED_COUNT],
}

impl EffectLayer {
    fn new(kind: EffectKind, rgb: [u8; 3], speed: u8, mask: [bool; protocol::LED_COUNT]) -> Self {
        Self {
            kind,
            rgb,
            speed: speed.min(protocol::SPEED_UI_MAX),
            reverse: false,
            blend: BlendMode::Add,
            enabled: true,
            mask,
        }
    }

    fn zone_count(&self) -> usize {
        self.mask.iter().filter(|active| **active).count()
    }

    fn zone_label(&self) -> String {
        if self.zone_count() >= protocol::LED_COUNT {
            "Tout le clavier".to_string()
        } else {
            format!("{} touche(s)", self.zone_count())
        }
    }
}

#[derive(Clone)]
struct LiveRuntime {
    started: Instant,
    last_frame: Instant,
}

#[cfg(windows)]
fn app_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("BerserkerRGB")
}

#[cfg(target_os = "linux")]
fn app_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(dir).join("berserker-rgb")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".local/share/berserker-rgb")
    } else {
        PathBuf::from(".")
    }
}

#[cfg(windows)]
fn factory_macros_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Berserker")
        .join("keyboard")
}

#[cfg(target_os = "linux")]
fn factory_macros_dir() -> PathBuf {
    PathBuf::new()
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// État complet d'un profil, à la manière du logiciel d'usine : les couleurs
/// per-touche, mais aussi le mode d'éclairage, la luminosité, la vitesse, la
/// direction et les réglages système sont conservés et réappliqués.
#[derive(Clone, Copy, Debug)]
struct ProfileSnapshot {
    mode_index: usize,
    rgb: [u8; 3],
    brightness: u8,
    speed: u8,
    reverse: bool,
    poll: u8,
    deb: u8,
    win_lock: bool,
    arrow_mode: bool,
    anti_ghosting: bool,
}

#[derive(Clone)]
struct MacroEntry {
    name: String,
    path: PathBuf,
    events: usize,
    source: &'static str,
}

impl Default for ProfileSnapshot {
    fn default() -> Self {
        Self {
            mode_index: 0,
            rgb: [255, 96, 0],
            brightness: 255,
            speed: 2,
            reverse: false,
            poll: 0,
            deb: 0,
            win_lock: false,
            arrow_mode: false,
            anti_ghosting: true,
        }
    }
}

fn mode_value_from_index(index: usize) -> u8 {
    if index == 0 {
        protocol::MODE_CUSTOM
    } else {
        protocol::MODES
            .get(index - 1)
            .map(|(_, value)| *value)
            .unwrap_or(protocol::MODE_STATIC)
    }
}

fn mode_index_from_value(value: u8) -> usize {
    if value == 0 || value == protocol::MODE_CUSTOM {
        0
    } else {
        protocol::MODES
            .iter()
            .position(|(_, mode)| *mode == value)
            .map(|index| index + 1)
            .unwrap_or(0)
    }
}

fn write_profile_state(path: &std::path::Path, s: &ProfileSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = format!(
        "version=3\nmode={}\nmode_id={}\nrgb={:02X}{:02X}{:02X}\nbrightness={}\nspeed={}\nreverse={}\npoll={}\ndeb={}\nwin={}\narrow={}\nanti={}\n",
        s.mode_index,
        mode_value_from_index(s.mode_index),
        s.rgb[0],
        s.rgb[1],
        s.rgb[2],
        s.brightness,
        s.speed,
        s.reverse as u8,
        s.poll,
        s.deb,
        s.win_lock as u8,
        s.arrow_mode as u8,
        s.anti_ghosting as u8
    );
    let tmp = path.with_extension("state.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn read_profile_state(path: &std::path::Path) -> Option<ProfileSnapshot> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut snap = ProfileSnapshot::default();
    let mut seen = false;
    let mut mode_index = 0usize;
    let mut mode_id: Option<u8> = None;
    let mut version = 1u32;
    let mut speed_seen = false;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "version" => version = value.parse().unwrap_or(1),
            "mode" => mode_index = value.parse().ok()?,
            "mode_id" => mode_id = value.parse().ok(),
            "rgb" => {
                if let Some(rgb) = parse_rgb_hex(value) {
                    snap.rgb = rgb;
                    seen = true;
                }
            }
            "brightness" => snap.brightness = value.parse().ok()?,
            "speed" => {
                snap.speed = value.parse().ok()?;
                speed_seen = true;
            }
            "reverse" => snap.reverse = value == "1",
            "poll" => snap.poll = value.parse().ok()?,
            "deb" => snap.deb = value.parse().ok()?,
            "win" => snap.win_lock = value == "1",
            "arrow" => snap.arrow_mode = value == "1",
            "anti" => snap.anti_ghosting = value == "1",
            _ => {}
        }
    }
    // Jusqu'à la version 2, la vitesse était stockée dans l'échelle brute du
    // firmware (0 = rapide). Depuis la version 3, l'interface utilise
    // 0 = lent, 4 = rapide.
    if version < 3 && speed_seen {
        snap.speed = protocol::ui_speed(snap.speed);
    } else {
        snap.speed = snap.speed.min(protocol::SPEED_UI_MAX);
    }
    snap.mode_index = match mode_id {
        Some(value) => mode_index_from_value(value),
        None => protocol::LEGACY_MODE_INDEX
            .get(mode_index)
            .copied()
            .map(|index| index as usize)
            .unwrap_or(0),
    };
    if seen { Some(snap) } else { None }
}

/// Écrit les calques d'effets PC d'un profil.
fn write_layers_file(path: &std::path::Path, layers: &[EffectLayer]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = String::from("version=1\n");
    for layer in layers.iter().take(MAX_LAYERS) {
        text.push_str(&format!(
            "layer={};{};{};{};{};{:02X}{:02X}{:02X};{}\n",
            layer.kind.id(),
            layer.speed.min(protocol::SPEED_UI_MAX),
            layer.reverse as u8,
            layer.blend.id(),
            layer.enabled as u8,
            layer.rgb[0],
            layer.rgb[1],
            layer.rgb[2],
            mask_to_hex(&layer.mask),
        ));
    }
    let tmp = path.with_extension("layers.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn read_layers_file(path: &std::path::Path) -> Vec<EffectLayer> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut layers = Vec::new();
    for line in text.lines() {
        let Some(value) = line.strip_prefix("layer=") else {
            continue;
        };
        let fields: Vec<&str> = value.split(';').collect();
        if fields.len() < 7 {
            continue;
        }
        let Ok(id) = fields[0].parse::<u8>() else {
            continue;
        };
        let Some(kind) = EffectKind::from_id(id) else {
            continue;
        };
        let Ok(speed) = fields[1].parse::<u8>() else {
            continue;
        };
        let reverse = fields[2] == "1";
        let blend = BlendMode::from_id(fields[3].parse::<u8>().unwrap_or(0));
        let enabled = fields[4] != "0";
        let Some(rgb) = parse_rgb_hex(fields[5]) else {
            continue;
        };
        let mask = mask_from_hex(fields[6]);
        layers.push(EffectLayer {
            kind,
            rgb,
            speed: speed.min(protocol::SPEED_UI_MAX),
            reverse,
            blend,
            enabled,
            mask,
        });
        if layers.len() >= MAX_LAYERS {
            break;
        }
    }
    layers
}

fn data_to_colors(data: &[u8]) -> Vec<[u8; 3]> {
    let mut colors = vec![[0u8; 3]; protocol::LED_COUNT];
    for (i, chunk) in data.chunks_exact(3).enumerate().take(protocol::LED_COUNT) {
        colors[i] = [chunk[0], chunk[1], chunk[2]];
    }
    colors
}

fn colors_to_data(colors: &[[u8; 3]]) -> Vec<u8> {
    let mut data = Vec::with_capacity(protocol::CUSTOM_COLORSET_SIZE);
    for color in colors.iter().take(protocol::LED_COUNT) {
        data.extend_from_slice(color);
    }
    while data.len() < protocol::CUSTOM_COLORSET_SIZE {
        data.extend_from_slice(&[0, 0, 0]);
    }
    data
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = (h.rem_euclid(1.0)) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [
        (r * 255.0).clamp(0.0, 255.0) as u8,
        (g * 255.0).clamp(0.0, 255.0) as u8,
        (b * 255.0).clamp(0.0, 255.0) as u8,
    ]
}

fn preview_hash(index: usize, tick: f32) -> f32 {
    let mut value =
        (index as u32).wrapping_mul(0x9E37_79B1) ^ (tick as u32).wrapping_mul(0x85EB_CA6B);
    value ^= value >> 13;
    value = value.wrapping_mul(0xC2B2_AE35);
    value ^= value >> 16;
    (value % 1000) as f32 / 1000.0
}

fn led_positions(keys: &[layout::KeyDef]) -> Vec<(f32, f32)> {
    let mut positions = vec![(0.5, 0.5); protocol::LED_COUNT];
    let mut mapped = [false; protocol::LED_COUNT];
    for key in keys {
        if key.pos < protocol::LED_COUNT {
            positions[key.pos] = (
                ((key.x + key.w * 0.5 - layout::BODY_X) / layout::TOTAL_W).clamp(0.0, 1.0),
                ((key.y + key.h * 0.5 - layout::BODY_Y) / layout::TOTAL_H).clamp(0.0, 1.0),
            );
            mapped[key.pos] = true;
        }
    }
    for (i, position) in positions.iter_mut().enumerate() {
        if !mapped[i] {
            *position = (i as f32 / protocol::LED_COUNT as f32, 0.5);
        }
    }
    positions
}

fn mask_all() -> [bool; protocol::LED_COUNT] {
    [true; protocol::LED_COUNT]
}

fn selection_mask(
    keys: &[layout::KeyDef],
    selected: &HashSet<usize>,
) -> [bool; protocol::LED_COUNT] {
    if selected.is_empty() {
        return mask_all();
    }
    let mut mask = [false; protocol::LED_COUNT];
    for &index in selected {
        if let Some(key) = keys.get(index) {
            if key.pos < protocol::LED_COUNT {
                mask[key.pos] = true;
            }
        }
    }
    mask
}

fn mask_to_hex(mask: &[bool; protocol::LED_COUNT]) -> String {
    let mut bytes = [0u8; protocol::LED_COUNT.div_ceil(8)];
    for (i, active) in mask.iter().enumerate() {
        if *active {
            bytes[i / 8] |= 1 << (i % 8);
        }
    }
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

fn mask_from_hex(text: &str) -> [bool; protocol::LED_COUNT] {
    let mut mask = [false; protocol::LED_COUNT];
    let mut nibble: Option<u8> = None;
    let mut index = 0usize;
    for ch in text.chars() {
        let Some(value) = ch.to_digit(16) else {
            continue;
        };
        match nibble.take() {
            None => nibble = Some(value as u8),
            Some(high) => {
                let byte = (high << 4) | value as u8;
                for bit in 0..8 {
                    if index < protocol::LED_COUNT {
                        mask[index] = byte & (1 << bit) != 0;
                    }
                    index += 1;
                }
            }
        }
    }
    mask
}

/// Couleurs d'un calque sur tout le clavier, à partir des positions physiques
/// des LED (0..1 en x et y).
fn effect_frame_colors(
    layer: &EffectLayer,
    elapsed: f32,
    positions: &[(f32, f32)],
) -> Vec<[u8; 3]> {
    let mut colors = vec![[0u8; 3]; protocol::LED_COUNT];
    let base = layer.rgb;
    let rate = 0.35 + layer.speed.min(protocol::SPEED_UI_MAX) as f32 * 0.35;
    let t = elapsed * rate;
    let dir = if layer.reverse { -1.0 } else { 1.0 };
    let shade = |factor: f32| -> [u8; 3] {
        [
            (base[0] as f32 * factor).clamp(0.0, 255.0) as u8,
            (base[1] as f32 * factor).clamp(0.0, 255.0) as u8,
            (base[2] as f32 * factor).clamp(0.0, 255.0) as u8,
        ]
    };
    match layer.kind {
        EffectKind::Solid => colors.fill(base),
        EffectKind::Breathing => {
            let pulse = 0.12 + 0.88 * (0.5 + 0.5 * (t * 2.2).sin());
            colors.fill(shade(pulse));
        }
        EffectKind::Wave => {
            for (i, color) in colors.iter_mut().enumerate() {
                let x = positions[i].0;
                let phase = (x * 3.0 - dir * t * 2.6) * std::f32::consts::TAU;
                let intensity = 0.15 + 0.85 * (0.5 + 0.5 * phase.sin());
                *color = shade(intensity);
            }
        }
        EffectKind::Cycle => {
            for (i, color) in colors.iter_mut().enumerate() {
                let x = positions[i].0;
                let hue = (x * 0.9 + dir * t * 0.12).rem_euclid(1.0);
                *color = hsv_to_rgb(hue, 1.0, 1.0);
            }
        }
        EffectKind::Gradient => {
            let complement = [255 - base[0], 255 - base[1], 255 - base[2]];
            for (i, color) in colors.iter_mut().enumerate() {
                let x = positions[i].0;
                let phase = x * std::f32::consts::TAU - dir * t * 2.0;
                let mix = 0.5 + 0.5 * phase.sin();
                *color = [
                    (base[0] as f32 * mix + complement[0] as f32 * (1.0 - mix)) as u8,
                    (base[1] as f32 * mix + complement[1] as f32 * (1.0 - mix)) as u8,
                    (base[2] as f32 * mix + complement[2] as f32 * (1.0 - mix)) as u8,
                ];
            }
        }
        EffectKind::Ripple => {
            for (i, color) in colors.iter_mut().enumerate() {
                let (x, y) = positions[i];
                let radius = ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt();
                let phase = (radius * 10.0 - dir * t * 3.0) * std::f32::consts::TAU;
                let intensity = 0.12 + 0.88 * (0.5 + 0.5 * phase.sin());
                *color = shade(intensity);
            }
        }
        EffectKind::Sparkle => {
            for (i, color) in colors.iter_mut().enumerate() {
                let spark = preview_hash(i, (t * 2.2).floor());
                if spark > 0.78 {
                    let twinkle = 0.35 + 0.65 * preview_hash(i.wrapping_mul(7) ^ 0x9E37, t * 5.0);
                    *color = shade(twinkle);
                } else {
                    *color = shade(0.02);
                }
            }
        }
        EffectKind::Rain => {
            for (i, color) in colors.iter_mut().enumerate() {
                let (x, y) = positions[i];
                let column = (x * 12.0).floor() as usize;
                let phase = preview_hash(column, 0.0);
                let fall = (y + dir * t * 0.9 + phase).rem_euclid(1.0);
                let intensity = if fall < 0.22 {
                    0.15 + 0.85 * (1.0 - fall / 0.22)
                } else {
                    0.02
                };
                *color = shade(intensity);
            }
        }
        EffectKind::Comet => {
            let head = (dir * t * 0.35).rem_euclid(1.2) - 0.1;
            for (i, color) in colors.iter_mut().enumerate() {
                let x = positions[i].0;
                let distance = (x - head).abs();
                let intensity = (1.0 - distance * 4.0).max(0.0);
                *color = shade(intensity);
            }
        }
        EffectKind::Ember => {
            for (i, color) in colors.iter_mut().enumerate() {
                let (_, y) = positions[i];
                let noise = preview_hash(i, (t * 6.0).floor());
                let lift = 0.55 + 0.45 * y;
                let intensity = (0.25 + 0.75 * noise) * lift;
                let warm = (25.0 * noise * lift) as u8;
                *color = [
                    (base[0] as f32 * intensity + warm as f32).clamp(0.0, 255.0) as u8,
                    (base[1] as f32 * intensity + (warm as f32 * 0.35)).clamp(0.0, 255.0) as u8,
                    (base[2] as f32 * intensity).clamp(0.0, 255.0) as u8,
                ];
            }
        }
        EffectKind::Explosion => {
            let phase = (t * 0.6).rem_euclid(1.0);
            let front = phase * 0.75;
            for (i, color) in colors.iter_mut().enumerate() {
                let (x, y) = positions[i];
                let radius = ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt();
                let band = (radius - front).abs();
                let burst = (1.0 - phase) * (1.0 - band * 9.0).max(0.0);
                *color = shade(0.03 + burst);
            }
        }
    }
    colors
}

/// Compose les calques sur une couleur de base (effet firmware ou couleurs
/// per-touche) ; chacun ne s'applique qu'à sa zone et selon son mode de mélange.
fn compose_effect_layers(
    layers: &[EffectLayer],
    elapsed: f32,
    base: &[[u8; 3]],
    positions: &[(f32, f32)],
) -> Vec<[u8; 3]> {
    let mut out = base.to_vec();
    for layer in layers {
        if !layer.enabled {
            continue;
        }
        let effect = effect_frame_colors(layer, elapsed, positions);
        for i in 0..protocol::LED_COUNT {
            if !layer.mask[i] {
                continue;
            }
            match layer.blend {
                BlendMode::Add => {
                    for channel in 0..3 {
                        out[i][channel] = out[i][channel].saturating_add(effect[i][channel]);
                    }
                }
                BlendMode::Replace => out[i] = effect[i],
            }
        }
    }
    out
}

/// Analyse une couleur RRGGBB. La validation ASCII précède toute découpe :
/// une chaîne UTF-8 de même longueur en octets ne peut pas provoquer de
/// découpe au milieu d'un caractère.
fn parse_rgb_hex(text: &str) -> Option<[u8; 3]> {
    let s = text.trim().trim_start_matches('#');
    if s.len() != 6 || !s.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&s[0..2], 16).ok()?,
        u8::from_str_radix(&s[2..4], 16).ok()?,
        u8::from_str_radix(&s[4..6], 16).ok()?,
    ])
}

fn parse_hex_color(s: &str) -> Result<[u8; 3]> {
    parse_rgb_hex(s)
        .ok_or_else(|| anyhow!("couleur attendue au format RRGGBB (6 chiffres hexadécimaux)"))
}

// ---------------------------------------------------------------------------
// CLI de diagnostic
// ---------------------------------------------------------------------------

fn cli_apply(rgb: [u8; 3]) -> Result<()> {
    let controller = Controller::open()?;
    controller.set_static_color(rgb)?;
    println!(
        "Couleur appliquée : #{:02X}{:02X}{:02X}",
        rgb[0], rgb[1], rgb[2]
    );
    Ok(())
}

fn cli_caps() -> Result<()> {
    let controller = Controller::open()?;
    let caps = controller.read_capabilities()?;
    let hex: Vec<String> = caps.iter().map(|b| format!("{b:02X}")).collect();
    println!("capacités ({} octets) : {}", caps.len(), hex.join(" "));
    if caps.len() >= 6 {
        println!("map_size (octet 5) = {}", caps[5]);
        println!(
            "macros_size (octet 6) = {}",
            caps.get(6).copied().unwrap_or(0)
        );
    }
    Ok(())
}

fn cli_config() -> Result<()> {
    let controller = Controller::open()?;
    let cfg = controller.read_config(1, 18)?;
    let hex: Vec<String> = cfg.iter().map(|b| format!("{b:02X}")).collect();
    println!("config profil courant : {}", hex.join(" "));
    if cfg.len() >= 8 {
        println!(
            "mode=0x{:02X} lum={} vit={} (octet firmware {}, 0=rapide) dir={} couleur=#{:02X}{:02X}{:02X}",
            cfg[0],
            cfg[1],
            protocol::ui_speed(cfg[2]),
            cfg[2],
            cfg[3],
            cfg[5],
            cfg[6],
            cfg[7]
        );
    }
    Ok(())
}

fn cli_dump_config(offset: u16, size: usize) -> Result<()> {
    if size == 0 || offset as usize + size > 0x1_0000 {
        return Err(anyhow!("plage de configuration invalide"));
    }
    let controller = Controller::open()?;
    let data = controller.read_config_block(offset, size)?;
    if data.len() < size {
        return Err(anyhow!(
            "réponse incomplète : {} octets reçus sur {size}",
            data.len()
        ));
    }
    for (i, chunk) in data.chunks(16).enumerate() {
        let values: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
        println!("{:04X}: {}", offset as usize + i * 16, values.join(" "));
    }
    println!("{size} octets lus");
    Ok(())
}

fn cli_map() -> Result<()> {
    let controller = Controller::open()?;
    let map = controller.read_key_map()?;
    println!("table firmware ({} entrées) :", map.len());
    for row in 0..6 {
        let start = row * 21;
        let values: Vec<String> = map[start..start + 21]
            .iter()
            .map(|v| format!("{v:3}"))
            .collect();
        println!("{}", values.join(" "));
    }
    let unique = map.iter().collect::<HashSet<_>>().len();
    println!("valeurs distinctes : {unique}/{}", map.len());
    Ok(())
}

fn cli_backup_all(dir: &str) -> Result<()> {
    let controller = Controller::open()?;
    let dir = std::path::Path::new(dir);
    std::fs::create_dir_all(dir)?;
    for set in 0..protocol::CUSTOM_COLORSET_COUNT {
        let data = controller.read_custom_colors(set, protocol::CUSTOM_COLORSET_SIZE)?;
        let path = dir.join(format!("profile_{set}.bin"));
        std::fs::write(&path, &data)?;
        println!("profil {set} -> {}", path.display());
    }
    let config = controller.read_config(1, 18)?;
    let path = dir.join("config_profil_courant.bin");
    std::fs::write(&path, &config)?;
    println!("configuration -> {}", path.display());
    Ok(())
}

fn cli_custom_mode(set: u8) -> Result<()> {
    let controller = Controller::open()?;
    controller.set_custom_mode(set, 255, 128, 0)?;
    println!("mode Custom activé sur le jeu {set}");
    Ok(())
}

fn cli_profile_switch(index: u8) -> Result<()> {
    let mut app = App::new();
    if app.controller.is_none() {
        return Err(anyhow!(app.status));
    }
    println!("avant : profil {} · {}", app.profile + 1, app.status);
    app.switch_profile(index);
    println!("après : profil {} · {}", app.profile + 1, app.status);
    if app.status_error {
        return Err(anyhow!(app.status));
    }
    if !app.device_matches(&app.snapshot()) {
        return Err(anyhow!("le clavier ne correspond pas au profil appliqué"));
    }
    println!("OK : clavier conforme au profil {}.", app.profile + 1);
    Ok(())
}

fn cli_get_system() -> Result<()> {
    let controller = Controller::open()?;
    let (poll, deb, win, arrow, anti) = controller.read_system_settings()?;
    println!(
        "fréquence USB (0x16) = {poll} ({})",
        [1000, 500, 250, 125][(poll as usize).min(3)]
    );
    println!(
        "debounce (0x19) = {deb} ({} ms)",
        [5, 10, 15, 20][(deb as usize).min(3)]
    );
    println!("Mode Jeu (0x15) = {win}");
    println!("Mode flèche (0x13) = {arrow}");
    println!("Anti-ghosting (0x14) = {anti}");
    Ok(())
}

fn cli_set_system(poll_idx: usize, deb_idx: usize) -> Result<()> {
    if poll_idx > 3 {
        return Err(anyhow!("index fréquence 0..3 attendu"));
    }
    if deb_idx > 3 {
        return Err(anyhow!("index debounce 0..3 attendu"));
    }
    let controller = Controller::open()?;
    let (_, _, win, arrow, anti) = controller.read_system_settings()?;
    controller.apply_system_settings(poll_idx as u8, deb_idx as u8, win, arrow, anti)?;
    println!(
        "réglages appliqués : {} Hz, debounce {} ms",
        [1000, 500, 250, 125][poll_idx],
        [5, 10, 15, 20][deb_idx]
    );
    Ok(())
}

fn cli_set_flags(args: &[String]) -> Result<()> {
    // --set-flags <mode_jeu 0|1> <mode_fleche 0|1> <anti_ghosting 0|1>
    let win = parse_bool(args.first())?;
    let arrow = parse_bool(args.get(1))?;
    let anti = parse_bool(args.get(2))?;
    let controller = Controller::open()?;
    let (poll, deb, _, _, _) = controller.read_system_settings()?;
    controller.apply_system_settings(poll, deb, win, arrow, anti)?;
    println!(
        "options appliquées : Mode Jeu {}, Mode flèche {}, Anti-ghosting {}",
        win as u8, arrow as u8, anti as u8
    );
    Ok(())
}

fn parse_bool(value: Option<&String>) -> Result<bool> {
    match value.map(|s| s.as_str()) {
        Some("1") | Some("on") | Some("true") => Ok(true),
        Some("0") | Some("off") | Some("false") => Ok(false),
        _ => Err(anyhow!("valeur booléenne 0/1 attendue")),
    }
}

fn cli_direct_test(rgb: [u8; 3], seconds: u64) -> Result<()> {
    let controller = Controller::open()?;
    let mut frame = vec![0u8; protocol::CUSTOM_COLORSET_SIZE];
    for pixel in frame.chunks_exact_mut(3) {
        pixel.copy_from_slice(&rgb);
    }
    println!(
        "Envoi direct #{:02X}{:02X}{:02X} pendant {} s (non persistant)",
        rgb[0], rgb[1], rgb[2], seconds
    );
    let mut first_error = None;
    let frames = (seconds.saturating_mul(10)).max(1);
    for _ in 0..frames {
        if let Err(error) = controller.send_direct_frame(&frame) {
            first_error = Some(error);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let end_result = controller.end_direct();
    if let Some(error) = first_error {
        return Err(error);
    }
    end_result?;
    println!("Mode direct terminé, retour à l'effet stocké.");
    Ok(())
}

fn build_custom(bg: [u8; 3], spots: &[(usize, [u8; 3])], map: Option<&[u8]>) -> Vec<u8> {
    let mut data = vec![0u8; protocol::CUSTOM_COLORSET_SIZE];
    for index in 0..protocol::LED_COUNT {
        let rgb = spots
            .iter()
            .find(|(i, _)| *i == index)
            .map(|(_, c)| *c)
            .unwrap_or(bg);
        let position = map
            .and_then(|m| m.get(index).copied())
            .map(|p| p as usize)
            .filter(|p| *p < protocol::LED_COUNT)
            .unwrap_or(index);
        data[position * 3] = rgb[0];
        data[position * 3 + 1] = rgb[1];
        data[position * 3 + 2] = rgb[2];
    }
    data
}

fn cli_custom_read(set: u8, path: &str) -> Result<()> {
    let controller = Controller::open()?;
    let data = controller.read_custom_colors(set, protocol::CUSTOM_COLORSET_SIZE)?;
    std::fs::write(path, &data)?;
    println!("{} octets lus (jeu {set}) -> {path}", data.len());
    Ok(())
}

fn cli_custom_write(set: u8, path: &str, activate: bool) -> Result<()> {
    let data = std::fs::read(path)?;
    if data.len() != protocol::CUSTOM_COLORSET_SIZE {
        return Err(anyhow!(
            "taille inattendue : {} (attendu {})",
            data.len(),
            protocol::CUSTOM_COLORSET_SIZE
        ));
    }
    let controller = Controller::open()?;
    controller.write_custom_colors(set, &data)?;
    if activate {
        controller.set_custom_mode(set, 255, 128, 0)?;
    }
    println!(
        "jeu {set} écrit ({} octets){}",
        data.len(),
        if activate {
            " + mode Custom activé"
        } else {
            ""
        }
    );
    Ok(())
}

fn cli_custom_solid(set: u8, rgb: [u8; 3]) -> Result<()> {
    let controller = Controller::open()?;
    let data = build_custom(rgb, &[], None);
    controller.write_custom_colors(set, &data)?;
    controller.set_custom_mode(set, 255, 128, 0)?;
    println!(
        "jeu {set} : 126 touches en #{:02X}{:02X}{:02X}, mode Custom activé",
        rgb[0], rgb[1], rgb[2]
    );
    Ok(())
}

fn parse_spots(spec: &str) -> Result<Vec<(usize, [u8; 3])>> {
    let mut spots = Vec::new();
    for part in spec.split(',') {
        let (idx, hex) = part
            .split_once(':')
            .ok_or_else(|| anyhow!("format attendu INDEX:RRGGBB"))?;
        spots.push((idx.trim().parse::<usize>()?, parse_hex_color(hex)?));
    }
    Ok(spots)
}

fn cli_custom_keys(set: u8, bg: [u8; 3], spec: &str) -> Result<()> {
    let spots = parse_spots(spec)?;
    let controller = Controller::open()?;
    let map = controller.read_key_map()?;
    let data = build_custom(bg, &spots, Some(&map));
    controller.write_custom_colors(set, &data)?;
    controller.set_custom_mode(set, 255, 128, 0)?;
    println!(
        "jeu {set} : fond #{:02X}{:02X}{:02X}, {} touche(s) personnalisée(s), mode Custom activé",
        bg[0],
        bg[1],
        bg[2],
        spots.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Application graphique
// ---------------------------------------------------------------------------

/// L'exe Windows de production est en sous-système GUI : aucune fenêtre
/// console ne s'ouvre. Pour les options CLI, on se rattache à la console du
/// parent et on réinstalle les poignées standard avant le premier affichage.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;
    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;
    const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const OPEN_EXISTING: u32 = 3;
    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            creation: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn GetStdHandle(std_handle: u32) -> *mut c_void;
        fn SetStdHandle(std_handle: u32, handle: *mut c_void) -> i32;
    }

    let valid = |handle: *mut c_void| !handle.is_null() && handle != INVALID_HANDLE_VALUE;
    unsafe {
        // Ne pas écraser une sortie déjà redirigée vers un fichier/pipe.
        let current_out = GetStdHandle(STD_OUTPUT_HANDLE);
        let current_err = GetStdHandle(STD_ERROR_HANDLE);
        let current_in = GetStdHandle(STD_INPUT_HANDLE);
        let need_out = !valid(current_out);
        let need_err = !valid(current_err);
        let need_in = !valid(current_in);
        if !(need_out || need_err || need_in) {
            return;
        }
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        let open = |name: &[u16], access: u32| unsafe {
            CreateFileW(
                name.as_ptr(),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null_mut(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        let out: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        let inp: Vec<u16> = "CONIN$\0".encode_utf16().collect();
        if need_out || need_err {
            let hout = open(&out, GENERIC_READ | GENERIC_WRITE);
            if valid(hout) {
                if need_out {
                    SetStdHandle(STD_OUTPUT_HANDLE, hout);
                }
                if need_err {
                    SetStdHandle(STD_ERROR_HANDLE, hout);
                }
            }
        }
        if need_in {
            let hin = open(&inp, GENERIC_READ | GENERIC_WRITE);
            if valid(hin) {
                SetStdHandle(STD_INPUT_HANDLE, hin);
            }
        }
    }
}

fn main() {
    #[cfg(all(windows, not(debug_assertions)))]
    if std::env::args_os().len() > 1 {
        attach_parent_console();
    }
    let args: Vec<String> = std::env::args().collect();

    if let Some(pos) = args.iter().position(|a| a == "--apply") {
        let Some(hex) = args.get(pos + 1) else {
            eprintln!("Usage: berserker-rgb --apply RRGGBB");
            std::process::exit(2);
        };
        let rgb = match parse_hex_color(hex) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Erreur: {e}");
                std::process::exit(2);
            }
        };
        if let Err(e) = cli_apply(rgb) {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if args.iter().any(|a| a == "--caps") {
        if let Err(e) = cli_caps() {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if args.iter().any(|a| a == "--config") {
        if let Err(e) = cli_config() {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if args.iter().any(|a| a == "--map") {
        if let Err(e) = cli_map() {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--backup-all") {
        let Some(dir) = args.get(pos + 1) else {
            eprintln!("Usage: berserker-rgb --backup-all <dossier>");
            std::process::exit(2);
        };
        if let Err(e) = cli_backup_all(dir) {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--custom-mode") {
        let Some(set) = args.get(pos + 1).and_then(|s| s.parse().ok()) else {
            eprintln!("Usage: berserker-rgb --custom-mode <jeu 0-9>");
            std::process::exit(2);
        };
        if let Err(e) = cli_custom_mode(set) {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--direct-test") {
        let Some(rgb) = args.get(pos + 1).and_then(|s| parse_hex_color(s).ok()) else {
            eprintln!("Usage: berserker-rgb --direct-test RRGGBB [secondes]");
            std::process::exit(2);
        };
        let seconds = args.get(pos + 2).and_then(|s| s.parse().ok()).unwrap_or(3);
        if let Err(e) = cli_direct_test(rgb, seconds) {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--dump-config") {
        let offset = args.get(pos + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let size = args
            .get(pos + 2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(128);
        if let Err(e) = cli_dump_config(offset, size) {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--effect") {
        let index = args.get(pos + 1).and_then(|s| s.parse::<usize>().ok());
        let speed = args
            .get(pos + 2)
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(2)
            .min(protocol::SPEED_UI_MAX);
        let brightness = args
            .get(pos + 3)
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(255);
        match index {
            Some(index) if index >= 1 && index <= MODES.len() => {
                let controller = match Controller::open() {
                    Ok(controller) => controller,
                    Err(e) => {
                        eprintln!("Erreur: {e:#}");
                        std::process::exit(1);
                    }
                };
                let (name, mode) = MODES[index - 1];
                if let Err(e) = controller.set_mode(
                    mode,
                    [255, 255, 255],
                    brightness,
                    protocol::firmware_speed(speed),
                    0,
                ) {
                    eprintln!("Erreur: {e:#}");
                    std::process::exit(1);
                }
                println!(
                    "effet « {name} » (0x{mode:02X}) appliqué, vitesse {speed} (0=lent, 4=rapide), luminosité {brightness}"
                );
            }
            _ => {
                eprintln!(
                    "Usage: berserker-rgb --effect <index 1-{}> [vitesse 0=lent..4=rapide] [luminosité]",
                    MODES.len()
                );
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--profile-switch") {
        let index = args.get(pos + 1).and_then(|s| s.parse::<u8>().ok());
        match index {
            Some(index) if index <= 9 => {
                if let Err(e) = cli_profile_switch(index) {
                    eprintln!("Erreur: {e:#}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Usage: berserker-rgb --profile-switch <0-9>");
                std::process::exit(2);
            }
        }
        return;
    }

    if args.iter().any(|a| a == "--get-system") {
        if let Err(e) = cli_get_system() {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--set-system") {
        let poll_idx = args.get(pos + 1).and_then(|s| s.parse().ok());
        let deb_idx = args.get(pos + 2).and_then(|s| s.parse().ok());
        match (poll_idx, deb_idx) {
            (Some(poll), Some(deb)) => {
                if let Err(e) = cli_set_system(poll, deb) {
                    eprintln!("Erreur: {e:#}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Usage: berserker-rgb --set-system <freq 0-3> <debounce 0-3>");
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--set-flags") {
        let rest = &args[(pos + 1).min(args.len())..];
        if let Err(e) = cli_set_flags(rest) {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--custom-read") {
        let set = args.get(pos + 1).and_then(|s| s.parse().ok());
        let path = args.get(pos + 2);
        match (set, path) {
            (Some(set), Some(path)) => {
                if let Err(e) = cli_custom_read(set, path) {
                    eprintln!("Erreur: {e:#}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Usage: berserker-rgb --custom-read <jeu 0-9> <fichier.bin>");
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--custom-write") {
        let set = args.get(pos + 1).and_then(|s| s.parse().ok());
        let path = args.get(pos + 2);
        match (set, path) {
            (Some(set), Some(path)) => {
                if let Err(e) = cli_custom_write(set, path, true) {
                    eprintln!("Erreur: {e:#}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Usage: berserker-rgb --custom-write <jeu 0-9> <fichier.bin>");
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--custom-solid") {
        let set = args.get(pos + 1).and_then(|s| s.parse().ok());
        let rgb = args.get(pos + 2).and_then(|s| parse_hex_color(s).ok());
        match (set, rgb) {
            (Some(set), Some(rgb)) => {
                if let Err(e) = cli_custom_solid(set, rgb) {
                    eprintln!("Erreur: {e:#}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Usage: berserker-rgb --custom-solid <jeu 0-9> RRGGBB");
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--custom-keys") {
        let set = args.get(pos + 1).and_then(|s| s.parse().ok());
        let bg = args.get(pos + 2).and_then(|s| parse_hex_color(s).ok());
        let spec = args.get(pos + 3);
        match (set, bg, spec) {
            (Some(set), Some(bg), Some(spec)) => {
                if let Err(e) = cli_custom_keys(set, bg, spec) {
                    eprintln!("Erreur: {e:#}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!(
                    "Usage: berserker-rgb --custom-keys <jeu 0-9> <fond RRGGBB> INDEX:RRGGBB[,INDEX:RRGGBB...]"
                );
                std::process::exit(2);
            }
        }
        return;
    }

    if args.iter().any(|a| a == "--off") {
        if let Err(e) = cli_apply([0, 0, 0]) {
            eprintln!("Erreur: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = run_gui() {
        eprintln!("Erreur GUI: {e}");
        std::process::exit(1);
    }
}

/// Configuration wgpu Windows : Direct3D 12 natif, GPU matériel en priorité,
/// puis WARP (rastériseur logiciel fourni par Windows) si aucun GPU n'est
/// disponible. Aucune DLL tierce n'est nécessaire.
#[cfg(windows)]
fn wgpu_options() -> eframe::WgpuConfiguration {
    use eframe::wgpu;

    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::DX12;
    setup.native_adapter_selector = Some(std::sync::Arc::new(|adapters, surface| {
        let mut best: Option<&wgpu::Adapter> = None;
        let mut best_rank = u8::MAX;
        for adapter in adapters {
            if let Some(surface) = surface {
                if !adapter.is_surface_supported(surface) {
                    continue;
                }
            }
            let rank = match adapter.get_info().device_type {
                wgpu::DeviceType::DiscreteGpu => 0,
                wgpu::DeviceType::IntegratedGpu => 1,
                wgpu::DeviceType::VirtualGpu => 2,
                wgpu::DeviceType::Other => 3,
                wgpu::DeviceType::Cpu => 4,
            };
            if rank < best_rank {
                best = Some(adapter);
                best_rank = rank;
            }
        }
        if std::env::var_os("BERSERKER_GPU_LOG").is_some() {
            for adapter in adapters {
                let info = adapter.get_info();
                eprintln!(
                    "wgpu : {} · {:?} · {:?}",
                    info.name, info.device_type, info.backend
                );
            }
            if let Some(best) = best {
                eprintln!("wgpu : sélection {}", best.get_info().name);
            }
        }
        best.cloned()
            .ok_or_else(|| "aucun adaptateur Direct3D 12 compatible avec la fenêtre".to_string())
    }));
    eframe::WgpuConfiguration {
        wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

fn run_gui() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1180.0, 880.0])
        .with_min_inner_size([980.0, 740.0])
        .with_title("Berserker RGB — Contrôle clavier");
    if std::env::var_os("BERSERKER_TOPMOST").is_some() {
        viewport = viewport.with_always_on_top();
    }
    #[cfg(windows)]
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_options(),
        ..Default::default()
    };
    #[cfg(not(windows))]
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Berserker RGB",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            cc.egui_ctx.global_style_mut(|style| {
                style.spacing.item_spacing = egui::vec2(8.0, 6.0);
                style.text_styles.insert(
                    egui::TextStyle::Heading,
                    FontId::new(20.0, FontFamily::Proportional),
                );
                style.text_styles.insert(
                    egui::TextStyle::Body,
                    FontId::new(13.0, FontFamily::Proportional),
                );
                style.text_styles.insert(
                    egui::TextStyle::Button,
                    FontId::new(13.0, FontFamily::Proportional),
                );
                style.text_styles.insert(
                    egui::TextStyle::Small,
                    FontId::new(11.0, FontFamily::Proportional),
                );
            });
            Ok(Box::new(App::new()))
        }),
    )
}

struct App {
    controller: Option<Controller>,
    colors: Vec<[u8; 3]>,
    keys: Vec<layout::KeyDef>,
    selected: HashSet<usize>,
    profile: u8,
    page: Page,
    mode_index: usize,
    rgb: [u8; 3],
    brightness: u8,
    speed: u8,
    reverse: bool,
    status: String,
    status_error: bool,
    drag_start: Option<Pos2>,
    drag_current: Option<Pos2>,
    undo_stack: Vec<Vec<[u8; 3]>>,
    redo_stack: Vec<Vec<[u8; 3]>>,
    backup_dir: PathBuf,
    profile_dir: PathBuf,
    pre_write_backup: Option<PathBuf>,
    last_backup: Option<PathBuf>,
    layers: Vec<EffectLayer>,
    selected_layer: Option<usize>,
    live: Option<LiveRuntime>,
    live_colors: Option<Vec<[u8; 3]>>,
    window_hidden: bool,
    polling_index: usize,
    debounce_index: usize,
    win_lock: bool,
    arrow_mode: bool,
    anti_ghosting: bool,
    macros: Vec<MacroEntry>,
    macro_selected: Option<usize>,
    macros_dir: PathBuf,
    factory_macros_dir: PathBuf,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            controller: None,
            colors: vec![[0, 0, 0]; protocol::LED_COUNT],
            keys: layout::keys(),
            selected: HashSet::new(),
            profile: 0,
            page: Page::from_env(),
            mode_index: 0,
            rgb: [255, 96, 0],
            brightness: 255,
            speed: 2,
            reverse: false,
            status: String::new(),
            status_error: false,
            drag_start: None,
            drag_current: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            backup_dir: app_data_dir().join("backups"),
            profile_dir: app_data_dir().join("profiles"),
            pre_write_backup: None,
            last_backup: None,
            layers: Vec::new(),
            selected_layer: None,
            live: None,
            live_colors: None,
            window_hidden: false,
            polling_index: 0,
            debounce_index: 0,
            win_lock: false,
            arrow_mode: false,
            anti_ghosting: true,
            macros: Vec::new(),
            macro_selected: None,
            macros_dir: app_data_dir().join("macros"),
            factory_macros_dir: factory_macros_dir(),
        };
        let _ = std::fs::create_dir_all(&app.backup_dir);
        let _ = std::fs::create_dir_all(&app.profile_dir);
        let _ = std::fs::create_dir_all(&app.macros_dir);
        app.refresh_macros();

        match Controller::open() {
            Ok(controller) => {
                app.controller = Some(controller);
                app.restore_profile(true);
                if !app.status_error {
                    app.status = format!("Clavier connecté · {}", app.status);
                }
            }
            Err(err) => {
                app.status = format!("Clavier introuvable : {err:#}");
                app.status_error = true;
            }
        }
        app
    }

    fn snapshot(&self) -> ProfileSnapshot {
        ProfileSnapshot {
            mode_index: self.mode_index,
            rgb: self.rgb,
            brightness: self.brightness,
            speed: self.speed,
            reverse: self.reverse,
            poll: self.polling_index as u8,
            deb: self.debounce_index as u8,
            win_lock: self.win_lock,
            arrow_mode: self.arrow_mode,
            anti_ghosting: self.anti_ghosting,
        }
    }

    fn apply_snapshot(&mut self, snap: &ProfileSnapshot) {
        self.mode_index = snap.mode_index.min(MODES.len());
        self.rgb = snap.rgb;
        self.brightness = snap.brightness;
        self.speed = snap.speed.min(protocol::SPEED_UI_MAX);
        self.reverse = snap.reverse;
        self.polling_index = (snap.poll as usize).min(3);
        self.debounce_index = (snap.deb as usize).min(3);
        self.win_lock = snap.win_lock;
        self.arrow_mode = snap.arrow_mode;
        self.anti_ghosting = snap.anti_ghosting;
    }

    fn profile_file(&self, index: u8) -> PathBuf {
        self.profile_dir.join(format!("profil_{}.bin", index + 1))
    }

    fn state_file(&self, index: u8) -> PathBuf {
        self.profile_dir.join(format!("profil_{}.state", index + 1))
    }

    fn layers_file(&self, index: u8) -> PathBuf {
        self.profile_dir
            .join(format!("profil_{}.layers", index + 1))
    }

    fn save_layers(&self) -> Result<()> {
        write_layers_file(&self.layers_file(self.profile), &self.layers)
    }

    fn active_index_file(&self) -> PathBuf {
        self.profile_dir.join("actif.cfg")
    }

    /// Relit l'état réel du clavier (mode, couleurs affichées, réglages).
    fn read_device_snapshot(&self) -> Option<(ProfileSnapshot, Vec<[u8; 3]>)> {
        let controller = self.controller.as_ref()?;
        let cfg = controller.read_config(1, 18).ok()?;
        if cfg.len() < 8 {
            return None;
        }
        let mode_index = mode_index_from_value(cfg[0]);
        let (poll, deb, win, arrow, anti) = controller.read_system_settings().ok()?;
        let data = controller
            .read_custom_colors(0, protocol::CUSTOM_COLORSET_SIZE)
            .ok()?;
        Some((
            ProfileSnapshot {
                mode_index,
                rgb: [cfg[5], cfg[6], cfg[7]],
                brightness: cfg[1],
                speed: protocol::ui_speed(cfg[2]),
                reverse: cfg[3] != 0,
                poll,
                deb,
                win_lock: win,
                arrow_mode: arrow,
                anti_ghosting: anti,
            },
            data_to_colors(&data),
        ))
    }

    /// Vrai si le clavier affiche déjà exactement ce profil.
    fn device_matches(&self, snap: &ProfileSnapshot) -> bool {
        let Some(controller) = &self.controller else {
            return false;
        };
        let Ok(data) = controller.read_custom_colors(0, protocol::CUSTOM_COLORSET_SIZE) else {
            return false;
        };
        if data_to_colors(&data) != self.colors {
            return false;
        }
        let Ok(cfg) = controller.read_config(1, 18) else {
            return false;
        };
        if cfg.len() < 8 {
            return false;
        }
        let mode = if snap.mode_index == 0 {
            protocol::MODE_CUSTOM
        } else {
            MODES[snap.mode_index - 1].1
        };
        if cfg[0] != mode
            || cfg[1] != snap.brightness
            || cfg[2] != protocol::firmware_speed(snap.speed)
            || cfg[3] != snap.reverse as u8
        {
            return false;
        }
        // En mode personnalisé, les octets RGB du bloc d'effet ne sont pas
        // utilisés : seules les couleurs per-touche comptent.
        if snap.mode_index != 0
            && (cfg[5] != snap.rgb[0] || cfg[6] != snap.rgb[1] || cfg[7] != snap.rgb[2])
        {
            return false;
        }
        let Ok((poll, deb, win, arrow, anti)) = controller.read_system_settings() else {
            return false;
        };
        poll == snap.poll
            && deb == snap.deb
            && win == snap.win_lock
            && arrow == snap.arrow_mode
            && anti == snap.anti_ghosting
    }

    /// Écrit le profil complet (couleurs, mode, réglages) sur le clavier.
    fn apply_profile_to_keyboard(&mut self) -> Result<()> {
        let Some(controller) = &self.controller else {
            return Err(anyhow!("aucun clavier ouvert"));
        };
        let data = colors_to_data(&self.colors);
        if self.profile > 0 {
            controller.write_custom_colors(self.profile, &data)?;
        }
        controller.write_custom_colors(0, &data)?;
        if self.mode_index == 0 {
            controller.set_custom_mode(
                0,
                self.brightness,
                protocol::firmware_speed(self.speed),
                if self.reverse { 1 } else { 0 },
            )?;
        } else {
            let (_, mode) = MODES[self.mode_index - 1];
            controller.set_mode(
                mode,
                self.rgb,
                self.brightness,
                protocol::firmware_speed(self.speed),
                if self.reverse { 1 } else { 0 },
            )?;
        }
        controller.apply_system_settings(
            self.polling_index as u8,
            self.debounce_index as u8,
            self.win_lock,
            self.arrow_mode,
            self.anti_ghosting,
        )?;
        Ok(())
    }

    /// Charge le profil actif depuis le disque et, au besoin, le réapplique au
    /// clavier. `apply_if_needed` évite toute écriture au démarrage quand le
    /// clavier est déjà dans l'état du profil.
    fn restore_profile(&mut self, apply_if_needed: bool) {
        self.stop_live();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.pre_write_backup = None;

        let active = std::fs::read_to_string(self.active_index_file())
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("profile="))
                    .and_then(|value| value.trim().parse::<u8>().ok())
            })
            .unwrap_or(0)
            .min(9);
        let state = read_profile_state(&self.state_file(active));
        let colors = std::fs::read(self.profile_file(active))
            .ok()
            .map(|data| data_to_colors(&data));

        match (state, colors) {
            (Some(state), Some(colors)) => {
                self.profile = active;
                self.layers = read_layers_file(&self.layers_file(active));
                self.selected_layer = None;
                self.apply_snapshot(&state);
                self.colors = colors;
                if apply_if_needed && !self.device_matches(&state) {
                    match self.apply_profile_to_keyboard() {
                        Ok(()) => {
                            self.status =
                                format!("Profil {} restauré sur le clavier.", self.profile + 1);
                            self.status_error = false;
                        }
                        Err(err) => {
                            self.status = format!("Restauration du profil impossible : {err:#}");
                            self.status_error = true;
                        }
                    }
                } else {
                    self.status = format!("Profil {} chargé.", self.profile + 1);
                    self.status_error = false;
                }
                let _ = self.save_profile_files();
            }
            _ => {
                // Premier lancement : on part de l'état réel du clavier.
                self.profile = 0;
                self.layers = read_layers_file(&self.layers_file(0));
                self.selected_layer = None;
                match self.read_device_snapshot() {
                    Some((state, colors)) => {
                        self.apply_snapshot(&state);
                        self.colors = colors;
                        if !self.status_error {
                            self.status = "Profil 1 initialisé depuis le clavier.".to_string();
                        }
                    }
                    None => {
                        if let Some(controller) = &self.controller {
                            match controller.read_custom_colors(0, protocol::CUSTOM_COLORSET_SIZE) {
                                Ok(data) => self.colors = data_to_colors(&data),
                                Err(err) => {
                                    self.status = format!("Lecture du profil impossible : {err:#}");
                                    self.status_error = true;
                                }
                            }
                        }
                    }
                }
                let _ = self.save_profile_files();
            }
        }
        if !self.layers.is_empty() {
            self.start_live();
        }
    }

    fn save_profile_files(&self) -> Result<()> {
        std::fs::create_dir_all(&self.profile_dir)?;
        std::fs::write(
            self.profile_file(self.profile),
            colors_to_data(&self.colors),
        )?;
        write_profile_state(&self.state_file(self.profile), &self.snapshot())?;
        self.save_layers()?;
        std::fs::write(
            self.active_index_file(),
            format!("profile={}\n", self.profile),
        )?;
        Ok(())
    }

    fn apply_system_settings(
        &mut self,
        polling_index: usize,
        debounce_index: usize,
        win_lock: bool,
        arrow_mode: bool,
        anti_ghosting: bool,
    ) {
        if polling_index == self.polling_index
            && debounce_index == self.debounce_index
            && win_lock == self.win_lock
            && arrow_mode == self.arrow_mode
            && anti_ghosting == self.anti_ghosting
        {
            return;
        }
        let Some(&poll) = [0u8, 1, 2, 3].get(polling_index) else {
            self.status = "Fréquence USB invalide.".to_string();
            self.status_error = true;
            return;
        };
        let Some(&deb) = [0u8, 1, 2, 3].get(debounce_index) else {
            self.status = "Debounce invalide.".to_string();
            self.status_error = true;
            return;
        };
        let was_live = self.live.is_some();
        self.stop_live();
        // Sauvegarde du bloc de configuration complet avant modification.
        if let Some(controller) = &self.controller {
            if let (Ok(lo), Ok(hi)) = (
                controller.read_config(0, 64),
                controller.read_config(64, 64),
            ) {
                let mut full = lo;
                full.extend_from_slice(&hi);
                let _ = std::fs::create_dir_all(&self.backup_dir);
                let path = self
                    .backup_dir
                    .join(format!("config_avant_{}.bin", timestamp()));
                if std::fs::write(&path, &full).is_ok() {
                    self.last_backup = Some(path);
                }
            }
        }
        let Some(controller) = &self.controller else {
            self.status = "Aucun clavier ouvert.".to_string();
            self.status_error = true;
            return;
        };
        match controller.apply_system_settings(poll, deb, win_lock, arrow_mode, anti_ghosting) {
            Ok(()) => {
                self.polling_index = polling_index;
                self.debounce_index = debounce_index;
                self.win_lock = win_lock;
                self.arrow_mode = arrow_mode;
                self.anti_ghosting = anti_ghosting;
                let _ = self.save_profile_files();
                self.status = format!(
                    "Réglages clavier : {} Hz, debounce {} ms, Mode Jeu {}, Mode flèche {}, Anti-ghosting {}.",
                    [1000, 500, 250, 125][polling_index],
                    [5, 10, 15, 20][debounce_index],
                    if win_lock { "activé" } else { "désactivé" },
                    if arrow_mode { "activé" } else { "désactivé" },
                    if anti_ghosting {
                        "activé"
                    } else {
                        "désactivé"
                    }
                );
                self.status_error = false;
                if was_live && !self.layers.is_empty() {
                    self.start_live();
                }
            }
            Err(err) => {
                self.status = format!("Réglages clavier refusés : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn switch_profile(&mut self, new: u8) {
        if new == self.profile {
            return;
        }
        self.stop_live();
        self.ensure_pre_write_backup();
        // Sauvegarde complète du profil courant avant de changer.
        if let Err(err) = self.save_profile_files() {
            self.status = format!("Sauvegarde du profil impossible : {err:#}");
            self.status_error = true;
        }

        // Un profil sans fichier local démarre comme une copie du profil
        // courant : l'utilisateur retrouve ses couleurs et ses réglages.
        let state = read_profile_state(&self.state_file(new)).unwrap_or_else(|| self.snapshot());
        let colors = std::fs::read(self.profile_file(new))
            .ok()
            .map(|data| data_to_colors(&data))
            .unwrap_or_else(|| self.colors.clone());
        let new_layers = if self.layers_file(new).exists() {
            read_layers_file(&self.layers_file(new))
        } else {
            self.layers.clone()
        };

        self.profile = new;
        self.layers = new_layers;
        self.selected_layer = None;
        self.apply_snapshot(&state);
        self.colors = colors;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.pre_write_backup = None;

        match self.apply_profile_to_keyboard() {
            Ok(()) => {
                let _ = self.save_profile_files();
                self.status = format!("Profil {} appliqué au clavier.", self.profile + 1);
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Écriture du profil impossible : {err:#}");
                self.status_error = true;
            }
        }
        if !self.layers.is_empty() {
            self.start_live();
        }
    }

    fn save_current_profile_file(&self) -> Result<PathBuf> {
        self.save_profile_files()?;
        Ok(self.profile_file(self.profile))
    }

    fn ensure_pre_write_backup(&mut self) {
        if self.controller.is_none() || self.pre_write_backup.is_some() {
            return;
        }
        let _ = std::fs::create_dir_all(&self.backup_dir);
        let path = self.backup_dir.join(format!(
            "profil_{}_avant_{}.bin",
            self.profile + 1,
            timestamp()
        ));
        match std::fs::write(&path, colors_to_data(&self.colors)) {
            Ok(()) => {
                self.pre_write_backup = Some(path.clone());
                self.last_backup = Some(path);
            }
            Err(err) => {
                self.status = format!("Backup avant écriture impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn save_backup_manual(&mut self) {
        let _ = std::fs::create_dir_all(&self.backup_dir);
        let _ = self.save_profile_files();
        let path = self.backup_dir.join(format!(
            "profil_{}_manuel_{}.bin",
            self.profile + 1,
            timestamp()
        ));
        match std::fs::write(&path, colors_to_data(&self.colors)) {
            Ok(()) => {
                self.last_backup = Some(path.clone());
                self.status = format!(
                    "Profil {} sauvegardé · backup {}",
                    self.profile + 1,
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Backup impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn latest_backup_path(&self) -> Option<PathBuf> {
        let mut best: Option<(u64, PathBuf)> = None;
        let entries = std::fs::read_dir(&self.backup_dir).ok()?;
        let prefix = format!("profil_{}_", self.profile + 1);
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy().to_string();
            if !(name.starts_with(&prefix) && name.ends_with(".bin")) {
                continue;
            }
            let stem = name.trim_start_matches(&prefix).trim_end_matches(".bin");
            let key = stem
                .split('_')
                .next_back()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            if best.as_ref().map(|(k, _)| key > *k).unwrap_or(true) {
                best = Some((key, entry.path()));
            }
        }
        best.map(|(_, path)| path)
    }

    fn restore_last_backup(&mut self) {
        self.stop_live();
        self.push_undo();
        self.ensure_pre_write_backup();
        let Some(path) = self.latest_backup_path() else {
            self.status = "Aucun backup pour ce profil.".to_string();
            self.status_error = true;
            return;
        };
        match std::fs::read(&path) {
            Ok(data) => {
                self.colors = data_to_colors(&data);
                match self.write_colors() {
                    Ok(()) => {
                        let _ = self.save_current_profile_file();
                        self.status = format!(
                            "Restauré : {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        );
                        self.status_error = false;
                    }
                    Err(err) => {
                        self.status = format!("Écriture de restauration impossible : {err:#}");
                        self.status_error = true;
                    }
                }
            }
            Err(err) => {
                self.status = format!("Lecture du backup impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.colors.clone());
        if self.undo_stack.len() > HISTORY_LIMIT {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        let Some(previous) = self.undo_stack.pop() else {
            self.status = "Rien à annuler.".to_string();
            self.status_error = false;
            return;
        };
        self.stop_live();
        self.ensure_pre_write_backup();
        self.redo_stack.push(self.colors.clone());
        self.colors = previous;
        match self.write_colors() {
            Ok(()) => {
                let _ = self.save_current_profile_file();
                self.status = "Modification annulée.".to_string();
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Annulation impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            self.status = "Rien à rétablir.".to_string();
            self.status_error = false;
            return;
        };
        self.stop_live();
        self.ensure_pre_write_backup();
        self.undo_stack.push(self.colors.clone());
        self.colors = next;
        match self.write_colors() {
            Ok(()) => {
                let _ = self.save_current_profile_file();
                self.status = "Modification rétablie.".to_string();
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Rétablissement impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    /// Applique immédiatement un changement de luminosité / vitesse / sens :
    /// en mode personnalisé on réécrit le profil per-touche, sinon on réapplique
    /// l'effet choisi. Le profil local est sauvegardé dans les deux cas.
    fn settings_edited(&mut self) {
        if self.controller.is_none() {
            return;
        }
        if self.mode_index == 0 {
            match self.write_colors() {
                Ok(()) => {
                    self.status = "Réglages du profil appliqués.".to_string();
                    self.status_error = false;
                }
                Err(err) => {
                    self.status = format!("Écriture impossible : {err:#}");
                    self.status_error = true;
                }
            }
        } else {
            self.apply_effect();
        }
    }

    fn write_colors(&mut self) -> Result<()> {
        let data = colors_to_data(&self.colors);
        let Some(controller) = &self.controller else {
            return Err(anyhow!("aucun clavier ouvert"));
        };
        if self.profile > 0 {
            controller.write_custom_colors(self.profile, &data)?;
        }
        controller.write_custom_colors(0, &data)?;
        controller.set_custom_mode(
            0,
            self.brightness,
            protocol::firmware_speed(self.speed),
            if self.reverse { 1 } else { 0 },
        )?;
        self.mode_index = 0;
        let _ = self.save_profile_files();
        Ok(())
    }

    /// Arrête le rendu PC et rend la main au firmware. Les calques sont
    /// conservés : ils peuvent être relancés ou restent enregistrés au profil.
    fn stop_live(&mut self) {
        if self.live.take().is_some() {
            if let Some(controller) = &self.controller {
                let _ = controller.end_direct();
            }
            self.live_colors = None;
            // Le 5041 reste noir après 0x13 : on ré-applique l'état stocké,
            // effet firmware ou couleurs per-touche selon le mode courant.
            if self.mode_index == 0 {
                let _ = self.write_colors();
            } else if let Some(controller) = &self.controller {
                let (_, mode) = MODES[self.mode_index - 1];
                let _ = controller.set_mode(
                    mode,
                    self.rgb,
                    self.brightness,
                    protocol::firmware_speed(self.speed),
                    if self.reverse { 1 } else { 0 },
                );
                let _ = self.save_profile_files();
            }
        }
    }

    fn base_layer_colors(&self, now: f32) -> Vec<[u8; 3]> {
        if self.mode_index == 0 {
            self.colors.clone()
        } else {
            self.effect_preview_palette(now)
        }
    }

    fn compose_live_colors(&self, now: f32) -> Vec<[u8; 3]> {
        let positions = led_positions(&self.keys);
        let base = self.base_layer_colors(now);
        compose_effect_layers(&self.layers, now, &base, &positions)
    }

    fn live_frame_colors(&self) -> Option<Vec<[u8; 3]>> {
        if let Some(colors) = &self.live_colors {
            Some(colors.clone())
        } else {
            self.live
                .as_ref()
                .map(|live| self.compose_live_colors(live.started.elapsed().as_secs_f32()))
        }
    }

    /// Publie une image statique des calques comme base per-touche du clavier.
    /// Elle sert de secours quand les frames directes s'arrêtent (fenêtre
    /// réduite ou application fermée) : le clavier garde l'effet au lieu de
    /// revenir immédiatement à l'éclairage de base.
    fn publish_live_snapshot(&self, colors: &[[u8; 3]]) -> Result<()> {
        let Some(controller) = &self.controller else {
            return Err(anyhow!("aucun clavier ouvert"));
        };
        let data = colors_to_data(colors);
        controller.write_custom_colors(0, &data)?;
        controller.set_custom_mode(
            0,
            self.brightness,
            protocol::firmware_speed(self.speed),
            if self.reverse { 1 } else { 0 },
        )?;
        Ok(())
    }

    /// Fige la dernière image des calques sur le clavier et arrête le rendu
    /// direct. Les couleurs locales et les calques ne sont pas modifiés.
    fn freeze_live_frame(&mut self) -> bool {
        if self.layers.is_empty() {
            return false;
        }
        let Some(colors) = self.live_frame_colors() else {
            return false;
        };
        let Some(controller) = &self.controller else {
            return false;
        };
        let _ = controller.end_direct();
        match self.publish_live_snapshot(&colors) {
            Ok(()) => {
                self.live = None;
                self.live_colors = None;
                self.status = "Dernière image des effets figée sur le clavier.".to_string();
                self.status_error = false;
                true
            }
            Err(err) => {
                self.status = format!("Effets PC arrêtés : {err:#}");
                self.status_error = true;
                self.stop_live();
                false
            }
        }
    }

    fn start_live(&mut self) -> bool {
        if self.controller.is_none() || self.layers.is_empty() {
            return false;
        }
        let first_frame = self.live.is_none();
        if first_frame {
            self.live = Some(LiveRuntime {
                started: Instant::now(),
                last_frame: Instant::now() - Duration::from_millis(60),
            });
        }
        let now = self
            .live
            .as_ref()
            .map(|live| live.started.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        let colors = self.compose_live_colors(now);
        if first_frame {
            if let Err(err) = self.publish_live_snapshot(&colors) {
                self.status = format!("Image de secours des effets non publiée : {err:#}");
                self.status_error = true;
            }
        }
        self.live_colors = Some(colors);
        true
    }

    fn add_effect_layer(&mut self, kind: EffectKind) {
        if self.layers.len() >= MAX_LAYERS {
            self.status = format!("Maximum de {MAX_LAYERS} calques atteint.");
            self.status_error = true;
            return;
        }
        let mask = selection_mask(&self.keys, &self.selected);
        let layer = EffectLayer::new(kind, self.rgb, self.speed, mask);
        let zone = layer.zone_label();
        self.layers.push(layer);
        self.selected_layer = Some(self.layers.len() - 1);
        let _ = self.save_profile_files();
        if self.controller.is_some() {
            self.start_live();
        }
        self.status = format!("Calque « {} » ajouté · {zone}.", kind.label());
        self.status_error = false;
    }

    fn remove_effect_layer(&mut self, index: usize) {
        if index >= self.layers.len() {
            return;
        }
        let removed = self.layers.remove(index);
        if self.selected_layer == Some(index) {
            self.selected_layer = self.layers.len().checked_sub(1);
        } else if let Some(selected) = self.selected_layer {
            if selected > index {
                self.selected_layer = Some(selected - 1);
            }
        }
        if self.layers.is_empty() {
            self.stop_live();
        } else if let Some(live) = self.live.as_mut() {
            live.last_frame = Instant::now() - Duration::from_millis(60);
        }
        let _ = self.save_profile_files();
        self.status = format!("Calque « {} » supprimé.", removed.kind.label());
        self.status_error = false;
    }

    fn move_effect_layer(&mut self, index: usize, delta: isize) {
        if index >= self.layers.len() {
            return;
        }
        let target = index as isize + delta;
        if target < 0 || target >= self.layers.len() as isize {
            return;
        }
        let target = target as usize;
        self.layers.swap(index, target);
        self.selected_layer = Some(target);
        if let Some(live) = self.live.as_mut() {
            live.last_frame = Instant::now() - Duration::from_millis(60);
        }
        let _ = self.save_profile_files();
    }

    fn assign_layer_zone(&mut self, index: usize) {
        if index >= self.layers.len() {
            return;
        }
        let mask = selection_mask(&self.keys, &self.selected);
        let count = mask.iter().filter(|active| **active).count();
        self.layers[index].mask = mask;
        if let Some(live) = self.live.as_mut() {
            live.last_frame = Instant::now() - Duration::from_millis(60);
        }
        let _ = self.save_profile_files();
        if count >= protocol::LED_COUNT {
            self.status = "Zone du calque : tout le clavier.".to_string();
        } else {
            self.status = format!("Zone du calque : {count} touche(s).");
        }
        self.status_error = false;
    }

    fn layer_changed(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.last_frame = Instant::now() - Duration::from_millis(60);
        }
        let _ = self.save_profile_files();
    }

    /// Quand la fenêtre est réduite ou masquée, les passes egui s'arrêtent.
    /// On publie l'image courante comme base per-touche pour que le clavier
    /// garde l'effet au lieu de revenir à l'éclairage de base.
    fn handle_window_visibility(&mut self, ctx: &egui::Context) {
        let (minimized, occluded) = ctx.input(|input| {
            let viewport = input.viewport();
            (
                viewport.minimized.unwrap_or(false),
                viewport.occluded.unwrap_or(false),
            )
        });
        let hidden = minimized || occluded;
        if hidden == self.window_hidden {
            return;
        }
        self.window_hidden = hidden;
        if hidden {
            if self.live.is_none() {
                return;
            }
            let Some(colors) = self.live_frame_colors() else {
                return;
            };
            let Some(controller) = &self.controller else {
                return;
            };
            let _ = controller.end_direct();
            if let Err(err) = self.publish_live_snapshot(&colors) {
                self.status = format!("Image de secours des effets non publiée : {err:#}");
                self.status_error = true;
            }
            if let Some(live) = self.live.as_mut() {
                live.last_frame = Instant::now() - Duration::from_millis(60);
            }
        } else if self.live.is_some() {
            if let Some(live) = self.live.as_mut() {
                live.last_frame = Instant::now() - Duration::from_millis(60);
            }
            ctx.request_repaint();
        }
    }

    fn tick_live(&mut self, ctx: &egui::Context) {
        if self.window_hidden {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        if self.layers.is_empty() {
            self.stop_live();
            return;
        }
        if live.last_frame.elapsed() < Duration::from_millis(40) {
            ctx.request_repaint_after(Duration::from_millis(5));
            return;
        }
        let now = live.started.elapsed().as_secs_f32();
        let colors = self.compose_live_colors(now);
        let data = colors_to_data(&colors);
        let result = self
            .controller
            .as_ref()
            .map(|controller| controller.send_direct_frame(&data));
        match result {
            Some(Ok(())) => {
                self.live_colors = Some(colors);
                self.live.as_mut().unwrap().last_frame = Instant::now();
                ctx.request_repaint_after(Duration::from_millis(40));
            }
            Some(Err(err)) => {
                let _ = self.controller.as_ref().map(|c| c.end_direct());
                self.live = None;
                self.live_colors = None;
                self.status = format!("Effets PC arrêtés : {err:#}");
                self.status_error = true;
            }
            None => {
                self.live = None;
                self.live_colors = None;
            }
        }
    }

    fn paint_selection(&mut self) {
        if self.selected.is_empty() {
            self.status = "Sélectionnez au moins une touche.".to_string();
            self.status_error = true;
            return;
        }
        for &index in &self.selected {
            if let Some(key) = self.keys.get(index) {
                if key.pos < protocol::LED_COUNT {
                    self.colors[key.pos] = self.rgb;
                }
            }
        }
        let count = self.selected.len();
        match self.write_colors() {
            Ok(()) => {
                let _ = self.save_current_profile_file();
                self.status = format!(
                    "{count} touche(s) peinte(s) · #{:02X}{:02X}{:02X}",
                    self.rgb[0], self.rgb[1], self.rgb[2]
                );
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Écriture impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn paint_all(&mut self, color: [u8; 3]) {
        self.stop_live();
        self.push_undo();
        self.ensure_pre_write_backup();
        self.colors = vec![color; protocol::LED_COUNT];
        match self.write_colors() {
            Ok(()) => {
                let _ = self.save_current_profile_file();
                self.status = format!(
                    "Clavier entier peint · #{:02X}{:02X}{:02X}",
                    color[0], color[1], color[2]
                );
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Écriture impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn paint_off_selection(&mut self) {
        if self.selected.is_empty() {
            self.status = "Sélectionnez au moins une touche.".to_string();
            self.status_error = true;
            return;
        }
        self.stop_live();
        self.push_undo();
        self.ensure_pre_write_backup();
        for &index in &self.selected {
            if let Some(key) = self.keys.get(index) {
                if key.pos < protocol::LED_COUNT {
                    self.colors[key.pos] = [0, 0, 0];
                }
            }
        }
        match self.write_colors() {
            Ok(()) => {
                let _ = self.save_current_profile_file();
                self.status = "Sélection éteinte.".to_string();
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Écriture impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn apply_effect(&mut self) {
        let was_live = self.live.is_some();
        self.stop_live();
        let Some(controller) = &self.controller else {
            self.status = "Aucun clavier ouvert.".to_string();
            self.status_error = true;
            return;
        };
        let mode = MODES[self.mode_index - 1].1;
        match controller.set_mode(
            mode,
            self.rgb,
            self.brightness,
            protocol::firmware_speed(self.speed),
            if self.reverse { 1 } else { 0 },
        ) {
            Ok(()) => {
                let _ = self.save_profile_files();
                self.status = format!(
                    "Effet « {} » appliqué · #{:02X}{:02X}{:02X}",
                    MODES[self.mode_index - 1].0,
                    self.rgb[0],
                    self.rgb[1],
                    self.rgb[2]
                );
                self.status_error = false;
                if was_live && !self.layers.is_empty() {
                    self.start_live();
                }
            }
            Err(err) => {
                self.status = format!("Écriture impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn reconnect(&mut self) {
        self.stop_live();
        match Controller::open() {
            Ok(controller) => {
                self.controller = Some(controller);
                self.restore_profile(true);
                self.status = "Clavier reconnecté.".to_string();
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Reconnexion impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        {
            let visuals = ui.visuals_mut();
            visuals.override_text_color = Some(TEXT);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(58, 58, 58);
            visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(58, 58, 58);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(72, 72, 72);
            visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(72, 72, 72);
            visuals.widgets.active.bg_fill = Color32::from_rgb(84, 84, 84);
            visuals.widgets.active.weak_bg_fill = Color32::from_rgb(84, 84, 84);
            visuals.widgets.inactive.fg_stroke.color = Color32::WHITE;
            visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
            visuals.widgets.active.fg_stroke.color = Color32::WHITE;
            visuals.selection.bg_fill = ACCENT;
            visuals.selection.stroke.color = Color32::BLACK;
            visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
            visuals.widgets.hovered.corner_radius = CornerRadius::same(4);
            visuals.widgets.active.corner_radius = CornerRadius::same(4);
        }

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, CornerRadius::same(8), Color32::WHITE);
            painter.text(
                rect.center() - Vec2::new(0.0, 1.0),
                Align2::CENTER_CENTER,
                "B",
                FontId::new(22.0, FontFamily::Proportional),
                Color32::BLACK,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    rect.left_bottom() + Vec2::new(7.0, -5.0),
                    Vec2::new(22.0, 3.0),
                ),
                CornerRadius::same(1),
                ACCENT,
            );
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("BERSERKER")
                        .size(17.0)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.label(
                    RichText::new("RGB CONTROL")
                        .size(9.5)
                        .strong()
                        .color(ACCENT),
                );
            });
        });

        ui.add_space(22.0);
        ui.label(RichText::new("PROFIL").size(9.5).strong().color(MUTED));
        ui.add_space(2.0);

        let mut new_profile = self.profile;
        egui::ComboBox::from_id_salt("profile_combo")
            .width(168.0)
            .selected_text(format!("Profil {}", self.profile + 1))
            .show_ui(ui, |ui| {
                for i in 0..10u8 {
                    ui.selectable_value(&mut new_profile, i, format!("Profil {}", i + 1));
                }
            });
        if new_profile != self.profile {
            self.switch_profile(new_profile);
        }

        ui.add_space(18.0);
        if menu_item(ui, "Éclairage", self.page == Page::Lighting, true) {
            self.page = Page::Lighting;
        }
        if menu_item(ui, "Effets", self.page == Page::Effects, true) {
            self.page = Page::Effects;
        }
        menu_item(ui, "Touches", false, false);
        if menu_item(ui, "Macros", self.page == Page::Macros, true) {
            self.page = Page::Macros;
        }
        if menu_item(ui, "Autres réglages", self.page == Page::Settings, true) {
            self.page = Page::Settings;
        }

        let remaining = ui.available_height();
        if remaining > 110.0 {
            ui.add_space(remaining - 110.0);
        }
        egui::Frame::new()
            .fill(Color32::from_rgb(56, 56, 56))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("Profil {}", self.profile + 1))
                        .size(12.0)
                        .color(Color32::WHITE),
                );
                ui.label(
                    RichText::new("Jeu de couleurs EVision")
                        .size(10.0)
                        .color(MUTED),
                );
            });
        ui.add_space(8.0);
        ui.label(
            RichText::new("EVision V2 · 126 zones")
                .size(9.5)
                .color(Color32::from_rgb(110, 110, 110)),
        );
    }

    fn effect_brightness(&self) -> f32 {
        if self.brightness == 0 {
            0.0
        } else if self.brightness <= 4 {
            self.brightness as f32 / 4.0
        } else {
            self.brightness as f32 / 255.0
        }
    }

    /// Aperçu animé approximatif des effets firmware. Les motifs sont
    /// recalculés à chaque frame (40 ms) pour montrer la vitesse choisie.
    fn effect_preview_palette(&self, time: f32) -> Vec<[u8; 3]> {
        let base = self.effect_preview_color();
        let bright = self.effect_brightness();
        let mode = MODES
            .get(self.mode_index.saturating_sub(1))
            .map(|(_, value)| *value)
            .unwrap_or(protocol::MODE_STATIC);
        // Les effets sans réglage de vitesse (Statique, Balayage, Radiation)
        // sont prévisualisés à cadence fixe, comme sur le clavier.
        let speed = if protocol::mode_supports_speed(mode) {
            0.25 + self.speed.min(protocol::SPEED_UI_MAX) as f32 * 0.4
        } else {
            1.6
        };
        let t = time * speed;
        let scale = |factor: f32| {
            [
                (base[0] as f32 * factor).clamp(0.0, 255.0) as u8,
                (base[1] as f32 * factor).clamp(0.0, 255.0) as u8,
                (base[2] as f32 * factor).clamp(0.0, 255.0) as u8,
            ]
        };
        let mut colors = vec![[0u8; 3]; protocol::LED_COUNT];
        for key in &self.keys {
            let x = ((key.x - layout::BODY_X) / layout::TOTAL_W).clamp(0.0, 1.0);
            let y = ((key.y - layout::BODY_Y) / layout::TOTAL_H).clamp(0.0, 1.0);
            let color = match mode {
                0x06 => base,
                0x05 => {
                    let pulse = 0.2 + 0.8 * (0.5 + 0.5 * (t * 2.2).sin());
                    scale(pulse)
                }
                0x03 => {
                    let dx = x - 0.5;
                    let dy = y - 0.5;
                    let angle = dy.atan2(dx) / std::f32::consts::TAU + 0.5;
                    let radius = (dx * dx + dy * dy).sqrt();
                    let swirl = 0.5
                        + 0.5
                            * ((angle * 2.0 + radius * 14.0 - t * 2.5) * std::f32::consts::TAU)
                                .sin();
                    scale(0.06 + 0.94 * swirl)
                }
                0x04 => {
                    let dx = x - 0.5;
                    let dy = y - 0.5;
                    let angle = dy.atan2(dx) / std::f32::consts::TAU + 0.5;
                    let radius = (dx * dx + dy * dy).sqrt();
                    let wave =
                        0.55 + 0.45 * ((radius * 18.0 - t * 3.0) * std::f32::consts::TAU).sin();
                    hsv_to_rgb(angle + t * 0.06, 1.0, bright * wave)
                }
                0x08 => {
                    let wave = 0.5 + 0.5 * ((x * 6.0 - t * 2.0) * std::f32::consts::TAU).sin();
                    scale(0.08 + 0.92 * wave)
                }
                0x0A => {
                    let cell = (x * 14.0).floor() + (y * 9.0).floor() * 17.0;
                    let spark = preview_hash(cell as usize, (t * 2.0).floor());
                    if spark > 0.78 {
                        let twinkle = 0.35 + 0.65 * preview_hash(key.pos, t * 6.0);
                        scale(twinkle)
                    } else {
                        scale(0.03)
                    }
                }
                0x0B => {
                    let cell = (x * 10.0).floor() as usize + (y * 7.0).floor() as usize * 11;
                    let pick = preview_hash(cell, (t * 0.8).floor());
                    let hue = preview_hash(cell.wrapping_mul(3), 1.0) + t * 0.02;
                    let level = 0.25 + 0.75 * pick;
                    let rgb = hsv_to_rgb(hue, 0.85, bright * level);
                    [
                        ((rgb[0] as f32 + base[0] as f32) * 0.5) as u8,
                        ((rgb[1] as f32 + base[1] as f32) * 0.5) as u8,
                        ((rgb[2] as f32 + base[2] as f32) * 0.5) as u8,
                    ]
                }
                0x0C => {
                    let band = 0.5 + 0.5 * ((y * 4.0 + t * 2.0) * std::f32::consts::TAU).sin();
                    scale(band.powf(2.0))
                }
                0x0D => {
                    let wave =
                        0.5 + 0.5 * ((x * 3.0 + y * 2.0 - t * 2.4) * std::f32::consts::TAU).sin();
                    scale(wave.powf(2.0))
                }
                0x0F => {
                    let column = (x * 16.0).floor() as usize;
                    let phase = preview_hash(column, 1.0);
                    let falling = ((t * 0.35 + phase) % 1.0);
                    let distance = (y - falling).abs();
                    if distance < 0.12 {
                        scale(1.0 - distance / 0.12)
                    } else {
                        scale(0.02)
                    }
                }
                0x10 => {
                    let band =
                        (0.5 + 0.5 * ((y * 3.0 - t * 1.8) * std::f32::consts::TAU).sin()).powf(4.0);
                    scale(band)
                }
                0x12 => {
                    let dx = x - 0.5;
                    let dy = y - 0.5;
                    let radius = (dx * dx + dy * dy).sqrt();
                    let rings =
                        0.5 + 0.5 * ((radius * 22.0 - t * 3.0) * std::f32::consts::TAU).sin();
                    scale(0.04 + 0.96 * rings)
                }
                _ => base,
            };
            if key.pos < protocol::LED_COUNT {
                colors[key.pos] = color;
            }
        }
        colors
    }

    fn effect_preview_color(&self) -> [u8; 3] {
        let factor = if self.brightness == 0 {
            0.0
        } else if self.brightness <= 4 {
            self.brightness as f32 / 4.0
        } else {
            self.brightness as f32 / 255.0
        };
        [
            (self.rgb[0] as f32 * factor).round().clamp(0.0, 255.0) as u8,
            (self.rgb[1] as f32 * factor).round().clamp(0.0, 255.0) as u8,
            (self.rgb[2] as f32 * factor).round().clamp(0.0, 255.0) as u8,
        ]
    }

    fn refresh_lighting_from_device(&mut self) {
        let Some((state, colors)) = self.read_device_snapshot() else {
            self.status = "Lecture de l'état du clavier impossible.".to_string();
            self.status_error = true;
            return;
        };
        self.apply_snapshot(&state);
        self.colors = colors;
        let _ = self.save_profile_files();
        self.status = "État réel du clavier relu.".to_string();
        self.status_error = false;
    }

    fn keyboard(&mut self, ui: &mut egui::Ui, colors_available: bool) {
        let avail = ui.available_width();
        let scale = ((avail - 8.0) / layout::TOTAL_W).clamp(0.95, 1.55);
        let kb_w = layout::TOTAL_W * scale;
        let kb_h = layout::TOTAL_H * scale;
        let (alloc, response) =
            ui.allocate_exact_size(Vec2::new(avail, kb_h + 10.0), Sense::click_and_drag());
        let body = Rect::from_center_size(alloc.center(), Vec2::new(kb_w, kb_h));
        let painter = ui.painter_at(alloc);
        painter.rect_filled(
            body.translate(Vec2::new(0.0, 3.0)),
            CornerRadius::same(10),
            Color32::from_black_alpha(28),
        );
        painter.rect_filled(body, CornerRadius::same(10), Color32::from_rgb(22, 22, 22));
        painter.rect_stroke(
            body,
            CornerRadius::same(10),
            Stroke::new(1.0, Color32::from_rgb(45, 45, 45)),
            StrokeKind::Inside,
        );

        let origin = body.min;
        // En mode effet, les couleurs per-touche stockées ne sont pas ce que le
        // clavier affiche : on montre la couleur de base de l'effet en cours.
        let effect_palette = if self.live_colors.is_none() && self.mode_index != 0 {
            let now = ui.ctx().input(|input| input.time) as f32;
            ui.ctx().request_repaint_after(Duration::from_millis(40));
            Some(self.effect_preview_palette(now))
        } else {
            None
        };
        let palette = self
            .live_colors
            .as_deref()
            .or(effect_palette.as_deref())
            .unwrap_or(self.colors.as_slice());
        let mut hitboxes: Vec<(usize, Rect)> = Vec::with_capacity(self.keys.len());
        let pointer = response.hover_pos();

        // Bandeau C / S / W du bloc navigation, comme sur le clavier.
        let strip = Rect::from_min_max(
            origin
                + Vec2::new(
                    (544.0 - layout::BODY_X) * scale,
                    (44.0 - layout::BODY_Y) * scale,
                ),
            origin
                + Vec2::new(
                    (632.5 - layout::BODY_X) * scale,
                    (57.0 - layout::BODY_Y) * scale,
                ),
        );
        painter.rect_filled(strip, CornerRadius::same(3), Color32::from_rgb(32, 32, 32));
        let strip_font = (scale * 6.2).clamp(6.5, 9.0);
        for (label, x) in [("C", 558.75), ("S", 588.25), ("W", 617.75)] {
            painter.text(
                pos2(origin.x + (x - layout::BODY_X) * scale, strip.center().y),
                Align2::CENTER_CENTER,
                label,
                FontId::new(strip_font, FontFamily::Proportional),
                Color32::from_rgb(210, 210, 210),
            );
        }

        for (index, key) in self.keys.iter().enumerate() {
            // Les keycaps sont légèrement plus larges que la cellule pour
            // retrouver les faibles jeux de la photo.
            let cell = Rect::from_min_size(
                origin
                    + Vec2::new(
                        (key.x - layout::BODY_X) * scale,
                        (key.y - layout::BODY_Y) * scale,
                    ),
                Vec2::new((key.w * scale).max(4.0), (key.h * scale).max(4.0)),
            );
            let (rect, ext) = match iso_enter_parts(key, origin, scale) {
                Some((main, ext)) => (main, Some(ext)),
                None => (cell, None),
            };
            let corner = (5.0 * scale).clamp(2.0, 8.0);
            let radius = CornerRadius::same(corner.round() as u8);
            hitboxes.push((index, rect));
            if let Some(ext) = ext {
                hitboxes.push((index, ext));
            }

            let color = palette.get(key.pos).copied().unwrap_or([0, 0, 0]);
            let lit = color != [0, 0, 0];
            let selected = self.selected.contains(&index);
            let fill = if selected {
                Color32::from_rgb(48, 43, 36)
            } else {
                Color32::from_rgb(34, 34, 34)
            };
            if let Some(ext) = ext {
                let ext_radius = CornerRadius::same(
                    (corner.min((rect.left() - ext.left()) * 0.45)).max(1.0) as u8,
                );
                let ext_fill =
                    Rect::from_min_max(ext.min, pos2(rect.left() + corner, ext.bottom()));
                painter.rect_filled(rect, radius, fill);
                painter.rect_filled(ext_fill, ext_radius, fill);
            } else {
                painter.rect_filled(rect, radius, fill);
            }

            let border = if lit {
                Color32::from_rgb(color[0], color[1], color[2])
            } else {
                Color32::from_rgb(58, 58, 58)
            };
            if let Some(ext) = ext {
                paint_iso_enter_outline(
                    &painter,
                    rect,
                    ext.left(),
                    ext.bottom(),
                    corner,
                    1.0,
                    border,
                );
            } else {
                painter.rect_stroke(rect, radius, Stroke::new(1.0, border), StrokeKind::Inside);
            }

            let text_color = if lit {
                Color32::from_rgb(
                    (color[0] as f32 * 0.55 + 110.0).min(255.0) as u8,
                    (color[1] as f32 * 0.55 + 110.0).min(255.0) as u8,
                    (color[2] as f32 * 0.55 + 110.0).min(255.0) as u8,
                )
            } else {
                Color32::from_rgb(160, 160, 160)
            };
            let font_size = (scale * 8.8).clamp(8.0, 13.0);

            let icon = if key.label.is_empty() {
                match key.pos {
                    34 => Some(KeyIcon::Backspace),
                    76 => Some(KeyIcon::Enter),
                    42 => Some(KeyIcon::Tab),
                    63 => Some(KeyIcon::Caps),
                    84 | 97 => Some(KeyIcon::Shift),
                    99 => Some(KeyIcon::ArrowUp),
                    119 => Some(KeyIcon::ArrowLeft),
                    120 => Some(KeyIcon::ArrowDown),
                    121 => Some(KeyIcon::ArrowRight),
                    111 => Some(KeyIcon::Space),
                    106 | 116 => Some(KeyIcon::Win),
                    _ => None,
                }
            } else {
                None
            };

            if let Some(icon) = icon {
                draw_key_icon(&painter, rect, icon, text_color, scale);
            } else {
                let lines: Vec<&str> = key.label.split('\n').collect();
                if lines.len() > 1 {
                    let f = FontId::new(font_size * 0.78, FontFamily::Proportional);
                    let step = font_size * 0.9;
                    for (i, line) in lines.iter().enumerate() {
                        let y =
                            rect.center().y + (i as f32 - (lines.len() - 1) as f32 / 2.0) * step;
                        painter.text(
                            pos2(rect.center().x, y),
                            Align2::CENTER_CENTER,
                            *line,
                            f.clone(),
                            text_color,
                        );
                    }
                } else {
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        key.label,
                        FontId::new(
                            if key.label.chars().count() > 4 {
                                font_size * 0.82
                            } else {
                                font_size
                            },
                            FontFamily::Proportional,
                        ),
                        text_color,
                    );
                }
            }

            if selected {
                if let Some(ext) = ext {
                    paint_iso_enter_outline(
                        &painter,
                        rect,
                        ext.left(),
                        ext.bottom(),
                        corner,
                        2.0,
                        ACCENT,
                    );
                } else {
                    painter.rect_stroke(rect, radius, Stroke::new(2.0, ACCENT), StrokeKind::Inside);
                }
            } else if let Some(p) = pointer {
                if rect.contains(p) || ext.is_some_and(|ext| ext.contains(p)) {
                    if let Some(ext) = ext {
                        paint_iso_enter_outline(
                            &painter,
                            rect,
                            ext.left(),
                            ext.bottom(),
                            corner,
                            1.0,
                            Color32::from_white_alpha(170),
                        );
                    } else {
                        painter.rect_stroke(
                            rect,
                            radius,
                            Stroke::new(1.0, Color32::from_white_alpha(170)),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
        }

        if let Some(p) = pointer {
            if hitboxes.iter().any(|(_, r)| r.contains(p)) {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
        }

        let ctrl = ui.input(|i| i.modifiers.ctrl);

        if response.drag_started() {
            if let Some(p) = response.interact_pointer_pos() {
                self.drag_start = Some(p);
                self.drag_current = Some(p);
            }
        }
        if response.dragged() {
            if let Some(p) = response.interact_pointer_pos() {
                self.drag_current = Some(p);
            }
        }
        if response.drag_stopped() {
            if let (Some(start), Some(current)) = (self.drag_start, self.drag_current) {
                let selection = Rect::from_two_pos(start, current);
                if selection.width() > 3.0 || selection.height() > 3.0 {
                    apply_rect_selection(&mut self.selected, &hitboxes, selection, ctrl);
                }
            }
            self.drag_start = None;
            self.drag_current = None;
        }

        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let index = hitboxes
                    .iter()
                    .find(|(_, r)| r.contains(p))
                    .map(|(index, _)| *index);
                apply_click(&mut self.selected, index, ctrl);
            }
        }

        if let (Some(start), Some(current)) = (self.drag_start, self.drag_current) {
            let selection = Rect::from_two_pos(start, current);
            painter.rect_filled(
                selection,
                CornerRadius::same(2),
                Color32::from_rgba_unmultiplied(255, 158, 44, 30),
            );
            painter.rect_stroke(
                selection,
                CornerRadius::same(2),
                Stroke::new(1.5, ACCENT),
                StrokeKind::Inside,
            );
        }

        if !colors_available {
            painter.text(
                alloc.center(),
                Align2::CENTER_CENTER,
                "Clavier hors ligne",
                FontId::new(16.0, FontFamily::Proportional),
                Color32::from_white_alpha(180),
            );
        }
    }

    fn lighting_panel(&mut self, ui: &mut egui::Ui) {
        {
            let visuals = ui.visuals_mut();
            visuals.override_text_color = Some(TEXT);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(58, 58, 58);
            visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(58, 58, 58);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(74, 74, 74);
            visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(74, 74, 74);
            visuals.widgets.active.bg_fill = Color32::from_rgb(88, 88, 88);
            visuals.widgets.active.weak_bg_fill = Color32::from_rgb(88, 88, 88);
            visuals.widgets.inactive.fg_stroke.color = Color32::WHITE;
            visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
            visuals.widgets.active.fg_stroke.color = Color32::WHITE;
            visuals.selection.bg_fill = ACCENT;
            visuals.selection.stroke.color = Color32::BLACK;
            visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
            visuals.widgets.hovered.corner_radius = CornerRadius::same(4);
            visuals.widgets.active.corner_radius = CornerRadius::same(4);
        }

        ui.label(
            RichText::new("Réglages éclairage")
                .size(13.0)
                .strong()
                .color(Color32::WHITE),
        );
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Mode").size(10.0).color(MUTED));
                let mut mode_index = self.mode_index;
                egui::ComboBox::from_id_salt("mode_combo")
                    .width(210.0)
                    .selected_text(if mode_index == 0 {
                        "Personnalisé (par touche)"
                    } else {
                        MODES[mode_index - 1].0
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mode_index, 0, "Personnalisé (par touche)");
                        for (i, (name, _)) in MODES.iter().enumerate() {
                            ui.selectable_value(&mut mode_index, i + 1, *name);
                        }
                    });
                if mode_index != self.mode_index {
                    self.mode_index = mode_index;
                    if mode_index == 0 {
                        // Retour au mode per-touche : on réaffiche les couleurs.
                        let _ = self.write_colors();
                    } else {
                        let _ = self.save_profile_files();
                    }
                } else {
                    self.mode_index = mode_index;
                }
            });
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Luminosité").size(10.0).color(MUTED));
                let response = ui.add_sized(
                    [170.0, 18.0],
                    egui::Slider::new(&mut self.brightness, 0..=255).show_value(true),
                );
                if response.drag_stopped() || (response.changed() && !response.dragged()) {
                    self.settings_edited();
                }
            });
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Vitesse").size(10.0).color(MUTED));
                let speed_editable = self.mode_index != 0
                    && protocol::mode_supports_speed(mode_value_from_index(self.mode_index));
                ui.add_enabled_ui(speed_editable, |ui| {
                    let response = ui.add_sized(
                        [170.0, 18.0],
                        egui::Slider::new(&mut self.speed, 0..=protocol::SPEED_UI_MAX)
                            .show_value(true),
                    );
                    if response.drag_stopped() || (response.changed() && !response.dragged()) {
                        self.settings_edited();
                    }
                });
                if speed_editable {
                    ui.label(
                        RichText::new("lent · rapide")
                            .size(9.0)
                            .color(Color32::from_rgb(110, 110, 110)),
                    );
                } else {
                    ui.label(
                        RichText::new(if self.mode_index == 0 {
                            "fixe en mode par touche"
                        } else {
                            "non réglable pour cet effet"
                        })
                        .size(9.0)
                        .color(Color32::from_rgb(110, 110, 110)),
                    );
                }
            });
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Direction").size(10.0).color(MUTED));
                ui.horizontal(|ui| {
                    if ui.selectable_label(!self.reverse, "Gauche").clicked() {
                        self.reverse = false;
                        self.settings_edited();
                    }
                    if ui.selectable_label(self.reverse, "Droite").clicked() {
                        self.reverse = true;
                        self.settings_edited();
                    }
                });
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(268.0);
                ui.label(RichText::new("Couleur").size(10.0).color(MUTED));
                egui::Grid::new("palette_grid")
                    .spacing(Vec2::new(4.0, 4.0))
                    .show(ui, |ui| {
                        for (i, color) in PALETTE.iter().enumerate() {
                            let selected = self.rgb == *color;
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(30.0, 22.0), Sense::click());
                            paint_color_swatch(ui.painter(), rect, 3, *color, selected);
                            if resp.clicked() {
                                self.rgb = *color;
                            }
                            if (i + 1) % 8 == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });

            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.set_width(196.0);
                ui.label(RichText::new("RGB").size(10.0).color(MUTED));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("R").size(11.0).color(MUTED));
                    ui.add_sized(
                        [48.0, 22.0],
                        egui::DragValue::new(&mut self.rgb[0]).range(0..=255),
                    );
                    ui.label(RichText::new("G").size(11.0).color(MUTED));
                    ui.add_sized(
                        [48.0, 22.0],
                        egui::DragValue::new(&mut self.rgb[1]).range(0..=255),
                    );
                    ui.label(RichText::new("B").size(11.0).color(MUTED));
                    ui.add_sized(
                        [48.0, 22.0],
                        egui::DragValue::new(&mut self.rgb[2]).range(0..=255),
                    );
                });
                ui.label(
                    RichText::new(format!(
                        "#{:02X}{:02X}{:02X}",
                        self.rgb[0], self.rgb[1], self.rgb[2]
                    ))
                    .size(11.0)
                    .color(MUTED),
                );
                ui.add_space(4.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 38.0), Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(4),
                    Color32::from_rgb(self.rgb[0], self.rgb[1], self.rgb[2]),
                );
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::same(4),
                    Stroke::new(1.0, Color32::from_gray(110)),
                    StrokeKind::Inside,
                );
                ui.label(RichText::new("Aperçu").size(10.0).color(MUTED));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Roue").size(10.0).color(MUTED));
                    let custom_selected = !PALETTE.contains(&self.rgb);
                    let _ = color_well(ui, &mut self.rgb, custom_selected);
                });
            });

            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.set_width(238.0);
                ui.label(RichText::new("Actions").size(10.0).color(MUTED));
                if self.mode_index == 0 {
                    let label = format!("Peindre la sélection ({})", self.selected.len());
                    if ui
                        .add_sized(
                            [238.0, 34.0],
                            egui::Button::new(RichText::new(label).strong().color(Color32::BLACK))
                                .fill(ACCENT)
                                .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        self.paint_selection();
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Éteindre la sélection").clicked() {
                            self.paint_off_selection();
                        }
                        if ui.button("Tout peindre").clicked() {
                            self.paint_all(self.rgb);
                        }
                        if ui.button("Tout éteindre").clicked() {
                            self.paint_all([0, 0, 0]);
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("Annuler").clicked() {
                            self.undo();
                        }
                        if ui.button("Rétablir").clicked() {
                            self.redo();
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Sauvegarder").clicked() {
                            self.save_backup_manual();
                        }
                        if ui.button("Restaurer").clicked() {
                            self.restore_last_backup();
                        }
                    });
                } else {
                    if ui
                        .add_sized(
                            [238.0, 34.0],
                            egui::Button::new(
                                RichText::new("Appliquer l'effet")
                                    .strong()
                                    .color(Color32::BLACK),
                            )
                            .fill(ACCENT)
                            .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        self.apply_effect();
                    }
                    ui.label(
                        RichText::new("Les touches ne sont pas éditables en mode effet.")
                            .size(10.0)
                            .color(MUTED),
                    );
                }
            });
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "Effets superposables · {} calque(s)",
                    self.layers.len()
                ))
                .size(10.0)
                .color(MUTED),
            );
            if self.live.is_some() {
                ui.label(RichText::new("actifs").size(10.0).color(OK));
            } else if !self.layers.is_empty() {
                ui.label(RichText::new("en pause").size(10.0).color(MUTED));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Ouvrir les effets").clicked() {
                    self.page = Page::Effects;
                }
            });
        });

        ui.add_space(6.0);
        ui.label(
            RichText::new(&self.status)
                .size(11.0)
                .color(if self.status_error { ERR } else { OK }),
        );
    }

    fn refresh_macros(&mut self) {
        let mut entries = scan_macros(&self.macros_dir, "Berserker RGB");
        if !self.factory_macros_dir.as_os_str().is_empty() {
            entries.extend(scan_macros(&self.factory_macros_dir, "Application d'usine"));
        }
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        self.macros = entries;
        if self
            .macro_selected
            .map(|i| i >= self.macros.len())
            .unwrap_or(false)
        {
            self.macro_selected = None;
        }
    }

    fn import_selected_macro(&mut self) {
        let Some(index) = self.macro_selected else {
            self.status = "Sélectionnez une macro à importer.".to_string();
            self.status_error = true;
            return;
        };
        let Some(entry) = self.macros.get(index).cloned() else {
            return;
        };
        if entry.source == "Berserker RGB" {
            self.status = "Cette macro est déjà dans votre dossier.".to_string();
            self.status_error = false;
            return;
        }
        let Some(file_name) = entry.path.file_name() else {
            self.status = "Nom de fichier invalide.".to_string();
            self.status_error = true;
            return;
        };
        let dest = self.macros_dir.join(file_name);
        match std::fs::copy(&entry.path, &dest) {
            Ok(_) => {
                self.status = format!(
                    "Macro « {} » importée dans {}.",
                    entry.name,
                    self.macros_dir.display()
                );
                self.status_error = false;
                self.refresh_macros();
            }
            Err(err) => {
                self.status = format!("Import de la macro impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn delete_selected_macro(&mut self) {
        let Some(index) = self.macro_selected else {
            self.status = "Sélectionnez une macro à supprimer.".to_string();
            self.status_error = true;
            return;
        };
        let Some(entry) = self.macros.get(index).cloned() else {
            return;
        };
        if entry.source != "Berserker RGB" {
            self.status =
                "Les macros du logiciel d'usine ne sont pas modifiées. Importez-les d'abord."
                    .to_string();
            self.status_error = true;
            return;
        }
        match std::fs::remove_file(&entry.path) {
            Ok(()) => {
                self.status = format!("Macro « {} » supprimée.", entry.name);
                self.status_error = false;
                self.macro_selected = None;
                self.refresh_macros();
            }
            Err(err) => {
                self.status = format!("Suppression impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn macros_page(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Macros")
                .size(24.0)
                .strong()
                .color(Color32::from_rgb(35, 35, 35)),
        );
        ui.label(
            RichText::new("Bibliothèque de macros .mac du logiciel d'usine et de vos imports.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if ui.button("Rafraîchir").clicked() {
                self.refresh_macros();
                self.status = format!("{} macro(s) trouvée(s).", self.macros.len());
                self.status_error = false;
            }
            if ui.button("Importer la sélection").clicked() {
                self.import_selected_macro();
            }
            if ui.button("Supprimer (mes macros)").clicked() {
                self.delete_selected_macro();
            }
        });

        ui.add_space(8.0);
        egui::Frame::new()
            .fill(Color32::WHITE)
            .corner_radius(10)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_min_height(240.0);
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.macros.is_empty() {
                            ui.label(
                                RichText::new(
                                    "Aucune macro trouvée. Le logiciel d'usine enregistre ses macros dans le dossier Berserker\\keyboard de LOCALAPPDATA.",
                                )
                                .size(12.0)
                                .color(MUTED),
                            );
                        }
                        for (index, entry) in self.macros.iter().enumerate() {
                            let selected = self.macro_selected == Some(index);
                            let text = format!(
                                "{}  ·  {}  ·  {} événement(s)",
                                entry.name, entry.source, entry.events
                            );
                            let response =
                                ui.selectable_label(selected, RichText::new(text).size(12.5));
                            if response.clicked() {
                                self.macro_selected = Some(index);
                            }
                            response.on_hover_text(entry.path.display().to_string());
                        }
                    });
            });

        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Lecture seule côté clavier : le firmware ne reçoit aucune commande de macro. Le protocole d'écriture constructeur des macros n'est pas documenté et n'est pas utilisé ici.",
            )
            .size(11.0)
            .color(MUTED),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(&self.status)
                .size(11.0)
                .color(if self.status_error { ERR } else { OK }),
        );
    }

    fn refresh_system_from_device(&mut self) {
        let Some(controller) = &self.controller else {
            self.status = "Aucun clavier ouvert.".to_string();
            self.status_error = true;
            return;
        };
        match controller.read_system_settings() {
            Ok((poll, deb, win, arrow, anti)) => {
                self.polling_index = (poll as usize).min(3);
                self.debounce_index = (deb as usize).min(3);
                self.win_lock = win;
                self.arrow_mode = arrow;
                self.anti_ghosting = anti;
                self.status = "Réglages relus depuis le clavier.".to_string();
                self.status_error = false;
            }
            Err(err) => {
                self.status = format!("Lecture des réglages impossible : {err:#}");
                self.status_error = true;
            }
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Autres réglages")
                .size(24.0)
                .strong()
                .color(Color32::from_rgb(35, 35, 35)),
        );
        ui.label(
            RichText::new("Fréquence USB, debounce et options du clavier.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(12.0);

        egui::Frame::new()
            .fill(Color32::WHITE)
            .corner_radius(10)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("FRÉQUENCE USB DU CLAVIER")
                            .size(10.0)
                            .strong()
                            .color(MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("Relire").clicked() {
                            self.refresh_system_from_device();
                        }
                    });
                });
                ui.label(
                    RichText::new("Fréquence d'interrogation envoyée au PC.")
                        .size(11.0)
                        .color(MUTED),
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    let labels = ["1000 Hz", "500 Hz", "250 Hz", "125 Hz"];
                    for (index, label) in labels.iter().enumerate() {
                        if option_pill(ui, label, self.polling_index == index) {
                            self.apply_system_settings(
                                index,
                                self.debounce_index,
                                self.win_lock,
                                self.arrow_mode,
                                self.anti_ghosting,
                            );
                        }
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label(RichText::new("DEBOUNCE").size(10.0).strong().color(MUTED));
                ui.label(
                    RichText::new("Délai anti-rebond appliqué aux touches.")
                        .size(11.0)
                        .color(MUTED),
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    let labels = ["5 ms", "10 ms", "15 ms", "20 ms"];
                    for (index, label) in labels.iter().enumerate() {
                        if option_pill(ui, label, self.debounce_index == index) {
                            self.apply_system_settings(
                                self.polling_index,
                                index,
                                self.win_lock,
                                self.arrow_mode,
                                self.anti_ghosting,
                            );
                        }
                    }
                });
            });

        ui.add_space(10.0);
        egui::Frame::new()
            .fill(Color32::WHITE)
            .corner_radius(10)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("OPTIONS DU CLAVIER")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                ui.add_space(4.0);
                let mut win = self.win_lock;
                if ui
                    .checkbox(&mut win, "Mode Jeu — verrouille la touche Windows")
                    .changed()
                {
                    self.apply_system_settings(
                        self.polling_index,
                        self.debounce_index,
                        win,
                        self.arrow_mode,
                        self.anti_ghosting,
                    );
                }
                let mut arrow = self.arrow_mode;
                if ui
                    .checkbox(&mut arrow, "Mode flèche — WASD devient les flèches")
                    .changed()
                {
                    self.apply_system_settings(
                        self.polling_index,
                        self.debounce_index,
                        self.win_lock,
                        arrow,
                        self.anti_ghosting,
                    );
                }
                let mut anti = self.anti_ghosting;
                if ui
                    .checkbox(&mut anti, "Anti-ghosting — appuis simultanés fiables")
                    .changed()
                {
                    self.apply_system_settings(
                        self.polling_index,
                        self.debounce_index,
                        self.win_lock,
                        self.arrow_mode,
                        anti,
                    );
                }
            });

        ui.add_space(10.0);
        ui.label(
            RichText::new(&self.status)
                .size(11.0)
                .color(if self.status_error { ERR } else { OK }),
        );
    }

    fn effects_page(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.effects_page_content(ui);
            });
    }

    fn effects_page_content(&mut self, ui: &mut egui::Ui) {
        {
            let visuals = ui.visuals_mut();
            visuals.override_text_color = Some(Color32::from_rgb(40, 40, 40));
        }
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Effets superposables")
                        .size(20.0)
                        .strong()
                        .color(Color32::from_rgb(30, 30, 30)),
                );
                ui.label(
                    RichText::new(
                        "Sélectionnez des touches, puis ajoutez un ou plusieurs effets. Chaque calque garde sa zone ; le rendu est envoyé par l'app.",
                    )
                    .size(11.0)
                    .color(Color32::from_rgb(110, 110, 110)),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (state, color) = if self.controller.is_some() {
                    ("● Connecté", OK)
                } else {
                    ("● Hors ligne", ERR)
                };
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.label(RichText::new(state).size(11.0).color(color));
                    });
            });
        });

        ui.add_space(10.0);

        egui::Frame::new()
            .fill(CARD)
            .corner_radius(10)
            .stroke(Stroke::new(1.0, Color32::from_rgb(220, 220, 220)))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Clavier")
                            .size(13.0)
                            .strong()
                            .color(Color32::from_rgb(45, 45, 45)),
                    );
                    ui.label(
                        RichText::new(format!("{} touche(s) sélectionnée(s)", self.selected.len()))
                            .size(11.0)
                            .color(if self.selected.is_empty() {
                                Color32::from_rgb(130, 130, 130)
                            } else {
                                ACCENT
                            }),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Relire").clicked() {
                            self.refresh_lighting_from_device();
                        }
                        if ui.button("Aucune").clicked() {
                            self.selected.clear();
                        }
                        if ui.button("Tout").clicked() {
                            self.selected = (0..self.keys.len()).collect();
                        }
                    });
                });
                let base_note = if !self.layers.is_empty() && self.live.is_some() {
                    format!(
                        "Aperçu des {} calque(s) PC actif(s) — la zone non couverte garde la base.",
                        self.layers.len()
                    )
                } else if self.mode_index == 0 {
                    "Base : couleurs par touche du profil.".to_string()
                } else {
                    let base = self.effect_preview_color();
                    format!(
                        "Base : effet firmware « {} » · #{:02X}{:02X}{:02X}.",
                        MODES[self.mode_index - 1].0,
                        base[0],
                        base[1],
                        base[2]
                    )
                };
                ui.label(RichText::new(base_note).size(10.5).color(MUTED));
                ui.label(
                    RichText::new(
                        "Fenêtre réduite ou application fermée : la dernière image des effets reste figée sur le clavier ; les calques sont rejoués au prochain lancement.",
                    )
                    .size(9.5)
                    .color(MUTED),
                );
                ui.add_space(4.0);
                self.keyboard(ui, self.controller.is_some());
            });

        ui.add_space(10.0);

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(410.0);
                self.effect_library(ui);
            });
            ui.add_space(10.0);
            ui.vertical(|ui| {
                self.layers_panel(ui);
            });
        });

        ui.add_space(6.0);
        ui.label(
            RichText::new(&self.status)
                .size(11.0)
                .color(if self.status_error { ERR } else { OK }),
        );
    }

    fn effect_library(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(CARD)
            .corner_radius(10)
            .stroke(Stroke::new(1.0, Color32::from_rgb(220, 220, 220)))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Bibliothèque d'effets")
                        .size(13.0)
                        .strong()
                        .color(Color32::from_rgb(45, 45, 45)),
                );
                let zone = if self.selected.is_empty() {
                    "Aucune sélection : ajout à tout le clavier".to_string()
                } else {
                    format!("Zone d'ajout : {} touche(s) sélectionnée(s)", self.selected.len())
                };
                ui.label(RichText::new(zone).size(10.0).color(MUTED));
                ui.add_space(4.0);
                egui::Grid::new("effect_library_grid")
                    .num_columns(2)
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        for (index, kind) in EffectKind::ALL.into_iter().enumerate() {
                            let button = egui::Button::new(
                                RichText::new(kind.label()).size(11.5).color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(52, 52, 52))
                            .min_size(Vec2::new(180.0, 32.0));
                            if ui
                                .add(button)
                                .on_hover_text(kind.description())
                                .clicked()
                            {
                                self.add_effect_layer(kind);
                            }
                            if index % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Couleur et vitesse courantes. Les calques se cumulent dans l'ordre de la liste.",
                    )
                    .size(9.0)
                    .color(MUTED),
                );
            });
    }

    fn layers_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(CARD)
            .corner_radius(10)
            .stroke(Stroke::new(1.0, Color32::from_rgb(220, 220, 220)))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Calques ({})", self.layers.len()))
                            .size(13.0)
                            .strong()
                            .color(Color32::from_rgb(45, 45, 45)),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !self.layers.is_empty() {
                            if ui.button("Tout effacer").clicked() {
                                self.layers.clear();
                                self.selected_layer = None;
                                self.stop_live();
                                let _ = self.save_profile_files();
                                self.status = "Tous les calques PC ont été supprimés.".to_string();
                                self.status_error = false;
                            }
                            if self.live.is_some() {
                                if ui.button("Arrêter").clicked() {
                                    self.stop_live();
                                    self.status =
                                        "Effets PC en pause ; calques conservés.".to_string();
                                    self.status_error = false;
                                }
                            } else if ui.button("Démarrer").clicked() {
                                if self.start_live() {
                                    self.status = "Effets PC démarrés.".to_string();
                                    self.status_error = false;
                                } else {
                                    self.status = "Clavier indisponible.".to_string();
                                    self.status_error = true;
                                }
                            }
                        }
                    });
                });

                if self.layers.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Aucun calque. Sélectionnez des touches puis cliquez un effet dans la bibliothèque.",
                        )
                        .size(10.5)
                        .color(MUTED),
                    );
                    return;
                }

                ui.add_space(4.0);
                let mut action: Option<(usize, isize)> = None;
                let mut remove_index: Option<usize> = None;
                egui::ScrollArea::vertical()
                    .max_height(208.0)
                    .show(ui, |ui| {
                        for index in 0..self.layers.len() {
                            let layer = self.layers[index].clone();
                            let selected = self.selected_layer == Some(index);
                            let mut enabled = layer.enabled;
                            let mut up = false;
                            let mut down = false;
                            let mut remove = false;
                            egui::Frame::new()
                                .fill(if selected {
                                    Color32::from_rgb(235, 235, 235)
                                } else {
                                    Color32::from_rgb(246, 246, 246)
                                })
                                .corner_radius(6)
                                .inner_margin(egui::Margin::symmetric(7, 5))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut enabled, "");
                                        if ui
                                            .selectable_label(
                                                selected,
                                                RichText::new(layer.kind.label())
                                                    .size(11.5)
                                                    .strong(),
                                            )
                                            .clicked()
                                        {
                                            self.selected_layer = Some(index);
                                        }
                                        ui.label(
                                            RichText::new(layer.zone_label())
                                                .size(9.5)
                                                .color(MUTED),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if ui.small_button("×").clicked() {
                                                    remove = true;
                                                }
                                                if arrow_button(ui, false).clicked() {
                                                    down = true;
                                                }
                                                if arrow_button(ui, true).clicked() {
                                                    up = true;
                                                }
                                                let (rect, _) = ui.allocate_exact_size(
                                                    Vec2::new(14.0, 14.0),
                                                    Sense::hover(),
                                                );
                                                paint_color_swatch(
                                                    ui.painter(),
                                                    rect,
                                                    3,
                                                    layer.rgb,
                                                    false,
                                                );
                                            },
                                        );
                                    });
                                });
                            if enabled != layer.enabled {
                                self.layers[index].enabled = enabled;
                                self.layer_changed();
                            }
                            if up {
                                action = Some((index, -1));
                                break;
                            }
                            if down {
                                action = Some((index, 1));
                                break;
                            }
                            if remove {
                                remove_index = Some(index);
                                break;
                            }
                        }
                    });
                if let Some(index) = remove_index {
                    self.remove_effect_layer(index);
                } else if let Some((index, delta)) = action {
                    self.move_effect_layer(index, delta);
                }

                if let Some(index) = self
                    .selected_layer
                    .filter(|index| *index < self.layers.len())
                {
                    ui.separator();
                    self.layer_editor(ui, index);
                }
            });
    }

    fn layer_editor(&mut self, ui: &mut egui::Ui, index: usize) {
        let layer = self.layers[index].clone();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Calque {} · {}", index + 1, layer.kind.label()))
                    .size(12.0)
                    .strong()
                    .color(Color32::from_rgb(45, 45, 45)),
            );
            ui.label(RichText::new(layer.zone_label()).size(10.0).color(MUTED));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Couleur").size(10.0).color(MUTED));
            egui::Grid::new("layer_color_grid")
                .spacing(Vec2::new(4.0, 4.0))
                .show(ui, |ui| {
                    for (i, color) in PALETTE.iter().enumerate() {
                        let selected = self.layers[index].rgb == *color;
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
                        paint_color_swatch(ui.painter(), rect, 3, *color, selected);
                        if response.clicked() {
                            self.layers[index].rgb = *color;
                            self.layer_changed();
                        }
                        if (i + 1) % 8 == 0 {
                            ui.end_row();
                        }
                    }
                });
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Libre").size(9.0).color(MUTED));
                let mut rgb = self.layers[index].rgb;
                let custom_selected = !PALETTE.contains(&rgb);
                if color_well(ui, &mut rgb, custom_selected).changed() {
                    self.layers[index].rgb = rgb;
                    self.layer_changed();
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Vitesse").size(10.0).color(MUTED));
            let mut speed = layer.speed;
            let response = ui.add_sized(
                [120.0, 18.0],
                egui::Slider::new(&mut speed, 0..=protocol::SPEED_UI_MAX).show_value(true),
            );
            if response.changed() {
                self.layers[index].speed = speed;
                if !response.dragged() {
                    self.layer_changed();
                }
            }
            if response.drag_stopped() {
                self.layer_changed();
            }
            let mut reverse = layer.reverse;
            if ui.checkbox(&mut reverse, "Inverser").changed() {
                self.layers[index].reverse = reverse;
                self.layer_changed();
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Mélange").size(10.0).color(MUTED));
            let mut blend = layer.blend;
            egui::ComboBox::from_id_salt("layer_blend_combo")
                .width(110.0)
                .selected_text(blend.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut blend, BlendMode::Add, BlendMode::Add.label());
                    ui.selectable_value(&mut blend, BlendMode::Replace, BlendMode::Replace.label());
                });
            if blend != layer.blend {
                self.layers[index].blend = blend;
                self.layer_changed();
            }
            let zone_text = if self.selected.is_empty() {
                "Appliquer à tout le clavier".to_string()
            } else {
                format!("Appliquer à la sélection ({})", self.selected.len())
            };
            if ui
                .button(zone_text)
                .on_hover_text("La zone du calque devient la sélection courante")
                .clicked()
            {
                self.assign_layer_zone(index);
            }
        });
        if layer.kind == EffectKind::Cycle {
            ui.label(
                RichText::new("Le cycle ignore la couleur du calque.")
                    .size(9.0)
                    .color(MUTED),
            );
        }
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        {
            let visuals = ui.visuals_mut();
            visuals.override_text_color = Some(Color32::from_rgb(40, 40, 40));
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Z)) {
            self.undo();
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Y)) {
            self.redo();
        }

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Éclairage du clavier")
                        .size(20.0)
                        .strong()
                        .color(Color32::from_rgb(30, 30, 30)),
                );
                ui.label(
                    RichText::new(
                        "Clic = une touche · Ctrl+clic = ajout/retrait · glisser = sélection rectangulaire",
                    )
                    .size(11.0)
                    .color(Color32::from_rgb(110, 110, 110)),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Reconnecter").clicked() {
                    self.reconnect();
                }
                let (state, color) = if self.controller.is_some() {
                    ("● Connecté", OK)
                } else {
                    ("● Hors ligne", ERR)
                };
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.label(RichText::new(state).size(11.0).color(color));
                    });
            });
        });

        ui.add_space(10.0);

        egui::Frame::new()
            .fill(CARD)
            .corner_radius(10)
            .stroke(Stroke::new(1.0, Color32::from_rgb(220, 220, 220)))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Clavier")
                            .size(13.0)
                            .strong()
                            .color(Color32::from_rgb(45, 45, 45)),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} touche(s) sélectionnée(s)",
                            self.selected.len()
                        ))
                        .size(11.0)
                        .color(if self.selected.is_empty() {
                            Color32::from_rgb(130, 130, 130)
                        } else {
                            ACCENT
                        }),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Relire").clicked() {
                            self.refresh_lighting_from_device();
                        }
                        if ui.button("Aucune").clicked() {
                            self.selected.clear();
                        }
                        if ui.button("Tout").clicked() {
                            self.selected = (0..self.keys.len()).collect();
                        }
                    });
                });
                if self.mode_index != 0 && self.live.is_none() {
                    let base = self.effect_preview_color();
                    ui.label(
                        RichText::new(format!(
                            "Aperçu de l'effet « {} » — couleur de base #{:02X}{:02X}{:02X}. Les couleurs par touche sont masquées.",
                            MODES[self.mode_index - 1].0, base[0], base[1], base[2]
                        ))
                        .size(10.5)
                        .color(MUTED),
                    );
                }
                ui.add_space(4.0);
                let available = self.controller.is_some();
                self.keyboard(ui, available);
            });

        ui.add_space(10.0);

        egui::Frame::new()
            .fill(PANEL)
            .corner_radius(10)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                self.lighting_panel(ui);
            });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("EVision V2 · configuration uniquement · aucun flash firmware")
                    .size(10.0)
                    .color(Color32::from_rgb(140, 140, 140)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("Berserker · 320F:5041")
                        .size(10.0)
                        .color(Color32::from_rgb(160, 160, 160)),
                );
            });
        });
    }
}

#[derive(Clone, Copy)]
enum KeyIcon {
    Backspace,
    Enter,
    Tab,
    Caps,
    Shift,
    Win,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

/// Rectangle de la barre verticale et extension supérieure gauche pour une
/// touche Entrée ISO. Retourne `None` pour les touches rectangulaires.
fn iso_enter_parts(key: &layout::KeyDef, origin: Pos2, scale: f32) -> Option<(Rect, Rect)> {
    let layout::KeyShape::IsoEnter { left_ext } = key.shape else {
        return None;
    };
    let main = Rect::from_min_size(
        origin
            + Vec2::new(
                (key.x - layout::BODY_X) * scale,
                (key.y - layout::BODY_Y) * scale,
            ),
        Vec2::new((key.w * scale).max(4.0), (key.h * scale).max(4.0)),
    );
    let step_y = main.top() + main.height() * 0.5;
    let ext_left = main.left() - left_ext * scale;
    let ext = Rect::from_min_max(pos2(ext_left, main.top()), pos2(main.left(), step_y));
    Some((main, ext))
}

/// Contour arrondi de la touche Entrée ISO, construit à partir de six sommets
/// (les coins extérieurs sont arrondis, le coin rentrant du L reste net).
fn iso_enter_outline_points(main: Rect, ext_left: f32, step_y: f32, radius: f32) -> Vec<Pos2> {
    let points = [
        pos2(ext_left, main.top()),
        pos2(main.right(), main.top()),
        pos2(main.right(), main.bottom()),
        pos2(main.left(), main.bottom()),
        pos2(main.left(), step_y),
        pos2(ext_left, step_y),
    ];
    let rounded = [true, true, true, true, false, true];
    let r = radius.min((main.left() - ext_left) * 0.45).max(1.0);
    let mut out = Vec::with_capacity(48);
    for i in 0..points.len() {
        let curr = points[i];
        if !rounded[i] {
            out.push(curr);
            continue;
        }
        let prev = points[(i + points.len() - 1) % points.len()];
        let next = points[(i + 1) % points.len()];
        let p1 = curr + (prev - curr).normalized() * r;
        let p2 = curr + (next - curr).normalized() * r;
        out.push(p1);
        for t in [0.25f32, 0.5, 0.75] {
            let a = (1.0 - t) * (1.0 - t);
            let b = 2.0 * (1.0 - t) * t;
            let c = t * t;
            out.push(pos2(
                p1.x * a + curr.x * b + p2.x * c,
                p1.y * a + curr.y * b + p2.y * c,
            ));
        }
        out.push(p2);
    }
    out
}

fn paint_iso_enter_outline(
    painter: &egui::Painter,
    main: Rect,
    ext_left: f32,
    step_y: f32,
    radius: f32,
    width: f32,
    color: Color32,
) {
    let points = iso_enter_outline_points(main, ext_left, step_y, radius);
    let mut stroke = egui::epaint::PathStroke::new(width, color);
    stroke.kind = StrokeKind::Inside;
    painter.add(egui::Shape::Path(egui::epaint::PathShape::closed_line(
        points, stroke,
    )));
}

fn draw_key_icon(painter: &egui::Painter, rect: Rect, icon: KeyIcon, color: Color32, _scale: f32) {
    let c = rect.center();
    let s = (rect.height() * 0.18).clamp(3.0, 6.0);
    let stroke = Stroke::new(1.4, color);
    match icon {
        KeyIcon::Backspace => {
            let left = c.x - s * 1.4;
            let right = c.x + s * 1.4;
            painter.line_segment([pos2(right, c.y), pos2(left, c.y)], stroke);
            painter.line_segment(
                [pos2(left, c.y), pos2(left + s * 0.8, c.y - s * 0.8)],
                stroke,
            );
            painter.line_segment(
                [pos2(left, c.y), pos2(left + s * 0.8, c.y + s * 0.8)],
                stroke,
            );
        }
        KeyIcon::Enter => {
            let x = c.x + s * 1.2;
            let y_bot = c.y + s * 0.9;
            painter.line_segment([pos2(x, c.y - s), pos2(x, y_bot)], stroke);
            painter.line_segment([pos2(x, y_bot), pos2(x - s * 2.4, y_bot)], stroke);
            painter.line_segment(
                [pos2(x - s * 2.4, y_bot), pos2(x - s * 1.5, y_bot - s * 0.8)],
                stroke,
            );
            painter.line_segment(
                [pos2(x - s * 2.4, y_bot), pos2(x - s * 1.5, y_bot + s * 0.8)],
                stroke,
            );
        }
        KeyIcon::Tab => {
            let x1 = c.x - s * 1.3;
            let x2 = c.x + s * 1.3;
            painter.line_segment([pos2(x1, c.y), pos2(x2, c.y)], stroke);
            painter.line_segment([pos2(x2, c.y), pos2(x2 - s * 0.8, c.y - s * 0.8)], stroke);
            painter.line_segment([pos2(x2, c.y), pos2(x2 - s * 0.8, c.y + s * 0.8)], stroke);
            painter.line_segment(
                [
                    pos2(x2 + s * 0.4, c.y - s * 1.1),
                    pos2(x2 + s * 0.4, c.y + s * 1.1),
                ],
                stroke,
            );
        }
        KeyIcon::Caps => {
            let base = c.y + s * 0.9;
            painter.line_segment(
                [pos2(c.x - s * 1.1, base), pos2(c.x + s * 1.1, base)],
                stroke,
            );
            painter.line_segment([pos2(c.x, c.y - s * 1.2), pos2(c.x, c.y + s * 0.2)], stroke);
            painter.line_segment(
                [pos2(c.x, c.y + s * 0.2), pos2(c.x - s * 0.8, c.y - s * 0.4)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x, c.y + s * 0.2), pos2(c.x + s * 0.8, c.y - s * 0.4)],
                stroke,
            );
        }
        KeyIcon::Shift => {
            painter.line_segment(
                [
                    pos2(c.x - s * 1.1, c.y + s * 0.9),
                    pos2(c.x + s * 1.1, c.y + s * 0.9),
                ],
                stroke,
            );
            painter.line_segment([pos2(c.x, c.y - s * 1.3), pos2(c.x, c.y + s * 0.4)], stroke);
            painter.line_segment(
                [pos2(c.x, c.y - s * 1.3), pos2(c.x - s * 0.8, c.y - s * 0.5)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x, c.y - s * 1.3), pos2(c.x + s * 0.8, c.y - s * 0.5)],
                stroke,
            );
        }
        KeyIcon::Win => {
            let q = s * 0.78;
            let gap = s * 0.16;
            for (dx, dy) in [
                (-q - gap, -q - gap),
                (gap, -q - gap),
                (-q - gap, gap),
                (gap, gap),
            ] {
                let r = Rect::from_min_size(pos2(c.x + dx, c.y + dy), Vec2::splat(q));
                painter.rect_filled(r, CornerRadius::ZERO, color);
            }
        }
        KeyIcon::Space => {
            painter.line_segment(
                [pos2(c.x - s * 2.4, c.y), pos2(c.x + s * 2.4, c.y)],
                Stroke::new(
                    1.6,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 150),
                ),
            );
        }
        KeyIcon::ArrowUp => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(c.x, c.y - s * 1.2),
                    pos2(c.x - s, c.y + s * 0.7),
                    pos2(c.x + s, c.y + s * 0.7),
                ],
                color,
                Stroke::NONE,
            ));
        }
        KeyIcon::ArrowDown => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(c.x, c.y + s * 1.2),
                    pos2(c.x - s, c.y - s * 0.7),
                    pos2(c.x + s, c.y - s * 0.7),
                ],
                color,
                Stroke::NONE,
            ));
        }
        KeyIcon::ArrowLeft => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(c.x - s * 1.2, c.y),
                    pos2(c.x + s * 0.7, c.y - s),
                    pos2(c.x + s * 0.7, c.y + s),
                ],
                color,
                Stroke::NONE,
            ));
        }
        KeyIcon::ArrowRight => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(c.x + s * 1.2, c.y),
                    pos2(c.x - s * 0.7, c.y - s),
                    pos2(c.x - s * 0.7, c.y + s),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
}

fn apply_click(selected: &mut HashSet<usize>, index: Option<usize>, ctrl: bool) {
    match (index, ctrl) {
        (Some(index), true) => {
            if !selected.remove(&index) {
                selected.insert(index);
            }
        }
        (Some(index), false) => {
            selected.clear();
            selected.insert(index);
        }
        (None, false) => selected.clear(),
        (None, true) => {}
    }
}

fn apply_rect_selection(
    selected: &mut HashSet<usize>,
    rects: &[(usize, Rect)],
    area: Rect,
    ctrl: bool,
) {
    if !ctrl {
        selected.clear();
    }
    for (index, rect) in rects {
        if rect.intersects(area) {
            selected.insert(*index);
        }
    }
}

fn scan_macros(dir: &std::path::Path, source: &'static str) -> Vec<MacroEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("mac") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = json_string_field(&text, "MacroName").unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "macro".to_string())
        });
        let events = text.matches("\"Action\"").count();
        entries.push(MacroEntry {
            name,
            path,
            events,
            source,
        });
    }
    entries
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let quote = rest.find('"')?;
    let rest = &rest[quote + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn option_pill(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let text = RichText::new(label).size(12.0).strong().color(if selected {
        Color32::BLACK
    } else {
        Color32::from_rgb(55, 55, 55)
    });
    let button = egui::Button::new(text)
        .fill(if selected {
            ACCENT
        } else {
            Color32::from_rgb(238, 238, 238)
        })
        .stroke(Stroke::new(
            1.0,
            if selected {
                ACCENT
            } else {
                Color32::from_rgb(210, 210, 210)
            },
        ))
        .corner_radius(5);
    ui.add_sized([108.0, 34.0], button).clicked()
}

fn menu_item(ui: &mut egui::Ui, label: &str, selected: bool, enabled: bool) -> bool {
    let text = RichText::new(label).size(12.0).color(if !enabled {
        Color32::from_rgb(120, 120, 120)
    } else if selected {
        Color32::WHITE
    } else {
        Color32::from_rgb(205, 205, 205)
    });
    let button = egui::Button::new(text)
        .fill(if selected { SIDEBAR_ITEM } else { SIDEBAR })
        .stroke(Stroke::NONE);
    let response = ui.add_enabled_ui(enabled, |ui| {
        ui.add_sized([ui.available_width(), 34.0], button)
    });
    if selected {
        let rect = response.inner.rect;
        ui.painter().rect_filled(
            Rect::from_min_size(rect.left_top(), Vec2::new(3.0, rect.height())),
            CornerRadius::ZERO,
            ACCENT,
        );
    }
    enabled && response.inner.clicked()
}

impl eframe::App for App {
    #[cfg(not(windows))]
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.freeze_live_frame() {
            self.stop_live();
        }
        let _ = self.save_profile_files();
    }

    #[cfg(windows)]
    fn on_exit(&mut self) {
        if !self.freeze_live_frame() {
            self.stop_live();
        }
        let _ = self.save_profile_files();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_window_visibility(ui.ctx());
        self.tick_live(ui.ctx());
        egui::Panel::left("sidebar")
            .exact_size(212.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(14, 16)),
            )
            .show(ui, |ui| {
                self.sidebar(ui);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(LIGHT_BG)
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ui, |ui| match self.page {
                Page::Lighting => self.central(ui),
                Page::Effects => self.effects_page(ui),
                Page::Macros => self.macros_page(ui),
                Page::Settings => self.settings_page(ui),
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn rect(x: f32) -> Rect {
        Rect::from_min_size(pos2(x, 0.0), vec2(10.0, 10.0))
    }

    #[test]
    fn clic_simple_remplace_la_selection() {
        let mut selected = HashSet::new();
        apply_click(&mut selected, Some(1), false);
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&1));
        apply_click(&mut selected, Some(2), false);
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&2));
        apply_click(&mut selected, None, false);
        assert!(selected.is_empty());
    }

    #[test]
    fn ctrl_clic_ajoute_et_retire() {
        let mut selected = HashSet::new();
        apply_click(&mut selected, Some(10), true);
        apply_click(&mut selected, Some(20), true);
        apply_click(&mut selected, Some(30), true);
        assert_eq!(selected.len(), 3);
        assert!(selected.contains(&10));
        assert!(selected.contains(&20));
        assert!(selected.contains(&30));
        apply_click(&mut selected, Some(20), true);
        assert_eq!(selected.len(), 2);
        assert!(!selected.contains(&20));
    }

    #[test]
    fn rectangle_selectionne_les_touches_intersectees() {
        let rects = vec![(0, rect(0.0)), (1, rect(20.0)), (2, rect(40.0))];
        let mut selected = HashSet::new();
        apply_rect_selection(
            &mut selected,
            &rects,
            Rect::from_min_size(pos2(5.0, 0.0), vec2(25.0, 10.0)),
            false,
        );
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&0));
        assert!(selected.contains(&1));
    }

    #[test]
    fn ctrl_rectangle_ajoute_a_la_selection() {
        let rects = vec![(0, rect(0.0)), (1, rect(20.0)), (2, rect(40.0))];
        let mut selected = HashSet::new();
        selected.insert(2);
        apply_rect_selection(
            &mut selected,
            &rects,
            Rect::from_min_size(pos2(5.0, 0.0), vec2(25.0, 10.0)),
            true,
        );
        assert_eq!(selected.len(), 3);
        assert!(selected.contains(&0));
        assert!(selected.contains(&1));
        assert!(selected.contains(&2));
    }

    #[test]
    fn layout_couvre_les_touches_principales() {
        let keys = layout::keys();
        assert!(keys.iter().any(|k| k.label == "D" && k.pos == 66));
        assert!(keys.iter().any(|k| k.label == "N" && k.pos == 91));
        assert!(keys.iter().any(|k| k.label == "U" && k.pos == 49));
        assert!(keys.iter().any(|k| k.pos == 111));
        assert!(keys.iter().any(|k| k.label == "Inser" && k.pos == 35));
        assert!(keys.iter().all(|k| k.pos < protocol::LED_COUNT));
    }

    #[test]
    fn couleur_utf8_invalide_rejetee_sans_panique() {
        assert!(parse_hex_color("a€bb").is_err());
        assert!(parse_hex_color("€€").is_err());
        assert!(parse_hex_color("éééééé").is_err());
        assert_eq!(parse_hex_color("#ff6000").unwrap(), [255, 96, 0]);
    }

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("berserker-rgb-{}-{name}", std::process::id()))
    }

    #[test]
    fn etat_utf8_invalide_ne_panique() {
        let path = test_path("state-utf8.state");
        std::fs::write(&path, "version=3\nmode=0\nrgb=a€bb\n").unwrap();
        assert!(read_profile_state(&path).is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn calques_utf8_invalide_ne_panique() {
        let path = test_path("layers-utf8.layers");
        std::fs::write(&path, "version=1\nlayer=2;1;0;0;1;a€bb;\n").unwrap();
        assert!(read_layers_file(&path).is_empty());
        let _ = std::fs::remove_file(path);
    }
}
