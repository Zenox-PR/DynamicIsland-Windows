# AppBar native — prototype intégré en attente de validation manuelle

3 novembre 2026. Sauvegarde locale avant modification/activation : **d2bf438**, `chore: sauvegarde Noir satine avant integration AppBar`. L'ancien processus Release 34740 a été fermé par WM_CLOSE avant les travaux. Aucune installation, publication GitHub, modification directe de zone de travail par SPI_SETWORKAREA ou manipulation d'Explorer. Aucun test de plantage planifié ; un incident de terminaison associé au collecteur est décrit plus bas.

## Comportement livré

Deux modes dans le menu de l'icône système et le menu contextuel de l'encoche :

- **Mode intégré — essai, bande de 48 DIP** : réservation fixe en haut de l'écran principal, encoche Noir satiné centrée dans le rectangle accordé. Les fenêtres maximisées respectant la zone de travail sont placées en dessous.
- **Mode flottant — libérer l'espace** : ABM_REMOVE, destruction de la fenêtre technique, retour à la position flottante. Le panneau reste utilisable ; aucun redémarrage requis.

Le démarrage sans argument reste **flottant** pour respecter la demande de validation avant activation définitive. `--integrated` active explicitement l'essai, `--floating` prévaut sur l'autre argument. Aucun réglage n'est mémorisé sur disque. Le défaut intégré et la persistance seront une étape distincte après accord. Une instance déjà ouverte conserve son mode : la seconde invocation ne lui transmet pas d'arguments.

Le panneau actuel mesure 172 × 32 DIP au repos et 384 × 168 DIP déplié. La bande de 48 DIP reste inchangée pendant les animations. Au DPI réel 120 (125 %) : encoche 215 × 40 px, panneau 480 × 210 px et bande 60 px. Marge haute 10 px, soit 8 DIP. Le panneau déplié recouvre temporairement le contenu des applications comme demandé ; les maximisations ne suivent pas son animation.

La fenêtre technique est transparente et non activable ; le rendu, le clavier local, le focus et les animations restent sur la fenêtre graphique existante. Pas de nouveau hook clavier ou moteur graphique. La réservation reste active en plein écran ; le panneau est masqué sur cet écran et revient sans activation lors du retour en fenêtre.

## Implémentation et nettoyage

Nouveau module `crates/island-windows/src/appbar.rs`. Cycle ABM_NEW → ABM_QUERYPOS → ABM_SETPOS, validation du rectangle accordé, dimensionnement de notre seul HWND, puis ABM_REMOVE avant DestroyWindow. Le contexte DPI reste Per Monitor V2. Le recalcul suit les événements de position, DPI, affichage et paramètres, sans sondage permanent. Cette première version suit l'écran principal ; aucun sélecteur secondaire ou encoche dupliquée n'est ajouté.

La Box contenant les données de callback reste vivante jusqu'après la destruction du HWND. Les callbacks partagent uniquement des Cell et postent une notification regroupée ; ils n'empruntent pas l'objet AppBar mutablement. Le panneau conserve également un drapeau différé pour ne pas perdre une notification lors d'une réentrance de son état. Les réservations identiques ne sont pas répétées, pour éviter une boucle ABM_SETPOS/ABN_POSCHANGED. La classe technique est réutilisée pendant toute la vie du processus.

Les échecs après création ou inscription sont nettoyés par RAII. Un échec du mode intégré conserve ou rétablit le mode flottant et affiche une explication ; les erreurs de rendu suivent toujours l'arrêt propre existant. Un retour d'Explorer est traité via TaskbarCreated : au plus trois tentatives de récupération de l'icône et de la réservation (250 ms, puis 1 s entre tentatives). Le timer est supprimé après succès/échec final et à l'arrêt. Un message de timer obsolète est ignoré. Choisir Flottant pendant cette période retire la demande de réservation, sans annuler la restauration de l'icône.

La récupération réelle après redémarrage d'Explorer n'a pas été exercée : aucun redémarrage ni faux broadcast TaskbarCreated n'a été provoqué. La terminaison forcée ne garantit pas l'exécution de Drop ; aucun scénario de crash n'est déclaré validé. L'application ne restaure jamais aveuglément un ancien rectangle global : le Shell recalcule la zone en conservant les autres AppBars.

## Prototype isolé, avant raccordement

`crates/island-windows/examples/appbar_probe.rs` a été compilé puis exécuté sur le bureau, avant le raccordement au panneau. Il crée uniquement ses propres fenêtres, se termine en quelques secondes et garde ses ressources dans des propriétaires à destruction automatique. Il ne modifie ni la taille ni la position de fenêtres tierces ; les ajustements des maximisations sont effectués par Windows.

