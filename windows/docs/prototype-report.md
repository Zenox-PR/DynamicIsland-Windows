# Rapport de vérification du prototype — 3 novembre 2026

Rapport historique de la première vérification. Ensuite, l'utilisateur a confirmé manuellement l'affichage compact, l'ouverture au clic, le repli au second clic et les animations, et signalé l'échec d'Échap. Voir [le correctif de focus et ses nouvelles vérifications](escape-focus-fix.md). Les anciens chiffres et limites ci-dessous concernent les binaires identifiés dans ce rapport.

## Résultat

Le socle natif compile et les tests automatisés passent. La validation des interactions réelles reste incomplète : l'outil graphique n'expose pas l'encoche comme fenêtre ciblable. Aucun test non exécuté n'est assimilé à une réussite. Aucune nouvelle fonctionnalité produit, installation, configuration globale ou publication GitHub pendant cette reprise.

## Nettoyage initial et méthode

Un ancien dynamic-island.exe, PID 24544, exécuté depuis le dossier Release du projet était actif. Le diagnostic du projet lui a envoyé WM_CLOSE ; deux secondes plus tard aucun processus du projet n'était présent. Aucun autre logiciel n'a été fermé.

La nouvelle exécution Release est le PID 30228, accompagnée du seul banc desktop_fixture.exe, PID 8720. Le contrôle des processus utilise le chemin absolu sous ce dépôt, pas seulement leur nom. L'outil Computer Use a activé le banc autorisé, sans agir sur les autres applications. Une entrée utilisateur détectée a bloqué l'envoi de F11 ; les entrées automatiques ont été suspendues. L'utilisateur a indiqué ne pas avoir testé l'encoche : aucun résultat manuel positif n'est ajouté.

## Configuration

- Windows 11 Famille, version 10.0.26300, build 26300.
- Intel Core i5-12450HX, 12 processeurs logiques ; RAM détectée 25 463 480 320 octets (23,71 Gio).
- Un moniteur actif, 1920 × 1080, fréquence annoncée par WMI 144 Hz.
- DPI de la fenêtre : 120, soit 125 %. Encoche repliée mesurée en pixels physiques : rectangle (850,10)–(1070,53), soit 220 × 43 pixels. Cela correspond aux dimensions DIP arrondies, centrées et à la marge attendues.
- Intel UHD Graphics pilote l'écran ; Intel Arc A530M également détecté. Son activité effective n'a pas été établie.
- Rust/cargo 1.99.1, cible x86_64-pc-windows-msvc. windows-sys 0.61.2, windows-link 0.2.1. Aucun changement de toolchain ou de SDK.
- Applications de l'utilisateur conservées, sans baseline système isolée. Alimentation et profils énergétiques non relevés. Ne pas extrapoler ces résultats à d'autres machines.

## Contrôles du code

| Contrôle | Résultat |
|---|---|
| cargo fmt --all, puis --check | Formatage appliqué ; contrôle réussi |
| cargo clippy --workspace --all-targets --offline -- -D warnings | Réussi, aucun avertissement accepté ou masqué |
| cargo test --workspace --all-targets --offline | 6 tests réussis, aucun échec |
| Tests de documentation du workspace | Réussis, aucun exemple exécutable présent |
| Compilation application et exemples Release | Réussie avant exécution |
| Test natif de ressources en Release (--lib) | Réussi |
| Recompilation finale cargo build --workspace --all-targets --release --locked --offline | Réussie après fermeture des exécutables, sans avertissement |

Les quatre tests du cœur vérifient arrêt des transitions, inversion continue, animations réduites et géométrie/DPI. Le test de pixels vérifie alpha prémultiplié et coins transparents. Le sixième test sollicite les véritables DIB/DC/polices du moteur Windows.

Une invocation de cargo test en Release sans --lib a tenté de reconstruire desktop_fixture.exe pendant son exécution et a échoué avec LNK1104. Cause : verrouillage Windows de l'EXE actif, pas un défaut du code source. Le test ciblé --lib a ensuite réussi. Les processus doivent être fermés avant une compilation de leurs exécutables. CARGO_INCREMENTAL=0 a été utilisé localement pour éviter les problèmes de cache de l'environnement isolé, sans réduire le niveau de contrôle des avertissements.

## Comportements réellement observés

