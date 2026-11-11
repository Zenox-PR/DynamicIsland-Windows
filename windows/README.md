# Application native — Dynamic Island Windows

[Accueil du dépôt](../README.md) · [Site de démonstration](../site/README.md)

Prototype Windows 11 x64 en Rust. Aucun WebView ou serveur embarqué. Rendu GDI/BGRA, fenêtre flottante Win32 et sessions multimédias Windows GSMTC.

## Compiler et lancer

Prérequis : Windows 11 x64, Rust stable compatible avec `rust-version = 1.99`, outils MSVC C++ et SDK Windows. Aucun installateur automatique fourni.

Depuis la racine du dépôt, dans PowerShell :

```powershell
cd windows
cargo build --release --locked
.\target\release\dynamic-island.exe
```

Les dépendances Cargo peuvent être téléchargées au premier build. Utiliser `--offline` lorsque le cache est déjà disponible. L'application peut être quittée normalement via son icône système.

## Interactions

- Clic : ouvrir/replier ; Tab/Maj+Tab puis Entrée/Espace : navigation ; Échap : replier ou annuler le déplacement.
- Ctrl + glisser ou menu « Déplacer l'encoche » : positionnement. Le menu système permet de masquer, restaurer et recentrer.
- Concentration : ligne de 96 × 4 DIP laissant passer les clics. La tray permet de revenir au mode normal.
- Multimédia : seules les sessions exposées par Windows sont disponibles. Pas d'inspection d'onglet ou de connexion à un service musical.
- Minuteurs : 1–240 minutes, Pomodoro 25/5/15 par défaut ; chaque nouvelle phase attend une action explicite.
- Raccourcis : catalogue, favoris et ordre ; un lancement exige une action explicite.

Le compact mesure 172 × 32 DIP, le panneau 384 × 168 DIP ; ouverture 220 ms, fermeture 180 ms. Préférences propres à l'application dans `%LOCALAPPDATA%\DynamicIsland`. Pas d'autostart ni de modification de réglage global.

## Architecture et vérification

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

Les tests de fenêtres nécessitent un bureau Windows interactif. Les bancs sous `scripts/` ne remplacent pas des essais physiques. [Architecture](docs/architecture.md), [multimédia](docs/media.md), [minuteurs](docs/timers.md), [raccourcis](docs/launcher.md), [socle système expérimental](docs/system-information.md).

Le lanceur attend encore des essais physiques détaillés. Le module système n'a pas d'interface active. Les rapports historiques décrivent leur révision et ne doivent pas être réattribués à un autre binaire.

## Licence

[PolyForm Noncommercial 1.0.0](../LICENSE), pour le code original. Aucune autorisation commerciale accordée. Dépendances sous leurs licences respectives.