| Contrôle | Observation |
|---|---|
| Zone avant inscription | (0,0)–(1920,1020) |
| Zone après inscription | (0,60)–(1920,1020) |
| Rectangle technique | (0,0)–(1920,60), DPI 120 |
| Maximisation de la fenêtre du banc | Cadre visible DWM (0,60)–(1920,1020), aucun recouvrement de la bande |
| Hit-testing dans la bande | WindowFromPoint(100,30) désigne la fenêtre cible placée dessous, pas l'AppBar |
| Notifications à l'état stabilisé | 0 message de recalcul pendant la fenêtre d'observation de 500 ms |
| Retour flottant / libération | Zone exactement revenue à (0,0)–(1920,1020) |
| Erreur contrôlée pendant une initialisation possédant déjà l'AppBar | Retour Err ordinaire, Drop exécuté, zone exactement restaurée |

L'erreur contrôlée est une sortie Rust `Err`, pas un plantage ni une simulation de tous les échecs possibles du Shell. Le test de ciblage vérifie le routage natif ; il ne remplace pas un clic physique. Une seconde exécution `--click-test` a exposé une cible pendant 60 s. Computer Use a trouvé la fenêtre mais a renvoyé **« Computer Use app approval timed out »** avant toute entrée. Aucun clic n'a été envoyé/reçu (compteur 0) ; le banc a terminé normalement et libéré l'espace. Le clic réel reste donc à valider manuellement.

Preuves locales : `artifacts/appbar-isolated.txt`, `artifacts/appbar-click.txt`, `artifacts/appbar-click-errors.txt`.

## Vérification de l'application raccordée

`scripts/verify-appbar.ps1` lance le vrai binaire Release, son diagnostic et le banc du projet. Les changements de mode passent par les mêmes commandes Win32 que les menus. Les ouvertures du panneau sont programmatiques et ne sont pas présentées comme des clics physiques.

Les **20 contrôles** ont réussi, y compris après la correction finale des timers :

- Démarrage sans argument en flottant, sans réservation.
- Activation du mode intégré, encoche visible devant notre fenêtre maximisée.
- Trois ouvertures/fermetures : zone de travail constante pendant le mouvement et à l'état déplié. Les snapshots montrent bien les tailles intermédiaires puis 480 × 210 px.
- Trois allers-retours Flottant/Intégré : restitution exacte, aucune accumulation des 60 px.
- Plein écran de notre banc : encoche masquée, bande conservée ; retour en fenêtre : encoche visible.
- Fermeture normale du prototype : zone initiale restituée ; banc fermé ; aucun processus du projet restant à la fin de ce scénario.

Le rectangle brut d'une fenêtre maximisée peut inclure sa bordure invisible (par exemple top=50 au lieu de 60). Le banc isolé vérifie donc le cadre visible DWM, pas uniquement GetWindowRect. Le journal mentionne deux refus Windows de focus lors des demandes programmatiques répétées ; ils ne sont pas contournés. Cela confirme que ces commandes ne prouvent pas un test clavier physique après chaque ouverture.

Preuves : `artifacts/appbar-integration-20261103-224758/`, puis `artifacts/appbar-integration-20261103-225139/` pour le binaire corrigé (checks.json, windows.txt et application.log, fin `run result=Ok(())`).

## Qualité et rendu

- Cargo fmt appliqué et contrôle final `--check` réussi ; Clippy workspace/all-targets avec `-D warnings` réussi, aucun avertissement Rust masqué. `git diff --check` réussi ; Git annonce seulement sa conversion habituelle LF→CRLF, sans modification de configuration globale.
- **13 tests Rust** : 8 cœur, 1 UI, 4 Windows. Suite Debug/all-targets réussie avant la petite correction finale ; suite Release/all-targets réussie après, avec la régression du timer obsolète ajoutée au test natif.
- Les tests de documentation réussissent (aucun exemple exécutable). Les exemples se compilent, sans tests propres exécutés automatiquement sur le bureau.
- Compilation Release réussie, Rust 1.99.1 MSVC x64 et dépendances inchangées. Aucune nouvelle dépendance.
- Test Release de 100 cycles de rendu : GDI 5→5 et USER 0→0, 3 300 surfaces, 5,452 s ; médiane 1,403 ms, p95 3,749 ms, maximum 9,186 ms. Allocation/dessin/destruction uniquement, sans présentation DWM, et sans 100 cycles AppBar réels. Ces temps ne prouvent pas la cadence visible ni l'absence de fuite en endurance.

