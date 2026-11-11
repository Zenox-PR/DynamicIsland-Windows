# Architecture du prototype natif

État : mode flottant déplaçable après abandon de l’AppBar, sept essais manuels validés. [Stabilisation](stabilization-floating.md). [Comportement, API, configuration et validation](floating-mode.md). Le [rapport AppBar](appbar-report.md) est historique ; son code reste accessible dans la sauvegarde Git `0ac7f9a`.

## Structure

Le [mode concentration](concentration.md) est validé physiquement et sauvegardé dans `f759778`. Le [multimédia GSMTC](media.md) est sauvegardé dans `e7eee43` après accord de poursuite ; les limitations des fournisseurs restent documentées. Les [modules et la navigation P3](modules.md) sont validés manuellement le 11 novembre ; P4 Pomodoro/minuteurs est autorisée.

Rust stable MSVC x64, windows-sys 0.61.2 et windows-link 0.2.1 pour Win32 ; projections Microsoft windows 0.62.2 et windows-future 0.3.2 pour WinRT (versions exactes, Cargo.lock conservé). Pas de runtime async tiers. Media_Playback est une feature de développement pour le seul banc SMTC, pas pour l'application distribuée.

```text
island-app → island-windows → island-ui → island-core
                          └────────────→ island-core

island-core/placement.rs       placement relatif, seuil de geste
island-core/visibility.rs      présentation Normal/Concentration et causes de masquage
island-core/media.rs           données bornées, sélection de session, progression
island-core/modules.rs         catalogue compilé, activation et page
island-core/timer.rs           échéances injectées, minuteur et cycle Pomodoro
island-ui/navigation.rs        géométrie, onglets et états visuels
island-windows/navigation.rs   routage des entrées et cycle de vie des modules
island-windows/media.rs        worker MTA, événements GSMTC, commandes, décodage
island-windows/media_view.rs   messages UI, menu de sessions, timer visible
island-windows/timer_view.rs   échéance HWND, réveil Windows, durées et notification de fin
island-ui/media.rs            géométrie, boutons, miniature et progression
island-windows/concentration.rs style traversant et transitions natives
island-windows/placement.rs    lecture des moniteurs et zones de travail
island-windows/preferences.rs  validation, raccourci et écriture locale coalescée
island-windows/surface.rs      DIB BGRA, GDI, polices et ressources
island-windows/lib.rs          messages, focus, capture, tray, animation
```

Le cœur et l’UI restent indépendants de Windows. WinRT est confiné au worker de l'adaptateur Windows : la fenêtre ne reçoit que des copies de données Rust et une miniature BGRA partagée en lecture seule. Aucun WebView, serveur ou service externe.

## Rendu

DIB BGRA prémultipliée présentée avec UpdateLayeredWindow. Texte GDI/Segoe UI, alpha rétabli dans les rectangles textuels après GdiFlush. Quatre polices et chaînes UTF-16 en cache par DPI ; surface renouvelée lorsque les dimensions changent. DWM compose le résultat : pas de garantie de GPU nul ni d’absence de réveil du GPU dédié.

Noir satiné inchangé : 172 × 32 DIP/rayon 16 compact, 384 × 168/rayon 26 déplié, marge haute initiale 8 DIP, marges internes 24 DIP. Un déplacement sans changement de contenu réutilise l’image composée et déplace seulement notre HWND.

Transition smoothstep 220 ms/180 ms, durée adaptée à la distance restante. Inversion continue en position, pas en vitesse. Répéter une cible ne prolonge pas l’animation. Timer 15 ms seulement pendant les transitions, sans VSync/timeBeginPeriod. Préférence Windows de réduction d’animation lue uniquement.

## Interaction et focus

WS_POPUP et WS_EX_LAYERED/TOOLWINDOW/NOACTIVATE/TOPMOST, alpha nul hors de la forme. Aucun HWND AppBar ni SHAppBarMessage dans le code courant. Aucun SPI_SETWORKAREA ni redimensionnement de fenêtre tierce.

