# Stabilisation du socle flottant — 4 novembre 2026

Périmètre : revue des changements depuis `0ac7f9a`, vérifications, correction de robustesse et documentation. Aucun changement de design, de dimensions, d'animation, de dépendance ou nouvelle fonctionnalité. Aucun téléchargement, installation, publication ou modification globale Windows.

## Validation manuelle reçue

L'utilisateur confirme sept essais réussis sur le mode flottant livré : Opera maximisé sans bande AppBar ; Ctrl + glisser ; déplacement par menu ; Échap pendant déplacement et ouverture ; masquage/restauration par tray ; position conservée au redémarrage ; Alt+Tab et clic extérieur. Ce retour est distinct des tests programmatiques ci-dessous. Plein écran exclusif, multi-écrans et raccourci physique ne sont pas compris dans ces sept essais.

## Revue depuis la sauvegarde

- Retrait de la fenêtre AppBar et de son API : aucun appel SHAppBarMessage/ABM_NEW/ABM_SETPOS/SPI_SETWORKAREA dans les sources applicatives. L'ancien argument intégré n'active plus de réservation. Les anciens bancs de réservation sont retirés.
- Placement, préférences et visibilité séparés. Position normalisée bornée, fichier versionné et écriture coalescée hors UI ; la dernière écriture est terminée à l'arrêt.
- La surface BGRA, la typographie, la palette et les courbes/durées d'animation sont inchangées. Les déplacements à contenu identique réutilisent l'image composée.
- Aucun hook clavier global. RegisterHotKey réserve uniquement la combinaison configurée, désactivée par défaut. Désinscription à l'arrêt.
- WH_MOUSE_LL limité au panneau déplié pour le clic extérieur, retiré au repli/masquage/arrêt. Dix cycles réels d'installation/retrait couverts dans le test natif ajouté.
- Trois abonnements WinEvent restent actifs pendant l'exécution : premier plan, position de la fenêtre active, fin de minimisation. Ils sont nécessaires au plein écran événementiel, ne sondent pas la souris et sont libérés à l'arrêt.
- Capture limitée au geste ; annulation réentrante conservée. Timers d'animation et de restauration du tray bornés, aucun timer continu à l'état stable. Thread UI dormant dans GetMessage ; writer dormant sur condition variable.
- DIB/DC/polices détenus et libérés par leurs propriétaires, objets sélectionnés restaurés avant destruction. Les tests de 100 cycles contrôlent la stabilité GDI/USER, sans certifier tous les chemins d'échec Windows.

## Défaut reproduit et corrigé

Le message enregistré `TaskbarCreated` était examiné après l'emprunt mutable de l'état. Si un appel Win32 réentrait pendant cet emprunt, le message était abandonné : l'icône pouvait manquer après le retour du Shell.

Le test ajouté a d'abord échoué explicitement avec « reentrant TaskbarCreated was lost ». Le numéro de message et son indicateur différé sont maintenant stockés dans des Cell propres au thread. La notification est reconnue avant l'emprunt, regroupée et traitée après libération de l'état, y compris au démarrage. Aucun thread/polling/timer permanent ajouté.

Test natif ciblé : suppression uniquement de l'icône temporaire du test, message envoyé seulement à sa fenêtre cachée, restauration effective via Shell_NotifyIcon, regroupement des notifications et inaction d'un timer obsolète. Le chemin d'échec utilise un HWND de test vérifié invalide ; les tentatives s'arrêtent à trois. Le test ne redémarre pas Explorer et n'émet aucun broadcast global. `cbSize=0` étant accepté par le Shell présent, il n'est pas utilisé pour simuler un échec. Le message d'erreur ne prétend plus qu'un raccourci est disponible lorsqu'il n'a pas été enregistré.

Le collecteur rejette maintenant explicitement une fenêtre absente ou appartenant à un autre PID : elle ne peut plus être confondue avec une encoche volontairement masquée.

## Vérifications automatisées

Rust/cargo stable 1.99.1, MSVC x64, exécution offline avec Cargo.lock conservé.