Preuves : `artifacts/appbar-tests.txt`, `artifacts/appbar-release-tests.txt`.

## Mesures de consommation

Configuration : Windows 11 Famille 10.0.26300, i5-12450HX (12 processeurs logiques), 23,71 Gio RAM, un écran 1920 × 1080 à 125 %, fréquence WMI 144 Hz. Intel UHD Graphics et Arc A530M détectés. Applications utilisateur conservées, pas de référence système isolée ni profil énergétique relevé.

Méthode : binaire Release, 60 s de stabilisation puis 300 s d'observation à environ 1 Hz. Le script vérifie chemin/PID, état compact/déplié, visibilité, présence d'une AppBar du même processus, hauteur DPI et limite supérieure de zone de travail à chaque point. Aucun test lourd en parallèle des collectes finales. CPU normalisé machine = 100 × delta temps CPU / (durée × 12). Working set total, working set privé et engagement privé sont distingués.

La première collecte compacte `artifacts/validation-20261103-224923/` a été interrompue volontairement par fermeture propre pour corriger le cas limite de récupération d'Explorer. Elle n'est pas utilisée comme mesure finale.

L'essai déplié `artifacts/validation-20261103-225150/` est également incomplet : des échantillons corrects jusqu'à 112,617 s, puis repli observé, cause non déterminée par ce diagnostic. Il ne compte pas comme cinq minutes dépliées. L'interruption du collecteur par le terminal a eu l'effet non souhaité de terminer également les processus enfants lancés dans sa session. Le contrôle immédiat a constaté zéro processus du projet, aucune fenêtre AppBar et la zone revenue à (0,0)–(1920,1020). Ce n'était pas un test de plantage planifié, et ce constat ponctuel ne certifie pas le nettoyage de tous les arrêts brutaux. L'incident a été signalé à l'utilisateur. Les relances suivantes du prototype et du collecteur utilisent des sessions distinctes pour éviter cet effet.

L'utilisateur a confirmé une nouvelle période de six minutes sans interaction. Aucun scénario supplémentaire de crash ni manipulation d'Explorer n'a été ajouté au protocole.

### Déplié intégré — collecte complète

`artifacts/validation-20261103-225701/`, PID 14216 : **300,181 s, 296 échantillons**, tous visibles, dépliés et en mode intégré, bande de 60 px et zone de travail commençant à y=60. Aucun repli pendant cette nouvelle collecte.

| Mesure | Résultat |
|---|---|
| CPU cumulé / moyenne machine / pic par intervalle | **0 s / 0 % / 0 % détectés** |
| Working set total moyen / maximum | **21,356 / 21,625 Mio** |
| Working set privé avant / après | **2,480 / 2,480 Mio** |
| Engagement privé moyen / maximum | **3,225 / 3,313 Mio** |
| Handles début / min / max / fin | **262 / 254 / 262 / 256** |
| Threads début / min / max / fin | 7 / 3 / 7 / 7 |
| Objets GDI | **17**, stables |
| Objets USER début / min / max / fin | **22 / 21 / 22 / 21** |
| GPU | Compteurs par PID vides, attribution indisponible |

Zéro CPU détecté signifie que le compteur TotalProcessorTime n'a pas progressé à sa précision disponible. Cela ne prouve pas un coût système ou DWM nul. Le working set total a augmenté d'environ 308 Kio entre les extrémités, alors que le working set privé est identique : ce constat seul ne prouve ni une fuite ni son absence en longue durée.

Le binaire mesuré porte l'empreinte SHA-256 **17BF422EF98469B03596BD452FFA527F20198033C1A9556EF7E6502EA5206CB8** (193 024 octets). Les objectifs de moyenne CPU ≤ 0,1 % et de working set privé ≤ 60 Mio sont respectés dans ce scénario déplié visible.

### Compact intégré — collecte complète

`artifacts/validation-20261103-230356/`, même PID et même binaire : **300,669 s, 296 échantillons**, tous visibles, compacts et en mode intégré, bande de 60 px et zone de travail commençant à y=60. La collecte suit 60 secondes de stabilisation après le repli.

