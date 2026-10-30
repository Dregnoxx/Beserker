# Changelog

Toutes les versions sont des versions alpha. Les binaires correspondants sont
publiés dans les releases GitHub.

## 1.2.0 — 2026-10-30

### Ajouté

- Page **Claviers** : détection HID en lecture seule des interfaces EVision
  (Berserker d'usine ou autre PID), bootloader Sonix ISP et QMK/VIA.
- Affichage VID/PID, usage, produit, chemin et puce Sonix ; copie des infos.
- Message de diagnostic adapté quand aucun contrôleur EVision TKL n'est
  disponible (clavier QMK, bootloader, autre modèle).

### Modifié

- L'éditeur RGB et le layout restent centrés sur le Berserker BYGG TKL.
- Aucun flash de firmware n'est implémenté ; SonixFlasherC reste un outil
  séparé, non inclus et non lancé.

## 1.1.7 — 2026-10-30

### Corrigé

- Fenêtre réduite ou masquée : un rendu d'arrière-plan indépendant des passes
  egui continue d'envoyer les frames directes ; l'animation des calques reste
  active.
- Fermeture : interception de la demande de fermeture dans l'interface et
  publication de la dernière image en couleurs per-touche avant l'arrêt, en
  plus du repli `on_exit`.
- Une image de secours reste publiée à l'activation et à la réduction pour
  couvrir l'indisponibilité du rendu d'arrière-plan.

### Ajouté

- Journal `berserker-rgb.log` : démarrage, effets, réduction, ouverture du
  worker, fermeture et erreurs HID.

## 1.1.6 — 2026-10-30

### Corrigé

- `--dump-config` lit toute la plage demandée par trames de 56 octets et
  retourne une erreur si la réponse est incomplète, au lieu de tronquer
  silencieusement à 56 octets.
- Analyse des couleurs CLI, des états de profil et des calques protégée
  contre les chaînes UTF-8 : plus de panique sur `a€bb` ou similaire.
- Fichiers `.sha256` Linux imbriqués des paquets v1.1.1 à v1.1.3 : chemins
  corrigés, `sha256sum -c` fonctionne depuis leur dossier.
- Paquet v1.1.4 : `Cargo.toml` et `Cargo.lock` passés en version 1.1.4.

### Ajouté

- Exe Windows : icône, informations de version `1.1.6.0` et manifeste
  embarqués (DPI PerMonitorV2, asInvoker, long paths, UTF-8).
- `docs/WINDOWS.md` : métadonnées et procédure de signature Authenticode.

### Modifié

- Prérequis de compilation : Rust 1.95+ (eframe/egui 0.37), au lieu de 1.85.

## 1.1.5 — 2026-10-30

### Corrigé

- Réduction ou fermeture de la fenêtre : la dernière image des calques est
  publiée en couleurs per-touche et reste affichée sur le clavier, au lieu de
  revenir immédiatement à l'éclairage de base.
- Une image de secours est publiée dès l'activation des calques pour couvrir
  l'arrêt des frames directes quand la fenêtre est réduite ou masquée.
- Le rendu animé et les calques enregistrés reprennent au lancement suivant.

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
