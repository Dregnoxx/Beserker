# Changelog

Toutes les versions sont des versions alpha. Les binaires correspondants sont
publiés dans les releases GitHub.

## 1.1.4 — 2026-10-30

### Ajouté

- Sélecteur de couleur libre « Roue » (HSV/RGB avec saisie numérique) dans la
  page Éclairage et dans l'éditeur de calque.
- Contour de sélection des pastilles en double anneau sombre/clair, lisible
  sur toutes les teintes.
- Boutons flèches monter/descendre dessinés pour réordonner les calques, avec
  infobulles.

### Modifié

- Libellés d'action : « Appliquer à la sélection (n) », « Appliquer à tout le
  clavier », échelle « lent · rapide ».
- Paquet Linux : binaire marqué exécutable (0755) dans le `tar.gz`.

## 1.1.3 — 2026-10-30

### Ajouté

- Page **Effets** avec bibliothèque de 11 effets logiciels.
- Calques superposables par sélection de touches, jusqu'à 8 calques.
- Couleur, vitesse, sens, mélange, activation et ordre par calque.
- Persistance des calques dans `profil_N.layers`, rejoués au démarrage.

### Modifié

- La base hors zones reste l'effet firmware en cours ou les couleurs
  per-touche.

## 1.1.2 — 2026-10-30

### Modifié

- Exe Windows en sous-système GUI : une seule fenêtre, sans console.
- Options CLI conservées par rattachement à la console parente.
- Clavier du GUI reconstruit sur une grille 1u régulière.
- Touche Entrée ISO en L alignée sur la grille.
- Aperçu d'effet et mention du mode effet clarifiés.

## 1.1.1 — 2026-10-30

### Ajouté

- Touche Entrée ISO dessinée en L avec contour, survol, sélection et zones
  cliquables fidèles à la forme.

### Modifié

- Exe Windows natif : Direct3D 12 (GPU/WARP), CRT statique, sans DLL tierce.
- Textes d'aperçu d'effet et de mode effet clarifiés.

## 1.1.0 — 2026-10-30

### Ajouté

- Profils 1-10 complets (couleurs, mode, luminosité, vitesse, direction,
  réglages système).
- Profil actif mémorisé et restauré au démarrage.
- Page **Autres réglages** : fréquence USB, debounce, Mode Jeu, Mode flèche,
  Anti-ghosting, avec relecture de contrôle.
- Liste d'effets EVision V2 vérifiée et vitesse 0-4.
- AppImage Linux x86_64.
- Effets live PC, undo/redo, sauvegarde/restauration.

## 1.0.0 — 2026-10-29

### Ajouté

- Première version : édition per-touche, interface TKL AZERTY, effets EVision.
- Sélection simple, `Ctrl+clic` et rectangle.
- Écriture per-touche EVision V2, 10 profils.
- Aucun flash firmware.
