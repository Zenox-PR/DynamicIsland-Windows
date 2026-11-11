# Mode flottant — implémentation et validation

Ce rapport décrit le socle historique. L'évolution concentration, ses préférences et sa validation sont décrites dans [concentration.md](concentration.md).

**Mise à jour :** les sept essais manuels annoncés par l’utilisateur sont réussis. Voir [la stabilisation](stabilization-floating.md) pour la revue et les nouvelles mesures ; les résultats ci-dessous décrivent la livraison initiale.

Travaux autorisés le 3 novembre 2026. Sauvegarde Git locale préalable : `0ac7f9a` (prototype AppBar). Aucune publication ni installation. Le mode concentration reste une possibilité future ; aucune ligne, transparence volontaire ou observation permanente de souris n'est ajoutée.

## Comportement

- Flottant uniquement, sans HWND technique ni inscription AppBar. `--integrated` est désormais ignoré et consigné seulement si le journal de diagnostic est activé. `--floating` reste compatible. L'ancienne implémentation et son banc sont conservés dans Git, retirés des sources courantes.
- Design Noir satiné inchangé : 172 × 32 DIP replié, 384 × 168 déplié. Animations et traitement réentrant d'Échap conservés.
- Clic normal : ouvrir/replier. Ctrl + bouton gauche maintenu et glissé : déplacement 2D. Un seuil natif adapté au DPI évite le démarrage sur une petite variation de position. Un geste Ctrl sans déplacement ne déclenche pas d'ouverture.
- Menu contextuel de l'encoche ou de l'icône : « Déplacer l'encoche — puis glisser à la souris ». Cette commande affiche le compact et arme le prochain glissement sur l'encoche, sans Ctrl. Le curseur indique le déplacement. Échap ou perte d'activation annule l'armement.
- Pendant le glissement : capture limitée au geste, activation explicite requise. Refus du focus : aucun contournement, aucun hook clavier. Échap, perte de capture ou annulation rétablit la position précédente. Le relâchement valide sans clic parasite.
- « Masquer temporairement », « Afficher l'encoche », « Recentrer et afficher », « Quitter ». Le clic gauche sur l'icône système affiche le compact ; « Ouvrir / Replier (clavier) » conserve l'ouverture avec focus explicite.
- Le masquage manuel est indépendant du plein écran. Un aller-retour plein écran ne l'annule pas. La restauration respecte le plein écran détecté et ne s'impose pas au-dessus d'un jeu. Le masquage temporaire n'est pas mémorisé au prochain lancement.
- Une seule instance, pas de service, réseau applicatif, autostart ou changement de configuration Windows.

## Placement et préférences

`%LOCALAPPDATA%\DynamicIsland\settings.ini` est propre à l'application. Version 1, cinq clés :

```ini
version=1
monitor=
x=500000
y=0
hotkey=none
```

Laisser monitor vide sélectionne le principal. x/y sont des fractions entières entre 0 et 1000000 de la zone disponible pour le compact, et non des pixels. La position est bornée à la zone de travail existante, en la lisant uniquement. Moniteur disparu : retour au principal et recentrage. Un identifiant DISPLAY n'est pas une identité matérielle immuable ; le recentrage reste le secours.

Le panneau déplié est ajusté aux bords sans remplacer la position compacte préférée. Si un écran est plus petit que le panneau lui-même, l'origine reste accessible mais tout le contenu ne peut pas tenir sans modification du design.

Pour un raccourci optionnel, quitter l'application puis remplacer `hotkey=none` par exemple par `hotkey=Ctrl+Shift+F10`. Touches acceptées : A-Z, 0-9, F1-F11, avec Ctrl ou Alt, Shift facultatif. Windows/F12/Échap ne sont pas proposés. La combinaison est globale pendant l'exécution via RegisterHotKey/MOD_NOREPEAT, sans hook. Un conflit affiche une erreur et laisse le tray utilisable. La modification est chargée au redémarrage ; pas de surveillance de fichier. Ce paramètre ne vérifie pas les raccourcis locaux de toutes les autres applications.

