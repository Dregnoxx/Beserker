//! Disposition TKL ISO du Berserker, construite sur une grille d'unités
//! régulière (1u = 29,5 px dans le repère du skin d'usine) : toutes les
//! touches 1u partagent la même largeur et le même pas, les touches larges
//! font des multiples exacts. Les libellés reprennent les légendes des
//! keycaps AZERTY.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyShape {
    Rect,
    /// Touche Entrée ISO : barre verticale sur toute la hauteur, avec une
    /// extension vers la gauche sur la rangée du haut (forme en L).
    IsoEnter {
        left_ext: f32,
    },
}

#[derive(Clone, Debug)]
pub struct KeyDef {
    pub pos: usize,
    pub label: &'static str,
    /// Rectangle du keycap (et non de la cellule) dans le repère du skin.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub shape: KeyShape,
}

/// Coordonnées du skin d'usine (image 673×218), rognées autour du clavier.
pub const BODY_X: f32 = 60.0;
pub const BODY_Y: f32 = 7.0;
pub const TOTAL_W: f32 = 580.0;
pub const TOTAL_H: f32 = 208.0;

/// Pas horizontal d'une touche 1u et marge entre deux keycaps.
const U: f32 = 29.5;
const CAP_MARGIN: f32 = 2.0;
const CAP_H: f32 = 24.0;
/// Bord gauche du bloc principal (cellule de Tab / Échap).
const X0: f32 = 72.0;

// Rangées : pas vertical régulier de 29,5 px, sauf l'espace sous la rangée F.
const ROW_F: f32 = 17.0;
const ROW_NUM: f32 = 60.0;
const ROW_TAB: f32 = 89.5;
const ROW_CAPS: f32 = 119.0;
const ROW_SHIFT: f32 = 148.5;
const ROW_BOTTOM: f32 = 178.0;

/// Bloc navigation / flèches : même pas de 1u, colonnes 16 à 18.
const NAV_CELL: f32 = 16.0;

/// Touche 1u ou de largeur `span` unités, positionnée par sa cellule.
fn kc(pos: usize, label: &'static str, cell: f32, span: f32, y: f32) -> KeyDef {
    KeyDef {
        pos,
        label,
        x: X0 + cell * U + CAP_MARGIN,
        y,
        w: span * U - 2.0 * CAP_MARGIN,
        h: CAP_H,
        shape: KeyShape::Rect,
    }
}

/// Entrée ISO : cellule basse de 1,25u et extension haute de 1,5u.
fn iso_enter() -> KeyDef {
    let top_cell = 13.5;
    let bottom_cell = 13.75;
    let bottom_x = X0 + bottom_cell * U + CAP_MARGIN;
    let top_x = X0 + top_cell * U + CAP_MARGIN;
    KeyDef {
        pos: 76,
        label: "",
        x: bottom_x,
        y: ROW_TAB,
        w: 1.25 * U - 2.0 * CAP_MARGIN,
        h: (ROW_CAPS + CAP_H) - ROW_TAB,
        shape: KeyShape::IsoEnter {
            left_ext: bottom_x - top_x,
        },
    }
}