Clic pour ouvrir/replier. WM_MOUSEACTIVATE retourne MA_NOACTIVATE ; activation demandée uniquement lors d’une action explicite. Les refus Windows ne sont pas contournés. Échap traite seulement notre fenêtre avec focus et survit aux callbacks réentrants grâce à un booléen différé. Alt+Tab/clic extérieur ne réactive pas de force la fenêtre précédente.

Ctrl + glisser, ou commande de menu puis glissement sans Ctrl. Seuil Windows/DPI, capture limitée au geste, refus si l’activation explicite échoue. Relâchement : sauvegarde sans clic parasite. Échap/perte de capture/désactivation : annulation. Perte de capture réentrante différée après libération de l’état. Aucun hook clavier.

Le hook souris existant fonctionne uniquement pendant le panneau déplié et transmet toujours les entrées ; la fermeture extérieure est suspendue pendant notre geste. Aucun polling souris. Raccourci optionnel via RegisterHotKey/MOD_NOREPEAT, libéré à l’arrêt ; cette combinaison est réservée pendant l’exécution.

## Placement et visibilité

Ancrage relatif à la zone de travail lue du moniteur choisi. Coordonnées négatives et DPI traités. La position compacte préférée reste distincte du rectangle déplié borné aux bords. Moniteur absent : retour au principal. Changements via WM_DPICHANGED/WM_DISPLAYCHANGE/WM_SETTINGCHANGE, sans configuration globale.

Masquage manuel et plein écran indépendants du mode Normal/Concentration. Le masquage est prioritaire sur le rendu. Concentration : ligne de 96 × 4 DIP centrée sur le compact mémorisé, WS_EX_TRANSPARENT sur notre HWND layered, animation arrêtée, capture et hook souris libérés. Aucun polling ni callback souris ajouté. La bascule de présentation préserve les causes de masquage. Afficher/Recentrer/clic tray enlèvent le masquage manuel et restaurent le normal ; seul l'environnement change la cause plein écran. Retour sans activation, ouverture/déplacement uniquement explicites. Présentation et masquage volontaire limités à la session.

WinEvent premier plan, position de fenêtre active et fin de minimisation, sans surveillance périodique. Heuristique plein écran sur le moniteur du panneau : fenêtre active sans légende couvrant l’écran, hors shell, non minimisée/maximisée. Les jeux exclusifs et cas borderless atypiques restent à vérifier.

## Stockage et arrêt

Le raccourci concentration est une seconde inscription RegisterHotKey facultative, sans hook clavier. Conflit non bloquant signalé par infobulle, inscription libérée à l'arrêt. Préférences v1 lues et conservées lorsqu'il n'est pas configuré ; v2 ajoute concentration_hotkey. La position ne dépend pas de la taille de ligne. Les contrats de priorité de visibilité décrits dans concentration.md s'appliquent aux snapshots multimédias ; ceux-ci ne peuvent changer la présentation.

Thread UI dormant dans GetMessage ; un writer dormant sur condition variable avec une seule préférence en attente remplaçable. Pas d’écriture à chaque mouvement. Fichier versionné strict de 4 Kio maximum sous LOCALAPPDATA ; remplacement temporaire atomique, flush puis jointure du worker à l’arrêt. Un stockage bloqué peut retarder la fermeture.

Mutex Local\DynamicIsland.Native.Prototype, une instance par session. RefCell sur le thread UI, pas de double emprunt mutable dans les callbacks réentrants. Aucun pointeur de message conservé. Erreurs de rendu : arrêt ordonné. Préférences/raccourci indisponibles : avertissement et secours par tray.

TaskbarCreated est conservé même pendant une réentrance, via un indicateur propre au thread ; il récupère l’icône en trois tentatives maximum (250 ms, puis 1 s) ; aucun retour de réservation. À l’arrêt : timers, worker, capture, raccourci, hooks, abonnements, tray, HICON, polices, bitmap, DC puis HWND/classe/mutex libérés. Aucun test forcé de plantage/Explorer.

## Limites