Lecture limitée à 4 Kio, validation stricte du format et des bornes. Un fichier invalide rétablit les valeurs par défaut avec un avertissement ; le fichier est réécrit avec ces valeurs. Sauvegardes coalescées par un worker dormant sur condition variable, uniquement à l'initialisation et après des actions explicites ; aucun accès disque par mouvement souris. Remplacement atomique du fichier temporaire local via MoveFileExW. Le worker termine sa dernière écriture et est joint à l'arrêt. Une erreur d'écriture est signalée ; la position reste utilisable en mémoire. Un stockage bloqué peut retarder cette fin d'écriture.

Pour les bancs uniquement, `ISLAND_SETTINGS_PATH` et `ISLAND_DIAGNOSTICS_PATH` isolent les données dans artifacts/. Ne pas définir ces variables pour l'usage normal sauf besoin de diagnostic.

## Architecture et ressources

- `island-core/src/placement.rs` : fractions, limites, seuil de geste et causes de visibilité. Aucun accès Windows.
- `island-windows/src/placement.rs` : interrogation des moniteurs ; aucun changement d'écran ou de zone de travail.
- `island-windows/src/preferences.rs` : configuration, validation du raccourci et writer borné (une valeur en attente remplaçable).
- `island-windows/src/lib.rs` : orchestration des messages, capture, raccourci, visibilité, tray et focus.
- `island-ui` et la surface GDI restent inchangés. Un déplacement à taille/DPI/contenu constants réutilise l'image déjà composée ; seul le HWND est déplacé. Les frames d'animation sont toujours temporaires.

Le thread UI attend GetMessage au repos. Le writer attend une condition variable. Aucun polling ajouté. Le hook souris déjà présent est limité au panneau déplié et transmet les clics ; il ignore la fermeture extérieure pendant notre geste de déplacement. La capture est libérée sur les chemins de terminaison et le raccourci est désinscrit à l'arrêt. Les messages de perte de capture et Échap reçus pendant un emprunt réentrant sont différés, sans timer.

La détection plein écran suit le moniteur du HWND, exclut le shell, les fenêtres minimisées/maximisées, et recherche une fenêtre active sans légende couvrant le moniteur. C'est une heuristique : jeux exclusifs et fenêtres sans bordure atypiques restent à tester. Les événements Windows remplacent tout balayage périodique. La récupération du tray après TaskbarCreated conserve ses tentatives bornées ; elle ne peut plus recréer d'AppBar.

## Différence avec L4 et droits

Étude de L4 à la révision 6fba8977b61403ebe7db816631a6e8668b3552b2, licence GPL-3.0-only. Aucune copie de code ou de ressource L4, WindyLand ou Nusa. L'implémentation de déplacement, des préférences et de visibilité est originale.

L4 combine Tauri/WebView2/React avec sondage souris 16–50 ms et plein écran 400 ms. Son triple-clic active un mode traversant semi-transparent/ligne, avec détection de sortie par observation des clics. Ces mécanismes ne sont pas repris. Notre futur mode concentration devra rester distinct des causes de masquage ; la restauration par tray/raccourci est déjà indépendante de la surface de l'encoche.

Sans réservation, une encoche visible et interactive peut encore recouvrir des onglets. Le déplacement et le masquage volontaire sont les solutions proposées. Aucun ajustement manuel de fenêtre tierce ni SPI_SETWORKAREA.

## Validation

Sauvegarde `0ac7f9a`. Rust/cargo 1.99.1, compilation MSVC x64 offline. `cargo fmt --all -- --check`, Clippy workspace/all-targets avec `-D warnings`, tests workspace/all-targets en Release : 17 tests réussis. Ils couvrent les transitions, Échap réentrant, annulation de déplacement/perte de capture réentrante, géométrie/DPI théorique, transparence du texte, 100 cycles de surfaces, format borné des préférences et flush de la dernière sauvegarde. Exécutable final : 260608 octets, SHA-256 `B520985C0C9448237E1E46BEF7FEC8BA77CD311DC19201199E15C5D15418C147`.