| Scénario | Résultat et limite |
|---|---|
| Démarrage | Processus et HWND créés ; journal sans erreur d'initialisation |
| Position/DPI actuel | Rectangle physique cohérent à 125 %, sur l'unique écran principal |
| Masquage plein écran | IsWindowVisible=0 lorsque la fenêtre active sans légende couvre 1920 × 1080 |
| Retour d'affichage | IsWindowVisible=1 après activation du banc de test en mode fenêtre |
| Focus à la réapparition | La fenêtre active reste celle du banc, pas l'encoche |
| Retour au plein écran déjà présent | Encoche de nouveau masquée, observée en lecture seule |
| Instance unique | Deuxième processus terminé en moins de 3 s, code 0, première instance conservée ; vérifié aussi sur le binaire final |
| Fermeture native WM_CLOSE | Terminaison en moins de 2 s ; binaire final code 0 et journal run result=Ok(()) |
| Nettoyage final | Banc fermé par CloseMainWindow, aucun processus restant sous le chemin du projet |
| F11 dans le banc | Non exécuté : outil bloqué par détection d'entrée utilisateur, puis manipulations suspendues |
| Ouverture au clic / fermeture extérieure | Non validées en entrée réelle : fenêtre outil de l'encoche absente des cibles proposées |
| Clics dans les coins transparents | Pixels validés ; transmission réelle non validée |
| Clavier et menu tray Quitter | Implémentés, parcours réel non validé |
| Fluidité affichée / absence de vol de focus au clic | Non validées |
| Autres DPI/résolutions, plusieurs écrans | Non testés : un seul moniteur actif, aucune modification globale Windows effectuée |
| Veille, verrouillage, jeux plein écran exclusif, redémarrage Explorer | Non testés |

Un changement de fenêtre active n'est pas une validation exhaustive des transitions plein écran d'un même programme. Aucune application tierce n'a été minimisée ou fermée pour ces essais.

## Cycles et rendu

Le test natif effectue un cycle d'échauffement puis 100 cycles replié→déplié→replié en mémoire, 33 surfaces par cycle, à l'échelle 125 %. Il alloue une DIB et un DC, dessine la forme et les textes via le code de production, puis libère les ressources. Les ressources sont relevées après 25, 50, 75 et 100 cycles.

- GDI : baseline 1, puis 1 à chaque contrôle.
- USER : baseline 0, puis 0 à chaque contrôle ; ce test ne crée pas de fenêtre.
- 100 cycles en Release : 2,233 s pour 3 300 générations de surface.
- Allocation + dessin + destruction : médiane 0,444 ms, p95 1,624 ms, maximum 5,254 ms.

Ce résultat ne mesure ni UpdateLayeredWindow, ni l'attente du timer, ni DWM, ni la latence événement→image. Il ne prouve pas 60 images/s et ne remplace pas 100 cycles réels de fenêtre avec hooks, tray et focus. Aucune fuite d'objets GDI/USER n'a été détectée dans le parcours testé ; les autres parcours restent à éprouver. L'endurance d'une heure n'a pas été réalisée.

## Mesure de cinq minutes

Collecte externe en lecture seule par scripts/measure-idle.ps1 : 60 secondes de stabilisation puis au moins 300 secondes, relevés espacés d'environ une seconde. Le CPU est normalisé machine : 100 × delta secondes CPU / (temps réel × 12). Les pics correspondent à ces intervalles et non à un pic instantané matériel.

L'encoche était masquée par une fenêtre plein écran aux contrôles de début et intermédiaires. Il n'y a pas de journal continu de visibilité ; cette session ne certifie donc pas cinq minutes de panneau visible et immobile. Les autres applications sont restées actives. Des tests de compilation/dessin ont aussi tourné dans un processus distinct au début de la collecte ; les compteurs publiés portent seulement sur le prototype, et le poste n'était pas isolé.

Les compteurs GPU par PID sont vides aux relevés initial et final. Cela signifie absence d'attribution disponible, pas preuve de 0 % GPU. Le coût DWM et le réveil éventuel de l'Arc ne sont pas mesurés. WorkingSet64 désigne le working set total ; WorkingSetPrivate provient des compteurs PerfProc séparés au début et à la fin. PrivateBytes est l'engagement privé.

Durée effective : **300,659 s**, **296 échantillons**. Encoche également masquée au contrôle final.

