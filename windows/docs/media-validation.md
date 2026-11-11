# Validation Phase 2 — 4 novembre 2026

**Candidat multimédia à valider manuellement, pas une certification de compatibilité universelle.** Fonctionnement et protocole : [media.md](media.md).

## Sauvegarde et vérifications

- Avant les changements : 23 tests du socle réussis, fmt/Clippy propres, commit local distinct `f759778` (phase concentration officiellement validée), arbre propre vérifié. Les fichiers concentration.rs, preferences.rs et visibility.rs n'ont pas changé dans cette phase.
- Rust stable 1.99.1, cible Windows MSVC x64. Dépendances Microsoft WinRT versionnées dans Cargo.lock ; aucun logiciel système installé. Aucun réglage global ni publication GitHub.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`, `cargo test --workspace --all-targets --release --locked --offline` : réussis, **33 tests** (16 core, 3 UI, 14 Windows). Doctests : aucun présent. Build Release réussi.
- Tests nouveaux : sélection automatique/manuelle/disparition, métadonnées et images bornées, timeline/pause/fin/valeurs invalides, capacités des boutons dont toggle, file coalescée bornée, erreurs/annulation WinRT, interruption d'attente à l'arrêt, réception réentrante pendant un menu, timer uniquement quand utile, raster média à 100/125/150/200 % et 400 redessins sans hausse GDI/USER. Les tests précédents d'animations, Échap, focus, préférences et tray restent actifs.
- Rendu de test avec/sans image et sans session inspecté dans `artifacts/media-previews/`. Il contient uniquement des données synthétiques. Ce contrôle ne constitue pas une capture du lecteur de l'utilisateur.
- Banc concentration : **23/23**, dont plein écran du banc, retour au mode antérieur, raccourci et persistance. 100 allers-retours : handles 313→313, GDI 16→16, USER 23→23, engagement privé 3 874 816→3 874 816 octets. Preuve : `artifacts/concentration-20261104-210141/report.json`.
- Banc flottant : **18/18**, dont maximisation, absence d'AppBar, plein écran/retour, masquage prioritaire, position conservée, redémarrage, ancien argument --integrated inactif, arrêt normal. 30 cycles : handles 317→307, GDI 17→17, USER 24→21, engagement privé 4 218 880→4 141 056 octets ; zone de travail inchangée `(0,0)–(1920,1020)`. Preuve : `artifacts/floating-20261104-210157/report.json`.

## Intégration réelle GSMTC

Le diagnostic en lecture seule a détecté la session Opera GX existante, en lecture, avec titre présent, miniature décodée et timeline valide. Pause disponible ; précédent/suivant non disponibles dans cette session. Aucune commande n'a été adressée à Opera. Cela confirme cette session et ce système, pas tous les sites ni tous les lecteurs.

Le banc `media_fixture` publie un SMTC silencieux (aucun fichier musical, aucun son, miniature originale en mémoire). Le Worker réel reçoit ses événements via Windows. Résultats : découverte, choix manuel, timeline, décodage, lecture/pause/suivant, 30 changements rapides de titre, rejet d'une ancienne génération, deux fournisseurs distincts musique/vidéo, conservation du choix manuel, disparition et libération du choix, rejet d'une commande adressée à une session supprimée sans redirection vers un autre lecteur. Arrêt du worker observé en 1 ms au dernier passage (2 ms au précédent). Le banc ne commande que l'identité de son propre fournisseur.

**Défauts trouvés puis corrigés pendant la campagne :**

1. Les wrappers COM changent lors de GetSessions : la première approche fondée sur leur égalité perdait le choix manuel. Correction par source AUMID unique, renouvellement des abonnements et générations. Rejeu réel à deux processus réussi. Les AUMID dupliqués restent ambigus et ne sont pas associés par supposition.
2. L'annulation d'une pochette lors d'une sélection pouvait la marquer chargée à tort et empêcher sa reprise. Les annulations dues à une génération plus récente réarment désormais le chargement. Le banc qui attend l'image après sélection passe.

Les premières exécutions du banc ont donc échoué ; elles ne sont pas présentées comme réussies. Les essais finaux sont conservés séparément dans `artifacts/media-fixture-final.log` et les traces de diagnostic dans `artifacts/media-fixture-debug.log`.

## Performances

Machine : Windows 11 Famille build 26300, i5-12450HX / 12 processeurs logiques, environ 23,7 Gio de RAM, Intel UHD + Arc A530M, écran actif 1920 × 1080 / 144 Hz / DPI 125 %. Autres applications conservées. Collecte du seul processus Release, pas du lecteur ni du broker Windows. Certaines compilations/tests indépendants ont tourné en parallèle : le bruit système n'est pas contrôlé. Les parcours déplié et suivants ont été mesurés après les corrections.

Deux collectes préliminaires de cinq minutes ont été autorisées et réalisées sur le binaire `AA127DE7…EDDCD`, avant les corrections de sélection/pochette :

| État, session Opera existante | Durée / relevés conformes | Delta CPU | Résident moyen | Engagement privé moyen | Handles | GDI / USER |
|---|---:|---:|---:|---:|---:|---:|
| Compact | 300,468 s / 296 sur 296 | 0,03125 s | 24,86 Mio | 3,28 Mio | 261–281 | 9 / 11–13 |
| Concentration | 300,503 s / 296 sur 296 | 0 s détectée | 24,99 Mio | 3,27 Mio | 261–265 | 9 / 11 |

Preuves : `artifacts/validation-20261104-203706/` et `204343/` (même préfixe `validation-20261104-`). Ces collectes ne sont pas présentées comme cinq minutes sur le binaire final.

**Release corrigée : 396 800 octets**, SHA-256 `7DF368743AA0BBC9FD4C2E670FEB291A5917A4464ACFDC50DCD0DD96D7672D01`. Résultats, session Opera conservée :

| État | Durée / conformité | Delta CPU | CPU machine moyen | Résident moyen | Engagement privé moyen | Handles | GDI / USER |
|---|---:|---:|---:|---:|---:|---:|---:|
| Déplié, progression disponible | 120,693 s / 119 sur 119 | 0,703125 s | 0,04855 % | 29,67 Mio | 4,23 Mio | 297–316 | 17 / 21–25 |
| Compact après ouverture | 60,982 s / 60 sur 60 | 0 s détectée | 0 % détecté | 28,99 Mio | 3,81 Mio | 299 | 17 / 19 |
| Concentration après ouverture | 60,805 s / 60 sur 60 | 0 s détectée | 0 % détecté | 28,95 Mio | 3,74 Mio | 293–299 | 17 / 19 |
| Masqué | 60,928 s / 60 sur 60, aucun visible | 0 s détectée | 0 % détecté | 29,10 Mio | 3,77 Mio | 293–295 | 17 / 19 |

Preuves : `205412/`, `205656/`, `205812/` sous `artifacts/validation-20261104-…`. Les intervalles courts du binaire corrigé ne remplacent pas une endurance de cinq minutes ou d'une heure. La progression de CPU est normalisée par 12 processeurs ; le déplié correspond à environ 0,583 % d'un cœur. WorkingSet64 est le résident total, pas la mémoire de travail privée. Par exemple le working set privé ponctuel final déplié vaut 2 646 016 octets, distinct des 4,23 Mio d'engagement privé moyen.

Référence `114a8f3` : compact frais environ 16,16 Mio résidents et 2,30 Mio d'engagement privé, GDI/USER 9/10. L'ajout WinRT et le décodage ont un coût visible : le compact préliminaire monte d'environ 8,70 Mio résidents et 0,98 Mio privés. Les relevés finaux après ouverture retiennent davantage de caches. Cette différence n'est pas une mesure isolée du coût du module : charge, médias et état des caches diffèrent. Aucune croissance monotone démontrée au repos ; ce n'est pas une preuve absolue d'absence de fuite.

Attribution GPU : listes de compteurs vides, donc **indisponible**, pas « GPU nul ». DWM et les services Windows partagés ne sont pas inclus dans le CPU/RAM du processus. Latence physique et cadence réelle des animations non instrumentées ici.

Preuve du masquage : `artifacts/validation-20261104-205927/`. Fermeture normale après collecte : 98 ms entre la demande et la fin observée, zéro processus résiduel ; journal `WM_QUIT received`, `run result=Ok(())`. L'objet Process réacquis ne fournit pas de code de sortie (null) : aucun code numérique n'est donc affirmé pour cette mesure. Preuves : `artifacts/media-final-shutdown.json`, `artifacts/media-runtime/final-diagnostics.txt`. Les bancs suivants se sont également arrêtés proprement ; inventaire final du projet vide. Pas de terminaison forcée.

## Limites restantes

- Validation utilisateur du multimédia encore requise ; les six réussites concentration antérieures ne la remplacent pas.
- Chrome, Edge, Firefox, Spotify, lecteurs de films/séries/animés et sites HTML5 divers : non certifiés par ces essais. Un lecteur qui n'expose rien à GSMTC reste invisible.
- Mesures sans aucun média et musique/vidéo réelle séparées non réalisées : Opera est resté actif, sans interruption forcée. Les fournisseurs synthétiques valident les mécanismes, pas la compatibilité d'un site ni une charge vidéo réelle.
- DPI dynamique, écrans multiples physiques, veille/verrouillage, jeux exclusifs, plantage forcé et vrai redémarrage d'Explorer non testés. Pas de modification de résolution ni d'Explorer.
- Métadonnées/timeline peuvent être incomplètes ou retardées ; sources AUMID parfois techniques, doublons de source ambigus, pas de fournisseur UI Automation pour le panneau raster.

La phase 2 reste dans l'arbre de travail en attente de validation manuelle, sans commit la déclarant stable et sans publication. Le retour à la phase concentration validée est disponible via `f759778`, en conservant d'abord les travaux multimédias si un retour est décidé.