Banc Release : 18 contrôles réussis, preuves `artifacts/floating-20261103-233735/report.json`. Écran réel unique 1920 × 1080, DPI 120 (125 %). Zone de travail avant, pendant et après : (0,0)–(1920,1020). Cadre DWM du banc maximisé identique : la bande ne subsiste pas. Pas d’AppBar au lancement, pendant l’essai ou après fermeture, même avec l’ancien argument `--integrated`. Le navigateur de l’utilisateur n’a pas été manipulé : sa validation visuelle reste manuelle.

Après 5 cycles d’échauffement, 30 cycles ouverture/repli/masquage/restauration : GDI 17→17, USER 21→21, octets privés 3244032→3244032, handles 262→263. Aucun accroissement GDI/USER observé ; la variation d’un handle ne prouve pas une fuite mais interdit d’affirmer une égalité de tous les compteurs. Le premier essai sans échauffement avait GDI 9→17 ; il est conservé dans `artifacts/floating-20261103-233601`. La stabilité après échauffement motive la distinction avec une croissance continue, sans garantie absolue d’absence de fuite.

Masquage/retour plein écran, masquage manuel persistant après ce cycle, restauration et recentrage réussis. Position de bord chargée : (1705,980)–(1920,1020), puis recentrage compact (852,10)–(1067,50). Fermeture normale dans le délai de cinq secondes, sortie Ok. Aucun test forcé de plantage/Explorer.

Incidents corrigés pendant le développement : normalisation de `hotkey=none` au rechargement, ciblage du banc (chaîne vide transmise par PowerShell au lieu d’un titre libre). Les échecs initiaux restent dans artifacts/. Les demandes d’activation programmatiques ont parfois été refusées par Windows ; le programme respecte ce refus. Ces essais ne valident donc pas le focus après un clic physique, ni le déplacement physique.

Mesure dépliée complète : `artifacts/validation-20261103-234552`, 60 s de stabilisation puis 300,197 s, 296/296 échantillons dépliés visibles et sans AppBar. CPU cumulé +0,015625 s, moyenne 0,000434 % de la capacité machine (12 processeurs logiques), mémoire résidente moyenne 21,50 Mio, octets privés engagés moyens 3,23 Mio, handles 254–256, GDI 17 stable, USER 21→20. Mémoire résidente privée CIM : 2,492→1,816 Mio. Compteurs GPU attribuables absents : usage inconnu, pas nul ; la composition DWM n’est pas isolée.

Deux collectes compactes de cinq minutes sont volontairement classées **mixtes**, pas repos strict. Première : `artifacts/validation-20261103-233856`, 293/296 échantillons compacts, +0,109375 s CPU. Seconde : `artifacts/validation-20261103-235213`, 291/296 compacts, +1,15625 s CPU, moyenne 0,032056 % machine, mémoire résidente moyenne 20,85 Mio, octets privés moyens 2,87 Mio, handles 254–256, GDI 17 stable, USER 19–23. De brèves ouvertures ont été observées ; le fichier de test contient aussi une position déplacée. L’utilisateur a demandé de conserver ces mesures signalées plutôt que recommencer. Elles ne constituent donc pas une certification de cinq minutes au repos compact.

Mesure masquée complète : `artifacts/validation-20261103-235805`, 15 s de stabilisation après les essais précédents puis 300,372 s ; 296/296 échantillons masqués et sans AppBar. CPU +0,015625 s, moyenne 0,000433 % machine, mémoire résidente moyenne 20,86 Mio, octets privés moyens 2,85 Mio, handles 254, GDI 17 et USER 19 stables. Aucune croissance de ces compteurs sur cette collecte. Compteurs GPU attribuables absents.

