# Packaging Linux

Ce dossier contient les fichiers nécessaires à l'utilisation du binaire Linux
en dehors d'un paquet distribué.

- `90-berserker-rgb.rules` : règle udev pour l'accès à `/dev/hidraw`.
- `run-berserker-rgb.sh` : lanceur pour une extraction manuelle.

## Installation de la règle udev

```bash
sudo cp 90-berserker-rgb.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
# débrancher puis rebrancher le clavier
```

Sur un système sans systemd/logind, remplacer `TAG+="uaccess"` par
`MODE="0666"` si un accès global est acceptable.

## Lancement

```bash
./run-berserker-rgb.sh
```

Le binaire `berserker-rgb` doit se trouver à côté du lanceur.

Les binaires publiés sont fournis dans les releases GitHub (`AppImage` et
`tar.gz`). La compilation depuis les sources est décrite dans le README
principal.