- `cargo fmt --all -- --check` : réussi.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` : réussi.
- `cargo test --workspace --all-targets --release --locked --offline` : 18 tests réussis, aucun ignoré.
- Tests de documentation : réussis, aucun exemple de documentation à exécuter.
- Compilation Release du workspace et des exemples : réussie après fermeture normale d'une ancienne instance masquée qui verrouillait l'EXE (erreur Windows 5 lors du premier remplacement).
- 100 cycles de rendu, 3300 surfaces : compteurs GDI/USER 12/3 identiques aux points de contrôle 25, 50, 75 et 100. Passage mesuré : médiane 3,573 ms, p95 9,250 ms, maximum 19,879 ms par allocation/dessin/libération. Ces chiffres concernent le raster hors présentation, avec applications tierces conservées ; ils ne mesurent pas une fréquence d'affichage garantie.

Binaire Release stabilisé : 261120 octets, SHA-256 `B4B7E808567DF253938E0B7320FF2C9A00ACFFADE00A54178E287745EA5DEC5A`.

Banc Release : 18/18 contrôles réussis, preuve `artifacts/floating-20261104-191847/report.json`. Après 5 cycles d’échauffement et 30 cycles ouverture/repli/masquage/restauration : GDI 17→17, USER 21→21, octets privés 3158016→3158016, handles 262→263. Variation d’un handle sans croissance de la mémoire privée ni des objets graphiques ; pas de preuve absolue d’absence de fuite. Zone de travail avant, pendant et après (0,0)–(1920,1020), cadre maximisé du banc identique. Plein écran et masquage manuel indépendants vérifiés. Position de bord chargée, recentrage et fermeture en moins de cinq secondes réussis.

Raccourci : 5/5 contrôles réussis, `artifacts/shortcut-20261104-191945/report.json`. Enregistrement, masquage/restauration via WM_HOTKEY, fermeture et libération de la combinaison. Il s’agit de messages natifs, pas d’un essai de frappe physique.

## Mesure compacte stricte et fermeture finale

Nouvelle instance Release, préférences de test isolées dans `artifacts/`, 60 secondes de stabilisation puis 300,140 secondes de collecte. L'utilisateur a accepté de laisser le bureau au repos et a été prévenu au début et à la fin. Les 294 relevés sur 294 confirment l'encoche compacte visible en mode flottant ; aucun état déplié ou masqué n'a contaminé cette mesure. Preuves : `artifacts/validation-20261104-192026/`.

Machine : Windows 11, Core i5-12450HX (12 processeurs logiques), un écran 1920 × 1080 à 125 % (120 DPI). Applications tierces conservées.

| Compteur du processus | Résultat |
| --- | --- |
| CPU cumulé sur cinq minutes | aucune progression détectée (delta 0 s) |
| CPU normalisé machine, moyenne / pic échantillonné | 0 % / 0 % à la résolution du compteur |
| Mémoire résidente totale, moyenne / maximum | 16,16 / 16,43 Mio |
| Engagement privé, moyenne / maximum | 2,30 / 2,39 Mio |
| Mémoire résidente privée, avant / après | 1 818 624 / 1 843 200 octets |
| Handles, minimum–maximum ; début → fin | 221–227 ; 227 → 223 |
| Threads, minimum–maximum | 4–8 |
| Objets GDI / USER | 9 / 10, constants |

Le delta CPU nul indique une activité sous la résolution de la mesure, pas un coût absolument nul. La mémoire résidente inclut les pages partagées ; l'engagement privé est un autre compteur. Les relevés GPU attribuables au processus sont indisponibles (liste vide), ce qui ne prouve pas une consommation GPU nulle et ne mesure pas le coût partagé de DWM. Cette instance fraîche n'a pas ouvert le panneau : ses caches ne sont pas identiques à ceux des précédentes mesures mixtes. La nouvelle collecte constitue la référence compacte stricte, sans conclure à un gain de performance dû au correctif.

Fermeture finale par le message normal du seul prototype : attente terminée en moins de cinq secondes, journal `WM_QUIT received` puis `run result=Ok(())`, aucun processus du projet restant. Le champ ExitCode de l'objet PowerShell est nul/non disponible ; aucun code numérique de sortie n'est revendiqué pour cette fermeture. Preuves : `artifacts/stable-compact.log` et `artifacts/stable-shutdown.json`. Le banc a séparément vérifié la zone de travail inchangée après fermeture, la persistance et la libération du raccourci. Les préférences réelles de l'utilisateur n'ont pas été remplacées par celles des tests.

## Limites conservées

Le plein écran reste une heuristique ; les jeux exclusifs et les configurations multi-écrans physiques ne sont pas certifiés. La récupération de l'icône est testée isolément, sans redémarrage réel d'Explorer ni crash forcé. L'accessibilité du panneau dessiné reste limitée. Un stockage bloqué peut retarder la jointure du writer à la fermeture. Certains retours Win32 de nettoyage/menu ne sont pas contrôlés. La version stable désigne ce socle et son périmètre testé, pas une garantie universelle.

Référence vérifiée via Context7 : [Shell_NotifyIconW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw). [Architecture](architecture.md), [historique de l'implémentation et des mesures](floating-mode.md).