| État et validité | Durée | Échantillons conformes | CPU moyen machine | Mémoire résidente moyenne | Octets privés moyens | Handles | GDI |
|---|---:|---:|---:|---:|---:|---:|---:|
| Déplié, repos strict | 300,197 s | 296/296 | 0,000434 % | 21,50 Mio | 3,23 Mio | 254–256 | 17 |
| Compact, deuxième essai mixte | 300,583 s | 291/296 | 0,032056 % | 20,85 Mio | 2,87 Mio | 254–256 | 17 |
| Masqué, repos strict | 300,372 s | 296/296 | 0,000433 % | 20,86 Mio | 2,85 Mio | 254 | 17 |

CPU normalisé aux 12 processeurs logiques ; les très faibles deltas sont proches de la granularité du compteur, pas une promesse de CPU nul. La mémoire résidente inclut les pages partagées ; les octets privés représentent l’engagement privé, pas la mémoire résidente privée. La collecte ne mesure pas l’ensemble du coût DWM ni la latence de toutes les applications Windows.

Contrôle raccourci : `artifacts/shortcut-20261104-000343/report.json`, cinq résultats positifs : combinaison libre avant démarrage puis enregistrée par l’application, WM_HOTKEY masque/restaure, fermeture propre, combinaison libre après sortie. Aucune frappe globale injectée ; la combinaison physique reste à tester manuellement. Réglage isolé `Ctrl+Shift+F10`, aucun raccourci utilisateur permanent créé.

Fin des essais : prototype de mesure fermé normalement depuis son état masqué, puis instance du test raccourci fermée. Énumération finale : aucun processus du projet ; HWND du prototype absent, AppBar absente, zone de travail (0,0)–(1920,1020). Préférences de test confinées à artifacts/ ; le lancement normal utilise le fichier LOCALAPPDATA. Aucun processus tiers arrêté. Aucun changement global de Windows, aucune installation ou publication.

Fichiers ajoutés : `island-core/src/placement.rs`, `island-windows/src/placement.rs`, `island-windows/src/preferences.rs`, `scripts/verify-floating.ps1`, `scripts/verify-shortcut.ps1`, ce rapport. Orchestration, diagnostic, collecteur, README, AGENTS et architecture mis à jour. Ancien module AppBar, banc de réservation et script associé retirés ; anciens exécutables appbar_probe supprimés de target/examples pour éviter leur relancement accidentel. Les rapports antérieurs restent historiques et ne constituent pas des mesures de ce binaire.

Le script `scripts/verify-floating.ps1` manipule uniquement l'application et son banc, sans injection globale d'entrée. Il contrôle la zone de travail, le cadre DWM maximisé, le plein écran, la visibilité manuelle, les ressources après échauffement, la position chargée et le recentrage. Ses messages programmatiques ne remplacent pas des clics physiques.

Protocole manuel de la livraison initiale (les essais annoncés ensuite par l’utilisateur sont consignés dans le rapport de stabilisation) :

1. Maximiser votre navigateur : plus de bande réservée ; les onglets reviennent en haut. Déplacer l'encoche si sa forme recouvre l'un d'eux.
2. Cliquer pour ouvrir, Échap immédiatement puis après animation ; tester Alt+Tab et le clic extérieur.
3. Ctrl + glisser, puis le menu « Déplacer l'encoche » suivi d'un glissement sans Ctrl. Vérifier qu'un clic ordinaire ne déplace pas l'encoche.
4. Annuler un déplacement avec Échap ; vérifier la position initiale et l'absence d'ouverture parasite au relâchement.
5. Quitter/reprendre : position mémorisée. Masquer, restaurer par l'icône, puis « Recentrer et afficher ».
6. Si souhaité, configurer un raccourci puis relancer et tester sa combinaison physique. Il doit rester possible de restaurer depuis le tray si elle est indisponible.
7. F11 dans le navigateur ou le banc : masquage puis retour. Vérifier qu'un masquage manuel persiste après cet aller-retour.
8. Multi-écrans/DPI physiques seulement si le matériel le permet. Pas de changement durable de configuration ni redémarrage forcé d'Explorer.

Sources API : [capture souris](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setcapture), [raccourci](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey), [DPI](https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged), [libération AppBar historique](https://learn.microsoft.com/en-us/windows/win32/shell/abm-remove).