| Mesure | Résultat |
|---|---|
| CPU cumulé / moyenne machine / pic par intervalle | **0 s / 0 % / 0 % détectés** |
| Working set total moyen / maximum | **20,909 / 20,930 Mio** |
| Working set privé avant / après | **2,070 / 2,070 Mio** |
| Engagement privé moyen / maximum | **2,848 / 2,879 Mio** |
| Handles | **254**, stables |
| Threads début / min / max / fin | 4 / 3 / 5 / 4 |
| Objets GDI | **17**, stables |
| Objets USER | **20**, stables |
| GPU | Compteurs par PID vides, attribution indisponible |

Les mêmes limites de précision CPU et d'attribution GPU/DWM s'appliquent. Le working set total augmente de 8 Kio entre premier et dernier point, le privé reste identique. Les cibles CPU/mémoire sont respectées sur cette collecte aussi ; aucun accroissement continu des handles/GDI/USER observé. Ces deux sessions ne démontrent pas l'absence de fuite pendant une longue session interactive ou un coût graphique nul.

## État final et fichiers

Après les mesures, fermeture normale du prototype en moins de deux secondes et fermeture du seul banc du projet. Zone de travail **(0,0)–(1920,1020)** retrouvée, fenêtre AppBar absente. Un dernier lancement `--integrated --floating` confirme la priorité du mode flottant, sans réservation, puis quitte avec code 0. Inventaire final par chemin absolu : **zéro processus du projet**. Preuve : `artifacts/appbar-final-lifecycle.json` ; l'empreinte du binaire livré est identique aux deux collectes complètes. Le journal du processus mesuré contient `run result=Ok(())`.

Fichiers créés :

- `crates/island-windows/src/appbar.rs` : propriétaire de réservation et notifications natives.
- `crates/island-windows/examples/appbar_probe.rs` : prototype isolé, maximisation, hit-testing, erreur contrôlée et libération.
- `scripts/verify-appbar.ps1` : vérification du vrai binaire et des deux modes.
- `docs/appbar-report.md` : présent rapport.

Fichiers modifiés : `crates/island-windows/src/lib.rs` (raccordement, modes, récupération), `examples/desktop_fixture.rs` et `examples/inspect.rs` dans ce même crate (diagnostics et préparation des essais), `scripts/measure-idle.ps1` (contrôle continu du mode et de la réservation), `README.md`, `AGENTS.md`, `docs/architecture.md`. Cargo.toml, Cargo.lock, dessin Noir satiné et moteur de transition inchangés. Les changements restent dans le répertoire de travail pour revue ; la sauvegarde Git préalable est conservée et rien n'est publié.

## Limites et protocole manuel

Le prototype cible l'écran principal. Les calculs DPI 100/125/150/200 % et coordonnées négatives sont testés, mais aucun changement réel de résolution/DPI ni essai multi-écrans n'a été imposé au poste. Les autres AppBars sont négociées via le Shell, sans preuve d'essai avec une barre tierce installée. Les fenêtres qui ignorent la zone de travail, les fenêtres flottantes placées manuellement, les jeux exclusifs et l'endurance d'une heure ne sont pas certifiés. La réservation utilise toute la largeur et reste vide autour de l'encoche.

Pour essayer, quitter l'ancienne instance puis lancer :

```powershell
& "C:\Projects\DynamicIsland-Windows\target\release\dynamic-island.exe" --integrated
```

1. Maximiser le navigateur et VS Code : vérifier que les onglets/boutons restent sous la bande et que l'encoche compacte reste visible.
2. Ouvrir/replier : les applications ne doivent pas changer de taille pendant l'animation ; le panneau peut les recouvrir temporairement.
3. Presser Échap immédiatement après le clic d'ouverture, puis après ouverture complète. Vérifier Alt+Tab et l'absence de prise de focus au survol.
4. Cliquer dans une partie transparente de la bande : le clic doit atteindre le bureau ou la fenêtre présente dessous. Le test isolé `appbar_probe.exe --click-test` fournit une cible dédiée pendant 60 s si souhaité, à lancer sans le prototype actif.
5. Passer en Mode flottant via le menu : l'espace doit être rendu immédiatement. Repasser en Intégré, puis Quitter : même restitution.
6. Tester F11 dans le banc local ou votre navigateur : masquage en plein écran, retour sans activation intempestive.

Si l'icône système est inaccessible, le clic droit sur l'encoche offre aussi Flottant et Quitter. Le diagnostic du projet `target\release\examples\inspect.exe floating` envoie la même commande à l'instance en cours ; `close` demande sa fermeture normale. Après fermeture, un lancement `--floating` garantit un essai sans réservation.

Ne pas activer définitivement le mode intégré, tester un plantage forcé, redémarrer Explorer ou ajouter un module avancé avant les validations correspondantes.