pub fn keys() -> Vec<KeyDef> {
    let mut keys = Vec::new();

    // Rangée F : Esc, F1-F4, F5-F8, F9-F12 avec les demi-pas d'usage.
    keys.push(kc(0, "Échap", 0.0, 1.0, ROW_F));
    let fkeys = [
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    ];
    let fcell = [
        2.0, 3.0, 4.0, 5.0, 6.5, 7.5, 8.5, 9.5, 11.0, 12.0, 13.0, 14.0,
    ];
    for (i, label) in fkeys.iter().enumerate() {
        keys.push(kc(2 + i, label, fcell[i], 1.0, ROW_F));
    }
    keys.push(kc(14, "Impr\nécran", NAV_CELL, 1.0, ROW_F));
    keys.push(kc(15, "Arrêt\ndéfil", NAV_CELL + 1.0, 1.0, ROW_F));
    keys.push(kc(16, "Pause", NAV_CELL + 2.0, 1.0, ROW_F));

    // Rangée des chiffres AZERTY (légendes primaire + chiffre).
    let nums = [
        "²", "&1", "é2", "\"3", "'4", "(5", "-6", "è7", "_8", "ç9", "à0", ")°", "=+",
    ];
    for (i, label) in nums.iter().enumerate() {
        keys.push(kc(21 + i, label, i as f32, 1.0, ROW_NUM));
    }
    keys.push(kc(34, "", 13.0, 2.0, ROW_NUM));

    // Navigation haute.
    keys.push(kc(35, "Inser", NAV_CELL, 1.0, ROW_NUM));
    keys.push(kc(36, "Orig", NAV_CELL + 1.0, 1.0, ROW_NUM));
    keys.push(kc(37, "PgPr", NAV_CELL + 2.0, 1.0, ROW_NUM));

    // Rangée Tab / A Z E R T Y U I O P / Entrée.
    keys.push(kc(42, "", 0.0, 1.5, ROW_TAB));
    let row2 = ["A", "Z", "E", "R", "T", "Y", "U", "I", "O", "P", "^¨", "$£"];
    for (i, label) in row2.iter().enumerate() {
        keys.push(kc(43 + i, label, 1.5 + i as f32, 1.0, ROW_TAB));
    }
    keys.push(iso_enter());

    // Navigation milieu.
    keys.push(kc(56, "Suppr", NAV_CELL, 1.0, ROW_TAB));
    keys.push(kc(57, "Fin", NAV_CELL + 1.0, 1.0, ROW_TAB));
    keys.push(kc(58, "PgSv", NAV_CELL + 2.0, 1.0, ROW_TAB));

    // Rangée QSDF / M ù% *µ.
    keys.push(kc(63, "", 0.0, 1.75, ROW_CAPS));
    let row3 = ["Q", "S", "D", "F", "G", "H", "J", "K", "L", "M", "ù%", "*µ"];
    for (i, label) in row3.iter().enumerate() {
        keys.push(kc(64 + i, label, 1.75 + i as f32, 1.0, ROW_CAPS));
    }

    // Rangée WXCV : Maj gauche 1,25u, <> 1u, puis 10 touches, Maj droite 2,75u.
    keys.push(kc(84, "", 0.0, 1.25, ROW_SHIFT));
    keys.push(kc(85, "<>", 1.25, 1.0, ROW_SHIFT));
    let row4 = ["W", "X", "C", "V", "B", "N", ",?", ";·", ":/", "!§"];
    for (i, label) in row4.iter().enumerate() {
        keys.push(kc(86 + i, label, 2.25 + i as f32, 1.0, ROW_SHIFT));
    }
    keys.push(kc(97, "", 12.25, 2.75, ROW_SHIFT));

    // Flèche haut, alignée sur la colonne du milieu du bloc navigation.
    keys.push(kc(99, "", NAV_CELL + 1.0, 1.0, ROW_SHIFT));

    // Rangée inférieure : 7 modificateurs de 1,25u + espace 6,25u = 15u.
    keys.push(kc(105, "Ctrl", 0.0, 1.25, ROW_BOTTOM));
    keys.push(kc(106, "", 1.25, 1.25, ROW_BOTTOM));
    keys.push(kc(107, "Alt", 2.5, 1.25, ROW_BOTTOM));
    keys.push(kc(111, "", 3.75, 6.25, ROW_BOTTOM));
    keys.push(kc(115, "AltGr", 10.0, 1.25, ROW_BOTTOM));
    keys.push(kc(116, "", 11.25, 1.25, ROW_BOTTOM));
    keys.push(kc(117, "Fn", 12.5, 1.25, ROW_BOTTOM));
    keys.push(kc(118, "Ctrl", 13.75, 1.25, ROW_BOTTOM));

    // Flèches gauche / bas / droite.
    keys.push(kc(119, "", NAV_CELL, 1.0, ROW_BOTTOM));
    keys.push(kc(120, "", NAV_CELL + 1.0, 1.0, ROW_BOTTOM));
    keys.push(kc(121, "", NAV_CELL + 2.0, 1.0, ROW_BOTTOM));

    keys
}