Accessibilité limitée au titre et aux menus natifs ; aucun fournisseur UI Automation du panneau. Certains retours Win32 de nettoyage/menu/DPI ne sont pas contrôlés. Les tests ne certifient pas tous les chemins unsafe.

Aucune élévation produit, installation, connexion réseau interne, autostart/service ou modification globale. Un raccourci HTTPS explicitement activé ouvre le navigateur externe. Préférences propres à l’application uniquement. Aucun code/ressource WindyLand/Nusa/L4 copié. Modules autorisés à ce stade : multimédia, minuteurs et raccourcis ; P6 et suivantes exclues. Sans réservation, la forme visible peut recouvrir des onglets : déplacement/masquage/recentrage/concentration donnent le contrôle à l’utilisateur.

## Refonte visuelle P1 (nouvelle feuille de route)

`island-core/selector.rs` conserve les choix par ID et annule une pression périmée. `island-ui/theme.rs` et `popover.rs` définissent palette, géométrie et raster sans Win32. `island-windows/popover.rs` possède un HWND layered, sa DIB et ses polices, sur le thread UI existant ; il poste des événements scalaires à son propriétaire. Fermeture par Échap/désactivation/masquage/changement d'écran ; destruction et libération du timer/capture/classe par RAII. Aucun hook ni thread supplémentaire. Le hook extérieur préexistant ignore la surface interactive du popover.

`app_names.rs` résout les noms via le Shell sur le worker COM existant, avec cache borné à 32. Les identifiants et commandes GSMTC restent séparés de la présentation. La résolution Shell est synchrone hors UI ; elle n'offre pas de délai maximal garanti si un composant système bloque. Le sélecteur et les boutons raster ne disposent pas encore d'un fournisseur UI Automation.

## Navigation P3

Accueil et Médias partagent le même HWND et la même géométrie dépliée. Une seule page est rendue ; les données du module restent indépendantes de la page. Les événements GSMTC ne changent ni page ni priorité de visibilité. Désactiver libère les abonnements et données sur le MTA ; un worker déjà créé attend ensuite sans délai sur sa Condvar. Aucun nouveau worker au démarrage si le module est désactivé. Le writer existant sauvegarde activation/page en v3, avec lecture v1/v2 et retour au schéma antérieur si les choix de modules sont remis aux valeurs par défaut. Voir [les contrats et vérifications](modules.md).

## Minuteurs P4

Voir [comportement et validation](timers.md). Moteur pur avec millisecondes monotones, adaptateur GetTickCount64 incluant la veille, un timer HWND à échéance finale lorsque le module n'est pas affiché. Affichage des secondes seulement sur la page visible ; aucun thread nouveau ni polling au repos. Désactivation = pause en mémoire ; navigation = état conservé ; fermeture = arrêt, seules les durées/préférences persistent. Notification via la tray existante, silencieuse et soumise à Windows, jamais de focus pris à la fin. Réconciliation coalescée après les callbacks Win32 réentrants. Aucun changement du style de concentration ni du moteur GSMTC.

## Raccourcis P5

Voir [contrats, stockage et protocole](launcher.md). Catalogue pur borné dans core ; icônes vectorielles dans UI ; adaptateur de lancement/stockage, coordination et éditeur séparés dans Windows. Un STA paresseux dort sur sa file bornée ; aucun polling ni timer nouveau. CreateProcessW reçoit une application absolue sans commande libre, ShellExecuteExW seulement un dossier/HTTPS, ActivateApplication un AUMID. Messages de résultat sans pointeur, réentrance coalescée, ancienne pression invalidée après changement de catalogue. Le formulaire indépendant non topmost utilise les contrôles et le routage clavier Win32. Annulation/fermeture de l'éditeur, arrêt des lancements en attente et jointure du worker à la fermeture ; les enfants lancés volontairement sont indépendants et ne sont pas tués par l'encoche. Schéma settings v5 rétrocompatible en lecture et catalogue shortcuts.ini séparé. Le moteur GSMTC et la présentation concentration restent inchangés.
