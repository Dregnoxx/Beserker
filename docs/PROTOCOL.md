# Protocole EVision V2 — notes techniques

Ce document décrit les échanges HID utilisés par l'application avec le clavier
Berserker BYGG `320F:5041`. Il couvre uniquement la configuration RGB et les
réglages système ; aucune commande de flash ou de bootloader n'est utilisée.

## Identité HID

| Champ | Valeur |
| --- | --- |
| Vendor ID | `0x320F` |
| Product ID | `0x5041` |
| Usage Page | `0xFF1C` |
| Usage | `0x0092` |
| Report ID | `4` |
| Trame | 64 octets |

Sous Linux, l'accès se fait par `/dev/hidraw` (une règle udev est fournie dans
`packaging/`). Sous Windows, l'application utilise hidapi.

## Trame

```text
octet 0      Report ID (0x04)
octets 1-2   Checksum, little-endian (somme des octets 3 à 63)
octet 3      Commande
octet 4      Taille de la charge utile
octets 5-6   Offset, little-endian
octet 7      Statut de la réponse (0 = OK, côté clavier)
octets 8-63  Données (56 octets maximum)
```

Le checksum est la somme non signée des octets de la trame à partir de
l'octet 3. Une réponse est acceptée si elle porte le même Report ID, la même
commande, le même offset et le même checksum. Le statut doit être nul.

## Commandes

| Valeur | Commande | Usage |
| --- | --- | --- |
| `0x01` | Begin configure | Ouvre une transaction d'écriture |
| `0x02` | End configure | Ferme la transaction |
| `0x03` | Read capabilities | Capacités (taille carte, macros) |
| `0x05` | Read config | Lecture d'un bloc de configuration |
| `0x06` | Write config | Écriture d'un bloc de configuration |
| `0x0A` | Read custom colors | Lecture d'un jeu per-touche |
| `0x0B` | Write custom colors | Écriture d'un jeu per-touche |
| `0x12` | Direct frame | Frame directe non persistante |
| `0x13` | Direct end | Fin du mode direct |
| `0x1B` | Read key map | Table index logique → position LED |

Les écritures de configuration sont encadrées par `Begin` puis `End`, puis
relues pour vérification. Une frame directe est envoyée en blocs successifs
avec `0x12`, puis terminée par `0x13`.

## Configuration

La configuration est organisée en blocs de profil de `0x40` octets. Pour un
profil `p`, le bloc principal commence à `p * 0x40 + 1` :

| Décalage | Champ |
| --- | --- |
| `+0` | Mode |
| `+1` | Luminosité |
| `+2` | Vitesse |
| `+3` | Direction (`0` gauche, `1` droite) |
| `+5..+7` | Couleur RGB |
| `+17` (`0x12`) | Variante / jeu de couleurs per-touche |

Les réglages système sont exposés à des offsets absolus :

| Offset | Champ | Valeurs |
| --- | --- | --- |
| `0x13` | Mode flèche | `0` / `1` |
| `0x14` | Anti-ghosting | `0` / `1` |
| `0x15` | Mode Jeu | `0` / `1` |
| `0x16` | Index fréquence USB | `0`=1000 Hz, `1`=500, `2`=250, `3`=125 |
| `0x19` | Index debounce | `0`=5 ms, `1`=10, `2`=15, `3`=20 |

## Modes

| ID | Mode | Vitesse |
| --- | --- | --- |
| `0x06` | Statique | non |
| `0x05` | Respiration | oui |
| `0x03` | Vortex | oui |
| `0x04` | Aura | oui |
| `0x08` | Vagues | oui |
| `0x0A` | Scintillements | oui |
| `0x0B` | Mosaïques | oui |
| `0x0C` | Cascade | oui |
| `0x0D` | Oscillation | oui |
| `0x0F` | Pluie | oui |
| `0x10` | Balayage | non |
| `0x12` | Radiation | non |
| `0x14` | Personnalisé | non |

La vitesse est un octet de `0` à `4`. Le firmware utilise l'échelle inversée
(`0` = le plus rapide, `4` = le plus lent) ; l'interface expose l'inverse
(`0` = lent, `4` = rapide).

## Couleurs per-touche

- 126 zones LED, 3 octets RGB par zone, soit 378 octets par jeu.
- 10 jeux de couleurs, indexés `0..9`, avec un pas de `512` octets dans la
  zone de stockage.
- Le firmware affiche le jeu 0. L'application recopie le profil actif vers le
  jeu 0 lorsqu'elle applique un profil.
- Écriture : `Begin`, blocs `0x0B` de 56 octets, `End`, puis relecture
  complète et comparaison octet par octet.

La table `0x1B` renvoie 126 octets : index logique de touche → position LED.

## Frames directes

Une frame directe contient les 378 octets de couleurs per-touche. Elle est
envoyée avec la commande `0x12`, offset `0`, en blocs de 56 octets, puis
terminée par `0x13`. Elle n'est pas enregistrée dans le firmware : à la fin ou
à la fermeture de l'application, le clavier revient à l'effet stocké.

## Persistance locale

Les profils ne sont pas stockés dans le clavier au-delà du jeu 0 ; ils sont
conservés par l'application :

```text
profil_N.bin     378 octets de couleurs per-touche (126 × RGB)
profil_N.state   état complet du profil (clé=valeur)
profil_N.layers  calques d'effets logiciels
actif.cfg        index du profil actif
```

Format `profil_N.layers` :

```text
version=1
layer=<id>;<vitesse>;<sens>;<mélange>;<actif>;<RRGGBB>;<masque hexadécimal>
```

Le masque fait 126 bits, encodé en 16 octets hexadécimaux (32 caractères).
