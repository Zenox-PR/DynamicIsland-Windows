# Noir satiné — réalisation et vérification

3 novembre 2026. Refonte autorisée par l'utilisateur. Sauvegarde du prototype fonctionnel **avant toute modification** : commit local `af15520` (`chore: sauvegarde du prototype avant Noir satine`). Aucun remote ajouté, aucune publication, installation, modification globale Windows ou fonctionnalité avancée.

## Réalisation

- Encoche 172 × 32 DIP, rayon 16 ; panneau 384 × 168 DIP, rayon 26. Ancrage centré sur l'écran principal, à 8 DIP du haut.
- Fond #0B0C0F, contour #2A2C32, texte #F3F4F6 et secondaire #A5A7AF. Poignée discrète et contour de focus conservé.
- Marges 24 DIP ; Segoe UI 20 semi-gras pour le titre, 14 pour la description, 12 pour l'aide, 11 pour Échap. En-tête, corps et pied séparé : structure prête à accueillir ultérieurement du contenu, sans modules ni faux boutons. La totalité du panneau conserve son action de repli au clic.
- Ouverture 220 ms, fermeture 180 ms, courbe smoothstep sans rebond ni dépassement. Inversion depuis la position courante, durée proportionnelle au trajet restant. Les requêtes répétées ne prolongent pas l'animation. Continuité de position, pas de conservation de la vitesse à l'inversion.
- Texte à taille constante, fondu sur le fond en fin d'ouverture (progression géométrique 82–100 %). Les animations réduites Windows gardent une transition immédiate.
- Pixels prémultipliés et transparence extérieure conservés. Aucun flou, ombre large, Acrylic/Mica, WebView, D3D explicite ou dépendance supplémentaire.

La logique de focus, d'Échap différé pendant une réentrance, de clic extérieur, d'Alt+Tab et de plein écran n'a pas été modifiée. L'ouverture explicite demande le focus ; le survol ne le demande pas. Aucun hook clavier global. La vérification humaine de ces parcours sur cette nouvelle version reste à faire.

