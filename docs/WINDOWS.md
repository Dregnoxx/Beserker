# Windows — métadonnées et signature

## Contenu de l'exécutable

Le binaire Windows embarque :

- une icône multi-résolutions (`resources/berserker-rgb.ico`) ;
- les informations de version `1.2.0.0` (produit, société, copyright) ;
- un manifeste (`resources/berserker-rgb.manifest`) : `asInvoker`,
  DPI `PerMonitorV2`, support Windows 7–11, chemins longs, page de code UTF-8.

Ces ressources sont compilées par `build.rs` avec la crate `winresource`,
uniquement lorsque la cible est Windows. La compilation nécessite le
compilateur de ressources `rc.exe` du Windows SDK.

## Signature Authenticode

Le binaire distribué n'est pas signé : aucun certificat de signature de code
n'est associé au projet. Windows SmartScreen peut donc afficher un
avertissement au premier lancement.

Pour signer avec un certificat existant :

```powershell
$exe = "dist\berserker-rgb.exe"
$signtool = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe"

# Certificat installé (empreinte SHA-1)
& $signtool sign /fd SHA256 /td SHA256 `
  /tr http://timestamp.digicert.com `
  /sha1 <empreinte_du_certificat> $exe

# Ou fichier PFX
& $signtool sign /fd SHA256 /td SHA256 `
  /tr http://timestamp.digicert.com `
  /f certificat.pfx /p <mot_de_passe> $exe

& $signtool verify /pa /v $exe
```

La réputation SmartScreen se construit avec le temps et le volume de
téléchargements ; elle n'est pas garantie par la seule signature.

## Vérifier les métadonnées

```powershell
(Get-Item "dist\berserker-rgb.exe").VersionInfo |
  Format-List FileVersion,ProductVersion,ProductName,CompanyName,OriginalFilename
```

Les ressources peuvent aussi être contrôlées avec `sigcheck`, `Resource Hacker`
ou l'Explorateur Windows (Propriétés → Détails).
