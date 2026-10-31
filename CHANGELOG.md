# Changelog

Toutes les versions sont des versions alpha. Les binaires correspondants sont
publiés dans les releases GitHub.

## 1.3.5 — 2026-10-31

### Corrigé

- Détection Linux QMK/VIA : le PID 320F:5042 produit désormais une fiche QMK
  même si le descripteur HID n'expose pas FF60:0061. L'ouverture sonde tous
  les nœuds hidraw du clavier et retient celui qui répond au protocole VIA.
- Panneau Éclairage dédié quand le clavier QMK/VIA est détecté mais que
  l'interface hidraw ne peut pas être ouverte (permissions, udev).
- Règle udev limitée explicitement à `KERNEL=="hidraw*"` et VID 320F.

### Ajouté

- `--devices` : diagnostic Linux des interfaces et nœuds hidraw (HID_ID,
  HID_NAME, usages EVision/QMK, résultat d'ouverture).

## 1.3.4 — 2026-10-31

### Corrigé

- Sauvegarde EEPROM QMK/VIA : la commande était envoyée sur le canal 0, donc
  refusée. Elle part maintenant sur le canal RGB Matrix (3), avec contrôle de
  la réponse `09 03` du firmware.
- L'application applique puis sauvegarde automatiquement l'état QMK après
  **Appliquer l'effet** ; la relecture de contrôle met à jour l'interface et
  signale un effet refusé.
- Les IDs d'effets 40-43 du firmware flashé sont visibles dans la liste
  (extensions sans nom standard) au lieu d'être absents.
- Les pages Effets et Autres réglages sont explicitement indisponibles en
  QMK/VIA au lieu d'afficher des commandes EVision inopérantes.
- Statut « Clavier QMK/VIA connecté » après l'ouverture automatique de VIA.

### Ajouté

- Commandes de diagnostic `--qmk-state`, `--qmk-solid`, `--qmk-set` et
  `--qmk-save` pour tester l'éclairage RGB Matrix sans interface graphique.

### Compatibilité

- Firmware EVision 320F:5041 inchangé : per-key, profils, effets PC et
  réglages matériels restent disponibles.
- QMK/VIA 320F:5042 : éclairage global uniquement ; le per-key n'est pas
  exposé par le firmware.

## 1.3.3 — 2026-10-30

### Ajouté

- Éclairage QMK/VIA intégré à la page **Éclairage** : liste d'effets avec ID
  brut, luminosité, vitesse, palette, saisie RGB, roue chromatique,
  application et sauvegarde EEPROM.
- Connexion automatique de l'interface VIA au démarrage.
- Bouton **Couleur unie** et bascule automatique vers l'effet 1 lorsqu'une
  couleur est choisie sur un effet arc-en-ciel.
- Aperçu QMK animé par type d'effet : les cycles et balises arc-en-ciel ne
  sont plus affichés comme une couleur fixe.

### Modifié

- Page **Claviers** QMK : diagnostic, connexion/déconnexion et accès direct à
  la page Éclairage ; les curseurs dupliqués sont retirés.
- Page Éclairage défilante pour les fenêtres courtes.
- Profils EVision et effets superposables PC désactivés en mode QMK/VIA.
- Version de fichier Windows 1.3.3.0.

### Corrigé

- Une couleur fixe n'est plus appliquée silencieusement sur un effet qui
  ignore la teinte ; une saturation nulle correspond au blanc et le noir
  sélectionne « Désactivé ».

## 1.3.2 — 2026-10-30

### Corrigé

- Règle udev Linux étendue à tout le VID `320F` : EVision 320F:5041 et
  QMK/VIA 320F:5042. Corrige `Permission denied` sur `/dev/hidraw7`.
- Message QMK/VIA explicite si l'accès hidraw est refusé.
- Règle fournie à la racine du paquet et dans `linux/`.

## 1.3.1 — 2026-10-30

### Corrigé

- Clavier QMK/VIA détecté sans EVision : statut informatif, sans erreur
  « 320F:5041 introuvable » en rouge.
- Badge « QMK/VIA détecté » et bouton « Éclairage QMK » vers Claviers.

## 1.3.0 — 2026-10-30

### Ajouté

- Client VIA minimal pour l'éclairage RGB Matrix des claviers QMK/VIA
  reflashés : luminosité, effet, vitesse, teinte, saturation, lecture et
  sauvegarde EEPROM.
- Effets QMK standard proposés avec ID brut pour les variantes de firmware.
- Dédoublonnage de la fiche QMK : la collection clavier standard n'apparaît
  plus comme un second modèle EVision.

### Limites

- Le per-key n'est pas exposé par le firmware QMK/VIA ; l'édition per-touche
  reste réservée au firmware EVision d'usine.
- Aucun flash n'est effectué ; SonixFlasherC reste un outil séparé.

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