Fichiers modifiés : `crates/island-core/src/lib.rs` (durées/tests), `crates/island-ui/src/lib.rs` (style/disposition/raster/tests), `crates/island-windows/src/lib.rs` (intégration/cache), `crates/island-windows/src/surface.rs` (typographie GDI/tests), `crates/island-windows/examples/desktop_fixture.rs` (préparation des états pour les mesures), `scripts/measure-idle.ps1` (vérification d'état), `AGENTS.md`, `README.md`, `docs/architecture.md`, `docs/visual-proposal.md`. Le présent rapport est créé. Cargo.toml, Cargo.lock et les dépendances sont inchangés.

## Organisation et ressources

`island-core` conserve les transitions et la fonction de distance ; dimensions, palette, dispositions et dessin pur sont regroupés dans `island-ui`. `island-windows` possède le cache des quatre HFONT/chaînes UTF-16 par DPI et le dessin natif GDI. Les quatre rectangles de texte sont un calcul arithmétique léger par image, sans moteur de disposition ni allocation ; ils ne sont pas mis en cache. La DIB reste recréée lorsque la taille change.

Les polices ne sont plus créées/détruites à chaque image. Elles sont désélectionnées après chaque dessin et libérées à l'arrêt ou au changement de DPI. Les échecs de création/sélection de ressources et de DrawText remontent au traitement d'erreur. GdiFlush précède l'accès CPU à l'alpha du texte ; sa restauration ne parcourt que les rectangles intérieurs, sans second calcul de distance sur tout le panneau.

Le thread attend dans GetMessageW au repos ; le timer de 15 ms n'existe que pendant le mouvement. Les événements Windows peuvent déclencher une réévaluation, sans interrogation périodique du système. DWM reste impliqué dans la composition ; l'absence de D3D explicite ne prouve pas une activité GPU nulle.

## Tests exécutés

- Cargo fmt appliqué puis contrôle `--check` réussi ; Clippy workspace/all-targets avec `-D warnings` réussi.
- 12 tests du workspace réussis en Debug/all-targets et en Release : 8 cœur, 1 raster pur, 3 Windows. Les deux exemples compilés n'ont pas de tests propres. Tests de documentation réussis, aucun exemple de documentation exécutable présent.
- Les tests couvrent l'inversion précoce, les cibles répétées, les durées et bornes, l'arrêt et les animations réduites ; les dimensions, zones de texte et alpha à 100, 125, 150 et 200 % ; la production GDI et sa libération ; le focus natif et Échap reçu pendant un emprunt réentrant de l'état.
- Les tests clavier emploient le véritable gestionnaire Win32 avec fenêtres cachées et messages locaux. Ils ne remplacent pas des touches physiques, Alt+Tab réel ou les restrictions d'activation du bureau.
- Aperçus du rendu GDI réel inspectés, notamment compact à 125 % et déplié à 125/200 % : texte lisible, proportions et coins cohérents, aucun débordement observé. Ce sont des DIB exportées sur fond clair, pas des captures prouvant la composition DWM.
- Compilation Release de l'application et des exemples réussie, dépendances verrouillées/offline. Rust stable MSVC x64 1.99.1, windows-sys 0.61.2 inchangé.

Preuves : `artifacts/satin-release-tests.txt`, `artifacts/satin-debug-tests.txt`, `artifacts/satin-preview/`. Les artefacts restent locaux et ignorés par Git. Git signale sa conversion habituelle LF→CRLF sur les fichiers texte ; `git diff --check` réussit. Ce message Git est distinct d'un avertissement Rust et aucune configuration Git globale n'a été modifiée.

## Cycles de ressources et coût de dessin

Après un cycle d'échauffement, 100 cycles de rasterisation à 125 %, soit 3 300 allocations/dessins/destructions de DIB/DC : **6,033 s**. Objets GDI **5 → 5**, USER **0 → 0**, aux contrôles 25/50/75/100 ; les quatre polices du cache sont volontairement vivantes pendant ce test.

Temps par image (allocation + dessin + destruction) : médiane **1,528 ms**, p95 **4,402 ms**, maximum **8,616 ms**. Ce résultat exclut UpdateLayeredWindow, la file de messages, le timer et DWM : ni cadence réelle à 60/144 Hz ni latence d'entrée à l'écran établies.

Le rapport initial relevait un p95 de 1,624 ms avec un dessin plus simple. Le présent coût est supérieur ; les sessions n'étant pas isolées ni simultanées, ce n'est pas une comparaison contrôlée. Il reste inférieur à 16,7 ms pour le seul dessin dans ce test. Aucune garantie de fluidité complète n'est déduite de ce chiffre. Les 100 cycles concernent les ressources de rendu, pas 100 interactions réelles de la fenêtre avec hooks/tray.

## Mesures visibles au repos

Méthode : binaire Release, 60 s de stabilisation puis 300 s, échantillonnage externe chaque seconde. Vérification du chemin/PID, de la visibilité et des dimensions à chaque point. Le banc propre au projet demande le premier plan en mode fenêtre et prépare l'état par un message local ; aucune application tierce n'est fermée ou minimisée. Les restrictions Windows d'activation ne sont pas contournées.

Configuration : Windows 11 Famille 10.0.26300, Intel i5-12450HX, 12 processeurs logiques, 23,71 Gio de RAM, un écran 1920 × 1080 annoncé à 144 Hz, DPI 120 (125 %). Intel UHD Graphics et Arc A530M détectés. Applications de fond conservées, aucune baseline système isolée ; alimentation/profil énergétique non relevés.

CPU machine = 100 × temps CPU processus / (durée réelle × 12). Working set total, working set privé et engagement privé sont distingués. Les compteurs GPU ne couvrent pas l'attribution DWM.

### Compacte visible — collecte complète

PID 33996, 300,899 s, 296 échantillons : **296/296 visibles en état compact**, 215 × 40 pixels, DPI 120. Aucun test de compilation/rasterisation lourd exécuté pendant cette fenêtre ; les applications de fond sont restées actives.

| Mesure | Résultat |
|---|---|
| Temps CPU consommé | 0,1875 s |
| CPU machine moyen / pic par intervalle | **0,005193 % / 0,254456 %** |
| Working set total moyen / min / max | **20,464 / 20,379 / 20,727 Mio** |
| Working set privé avant / après | **2,137 / 2,090 Mio** (deux relevés PerfProc) |
| Engagement privé moyen / maximum | **2,803 / 2,984 Mio** |
| Handles début / min / max / fin | **260 / 251 / 260 / 255** |
| Threads début / min / max / fin | 10 / 3 / 11 / 7 |
| Objets GDI | **17**, stables |
| Objets USER début / min / max / fin | **20 / 19 / 20 / 19** |
| GPU | Compteurs par PID vides avant/après ; attribution indisponible |

La cible CPU moyenne ≤ 0,1 % et le plafond de working set privé de 60 Mio sont respectés dans cet essai compact visible. Aucun accroissement continu de handles/GDI n'est observé. Le working set total augmente d'environ 292 Kio entre premier et dernier échantillon, tandis que le privé mesuré avant/après diminue : ce seul relevé ne démontre pas une fuite ni son absence en endurance.

Preuves : `artifacts/validation-20261103-215154/` (environnement, CSV, résumé, compteurs avant/après). Binaire SHA-256 : `37274C8389EABEAD04746B28EBCDFE2D09DAA9BE2F5ECDCCF25DEE797C2E8E77`.

### Dépliée — premier essai non valide

Le panneau a atteint 480 × 210 pixels à 125 %, puis était revenu à 215 × 40 avant la fin de la stabilisation. Le garde-fou `ExpectedState expanded` a arrêté le script **avant toute collecte au repos**. La cause précise du repli n'est pas établie par ce diagnostic ; l'application restait visible et répondait. Les inventaires de processus effectués dans le bac à sable ne voient pas nécessairement les exécutables lancés sur le bureau ; le nettoyage s'appuie sur l'inventaire hors bac à sable et sur le chemin absolu du projet.

L'essai `artifacts/validation-20261103-215849/` contient uniquement l'environnement : aucun résultat CPU/RAM de cinq minutes déplié ne peut en être tiré. L'utilisateur a ensuite confirmé une période sans interaction pour recommencer. La fermeture extérieure reste active ; aucun forçage en boucle du premier plan.

### Dépliée visible — seconde collecte complète

Après fermeture propre du précédent prototype et du banc restant, nouvelle exécution PID 35368 du **même binaire SHA-256**. 60 s de stabilisation puis **300,980 s**, **296 échantillons, tous visibles et dépliés**, 480 × 210 pixels, DPI 120. Aucun test lourd ni manipulation du bureau pendant la collecte. Le panneau avait le premier plan aux diagnostics initial et final.

| Mesure | Résultat |
|---|---|
| Temps CPU consommé / CPU moyen / pic | **0 s / 0 % / 0 % détectés par le compteur** |
| Working set total moyen / min / max | **20,977 / 20,902 / 21,246 Mio** |
| Working set privé avant / après | **2,406 / 1,727 Mio** (deux relevés PerfProc) |
| Engagement privé moyen / maximum | **3,154 / 3,258 Mio** |
| Handles début / min / max / fin | **256 / 250 / 256 / 252** |
| Threads début / min / max / fin | 8 / 3 / 8 / 7 |
| Objets GDI | **17**, stables |
| Objets USER début / min / max / fin | **21 / 20 / 21 / 20** |
| GPU | Compteurs par PID vides avant/après ; attribution indisponible |

Zéro CPU mesuré signifie que TotalProcessorTime n'a pas progressé à la précision disponible, pas que le processus ou la composition n'ont absolument aucun coût. Les sessions compacte et dépliée ont des activités de fond différentes ; ne pas conclure que le panneau déplié consomme moins de CPU. Les cibles de moyenne CPU et de mémoire privée sont respectées dans cette seconde collecte également. Pas de croissance continue de handles/GDI observée ; le working set total prend environ 276 Kio entre premier et dernier point, ce qui ne constitue pas à lui seul un diagnostic de fuite.

Preuves : `artifacts/validation-20261103-220227/`. L'activité DWM, le GPU dédié, les pics GPU transitoires et une latence visuelle réelle restent non mesurés. Aucune affirmation d'absence totale de ralentissement Windows ou de fuite sur longue durée.

## Arrêt et état livré

Instance unique recontrôlée : second lancement terminé avec code 0, première instance conservée. Après la dernière collecte, WM_CLOSE a fermé le prototype en moins de deux secondes ; le banc PID 5392 a quitté via CloseMainWindow. L'inventaire final hors bac à sable par chemin absolu retourne **zéro processus du projet**. Le code de sortie du premier processus obtenu par Get-Process n'était pas disponible : seule sa terminaison est certifiée. Le parcours humain du menu Quitter reste à tester séparément.

Preuves de cycle de vie : `artifacts/satin-lifecycle.json` (premier arrêt, banc encore présent puis fermé avant la reprise) et `artifacts/satin-final-lifecycle.json` (nettoyage final complet). Exécutable prêt : `target/release/dynamic-island.exe`, 177 152 octets, SHA-256 identique aux deux collectes complètes. Les modifications restent dans le répertoire de travail pour revue ; le commit préalable de sauvegarde est conservé. Aucun contenu publié sur GitHub.

## Limites et validation manuelle

La refonte n'autorise pas à requalifier d'anciens essais manuels comme essais de cette version. Aucun échec automatisé restant ; le ressenti de fluidité, le clavier réel et la composition sur fonds clairs/sombres demandent votre confirmation. Plusieurs écrans, changement physique de DPI/résolution, jeux exclusifs, veille et endurance d'une heure ne sont pas certifiés. Le timer Win32 reste non synchronisé à l'écran. L'accessibilité du panneau dessiné reste limitée, sans fournisseur UI Automation.

Pour valider après lancement de `target\release\dynamic-island.exe` :

1. Cliquer sur la pilule : ouverture ; recliquer : repli. Vérifier la lisibilité et les proportions.
2. Cliquer pour ouvrir puis presser immédiatement Échap, plusieurs fois à des instants différents ; le panneau doit repartir de sa position courante, sans saut ni réouverture. Répéter une fois l'ouverture terminée.
3. Alterner rapidement les clics pendant les transitions ; vérifier l'absence de blocage et de rebond excessif.
4. Ouvrir, puis Alt+Tab : l'autre application doit garder son focus ; Échap doit alors rester destiné à cette application. Survoler l'encoche sans cliquer ne doit pas déplacer le focus.
5. Cliquer à l'extérieur et dans un coin transparent ; le clic doit parvenir à sa cible. Le panneau reste repliable à la souris sans clavier.
6. Avec le banc local, F11 doit masquer l'encoche en plein écran puis la rendre en revenant en fenêtre. Quitter via l'icône système.

Attendre la validation de l'utilisateur avant tout ajout de module ou nouvelle phase.