| Mesure | Résultat |
|---|---|
| Temps CPU cumulé pendant la fenêtre | 0,0625 s |
| CPU machine moyen / pic sur un intervalle | **0,001732 % / 0,129339 %** |
| Working set total moyen | **17,582 Mio** |
| Working set total minimum / maximum | 17,215 / 17,699 Mio |
| Working set privé initial / final | **1,828 / 1,832 Mio** (+4 Kio, deux relevés) |
| Engagement privé moyen / maximum | **2,649 / 2,738 Mio** |
| Engagement privé initial / final | 2,582 / 2,582 Mio |
| Handles initial / minimum / maximum / final | **226 / 224 / 232 / 226** |
| Threads initial / maximum / final | 4 / 9 / 4 |
| Objets GDI | **6** à chaque échantillon |
| Objets USER initial / minimum / maximum / final | 26 / 10 / 26 / 10 |
| GPU | Attribution indisponible ; DWM non mesuré |

La cible CPU moyenne ≤ 0,1 % est respectée **dans ce scénario de masquage**. Le working set privé reste bien inférieur au plafond mémoire envisagé de 60 Mio. Les handles et l'engagement privé reviennent au niveau initial ; aucune croissance continue n'apparaît sur cette période. Cela ne démontre pas l'absence de fuite pendant des interactions répétées ou une longue session. Les threads internes Windows varient, malgré un seul thread de messages créé explicitement par le code.

Preuves locales : `artifacts/validation-20261103-205625/` contient environment.json, idle.csv, summary.json, les compteurs mémoire/GPU avant/après, final-build.txt, final-startup.log et final-lifecycle.json. Les données sont ignorées par Git, jamais envoyées à un service externe.

Traçabilité : la collecte de cinq minutes porte sur le binaire SHA-256 `1DF1C4E44412DBDDE662E4B3F7246D9E3AEAD0665427DF4B571C0DE636BC85AB`. L'ajout du test sous cfg(test), le formatage et la recompilation finale ont produit `B6170A79435CB98D5B044F8E3EB39932C63710078CEFDE56A434CE8BD8A6A898`. Aucun comportement de production n'a été modifié pendant cette reprise. Le dernier binaire a été contrôlé pour le démarrage, l'instance unique et l'arrêt, mais sa collecte de cinq minutes n'a pas été répétée : les deux artefacts sont explicitement distingués.

## Arrêt final et fichiers modifiés

Le PID mesuré 30228 a quitté après WM_CLOSE avec journal `run result=Ok(())`. Le banc PID 8720 a quitté dans les deux secondes suivant CloseMainWindow. Après recompilation, le PID 26544 a démarré ; le second PID 26080 a quitté avec code 0, mutex déjà existant. Le PID 26544 a ensuite quitté avec code 0 dans les deux secondes suivant WM_CLOSE. Le dernier inventaire par chemin absolu retourne **0 processus du projet**. Le parcours de menu « Quitter » reste distinct de cette fermeture native et n'est pas déclaré testé.

Cette reprise a ajouté le test de ressources dans `crates/island-windows/src/surface.rs`, enrichi le diagnostic en lecture seule `examples/inspect.rs` et ajouté `scripts/measure-idle.ps1`. Les autres retouches Rust sont du formatage. README.md, AGENTS.md et les documents d'architecture, installation, protocole et présent rapport reflètent désormais l'état réel. Aucun code de fonctionnalité avancée n'a été ajouté. Aucun remote Git n'est configuré ; rien n'est publié.

## Points à corriger ou compléter avant validation complète

1. Vérifier humainement, ou avec un outil capable de cibler cette fenêtre outil, le clic d'ouverture, le clic extérieur livré au destinataire, les coins transparents, le clavier, le tray et Quitter.
2. Effectuer des cycles réels avec contrôle mémoire/handles avant et après ; la stabilité GDI du moteur seul ne couvre pas les hooks et le menu.
3. Mesurer le panneau visible au repos et la cadence réelle pendant les animations. Le timer de 15 ms n'offre pas une synchronisation à 60 Hz, encore moins à 144 Hz.
4. Contrôler les retours Win32 encore ignorés (SelectObject, menu, attribution DPI, restauration tray) et définir le traitement des échecs rares. Aucun avertissement de compilation ne signale automatiquement ces défauts.
5. Valider les limitations de l'heuristique plein écran, les autres DPI/écrans et l'accessibilité du panneau dessiné. Pas de fournisseur UI Automation actuel.
6. Le canal stable n'est pas un verrouillage exact de Rust ; version vérifiée 1.99.1, sans mise à jour ni installation pendant ces essais.

Aucun défaut fonctionnel d'entrée réelle n'est déclaré reproduit : plusieurs parcours n'ont simplement pas pu être exécutés. Ces limites empêchent de déclarer l'étape B entièrement validée.
