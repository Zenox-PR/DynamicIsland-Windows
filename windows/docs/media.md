# Phase 2 — Multimédia natif GSMTC

État : refonte validée dans `dec6186`, finalisation multimédia P2 testée automatiquement, **validation manuelle multimédia encore requise**. Voir [le rapport P2 et son protocole](media-finalization.md). Le commit local `f759778` contient la phase concentration officiellement validée le 4 novembre 2026. Les 23 tests du socle passaient et l'arbre était propre avant le développement multimédia. Aucun code de `concentration.rs` n'a été modifié dans cette phase. Aucun code L4/WindyLand repris pour ce module.

## Périmètre réel

Découverte et contrôle des sessions que les lecteurs publient à Windows par SMTC/GSMTC. Musique et vidéo utilisent le même chemin. Un film, une série ou un animé n'a pas besoin d'un module spécifique : il faut que son lecteur publie une session et des métadonnées. Aucun scraping, inspection d'onglets, extension, authentification ou appel à un service distant.

Opera, Chrome, Edge, Firefox, Spotify et les lecteurs locaux ne constituent pas une liste de compatibilité certifiée. Le résultat dépend du lecteur, du site, des réglages et des données publiées. Une session de navigateur peut agréger plusieurs onglets. Les informations manquantes restent indisponibles. La source affichée est le nom résolu par le Shell Windows, avec cache borné et repli prudent « Lecteur Windows ». L'AUMID reste l'identité technique du moteur et du diagnostic. Aucun nom de domaine ou d'onglet n'est déduit.

Le panneau Noir satiné conserve 384 × 168 DIP. Titre tronqué avec ellipse, artiste/album/type disponible, pochette ou vignette de remplacement, progression si la durée est exploitable. Lecture/pause, précédent/suivant sont activés uniquement selon les capacités courantes. Aucun déplacement dans la piste ni volume ajouté. Le compact 172 × 32 DIP et la ligne 96 × 4 DIP restent inchangés, sans animation décorative.

Le clic sur un bouton commande le média sans replier. Le clic sur le fond conserve le repli. Tab/Maj+Tab parcourent les boutons et le sélecteur de source ; Entrée/Espace activent la sélection. Échap conserve son traitement local prioritaire, y compris pendant l'animation. Les notifications multimédias n'activent jamais le focus ni l'ouverture de l'encoche. Le masquage volontaire, le plein écran et la concentration restent prioritaires.

## Architecture et durées de vie

- `island-core/media.rs` : types sans COM, validation des données, sélection et calcul de progression, tests déterministes.
- `island-windows/media.rs` : unique worker MTA, RoInitialize/RoUninitialize équilibrés ; manager, sessions et tokens d'événements possédés par ce thread. Les callbacks ne font que marquer une boîte aux lettres bornée et réveiller sa Condvar.
- `media_view.rs` : réception d'un snapshot par WM_APP, popover sombre modeless, commandes non bloquantes et timers conditionnels. Une notification réentrante reçue pendant un menu reste en attente jusqu'à libération de l'état UI.
- `island-ui/media.rs` et `surface.rs` : géométrie commune au rendu et aux clics, raster BGRA/GDI/Segoe UI, aucune API multimédia dans le thread de rendu.

Maximum 32 sessions et 32 marques d'événements ; les notifications répétées fusionnent. Une sélection remplaçable, une commande en attente/exécution et un snapshot remplaçable, aucune file illimitée. Titre limité à 512 caractères, autres champs à 256, caractères de contrôle retirés. Aucun titre/pochette enregistré dans les préférences ou journaux applicatifs.

Les opérations WinRT asynchrones ont chacune un délai de 5 s et une annulation à l'arrêt. La génération invalide les réponses de métadonnées/pochettes devenues anciennes. Une commande porte l'identité stable, la génération et l'époque des événements ; une piste remplacée, session fermée, sélection différente ou invalidation en attente la fait refuser avant envoi. Le contenu périmé est retiré dans un snapshot avant la lecture asynchrone, et les nouveaux champs sont affectés ensemble après contrôle de fraîcheur. Le lecteur peut aussi refuser une commande annoncée comme disponible : le panneau indique l'échec et attend l'état réel, sans lecture optimiste fictive.

Les appels synchrones au broker Windows ou aux codecs restent dépendants de Windows ; aucune garantie absolue de délai en cas de broker bloqué. Le worker est joint avant destruction du HWND, les abonnements retirés avant désinitialisation WinRT. Une panne du manager est affichée ; « Actualiser / Réessayer » tente une nouvelle acquisition. Une erreur d'initialisation de l'appartement du worker nécessite un redémarrage de l'application.

## Sélection

1. Choix manuel encore présent (même en pause).
2. Session courante Windows si elle joue.
3. Session précédemment sélectionnée si elle joue.
4. Première session en lecture, dans un ordre stable.
5. Session courante Windows, puis ancienne sélection valide, puis première session.

La commande « Sessions multimédias… » existe aussi dans l'icône système. Elle ouvre le sélecteur en concentration ; si l'encoche est masquée, la réafficher d'abord, car le masquage/plein écran reste prioritaire. La sélection manuelle est temporaire ; elle disparaît avec la session et n'est pas persistée au redémarrage. Aucun média n'est contrôlé automatiquement lors d'un changement de sélection.

GSMTC renvoie de nouveaux wrappers COM lorsque la liste change : leur adresse n'est pas une identité de session stable. Le banc a détecté ce défaut, puis validé sa correction. L'identité logique est conservée par AUMID uniquement lorsqu'il est unique dans les deux listes ; les abonnements sont recréés sur les wrappers actuels et la génération est avancée. Si une source disparaît, son identité est retirée. Des sources identiques ambiguës ne sont jamais associées arbitrairement : un changement de liste peut alors annuler le choix manuel, à renouveler. Une fermeture/réouverture entre événements fusionnés peut être vue comme le remplacement de la même source ; toute commande antérieure est néanmoins invalidée par sa génération. Une commande refusée n'est jamais redirigée vers un autre lecteur.

## Images et progression

Seule la miniature de la session sélectionnée est décodée, à partir du flux fourni par Windows. Pas de téléchargement par l'application ni de cache disque. Flux limité à 8 Mio, dimensions au plus 4096 par côté et 4 millions de pixels avant décodage de sortie. Sortie proportionnelle maximale 160 × 160 BGRA prémultiplié, soit 100 Kio. Une ancienne image peut rester brièvement retenue par le snapshot UI pendant le remplacement. Les données trop grandes, invalides ou absentes donnent une vignette de remplacement ; les codecs système gardent leurs propres allocations transitoires.

La progression utilise les bornes, position et horodatage Windows avec le taux de lecture fourni. Les valeurs incohérentes et horodatages de plus d'un jour sont refusés ; la position est bornée à la fin. Pas de durée inventée pour un direct. La progression est extrapolée, pas une horloge certifiée : un lecteur qui ne publie pas ses changements peut devenir imprécis.

Timer UI de 1 Hz seulement pour une session en lecture, à durée connue, dans un panneau visible et entièrement déplié, jusqu'à la fin connue. Aucun timer de progression en compact, concentration, masqué, pause ou durée inconnue. Le worker dort sans délai au repos ; aucun polling de sessions, de souris ou d'onglets. Les autres timers sont limités aux transitions (survol 80 ms, miniature 140 ms, apparition du popover 120 ms), aux animations du socle et à la récupération bornée du tray. Le popover ne dispose pas de timer au repos.

## Vérifications et mesures

Voir [les résultats de validation et mesures](media-validation.md). Les sources de preuve locales sont dans `artifacts/` (ignoré par Git). Les mesures distinguent session réelle, fournisseur synthétique et absence de session. Les contrôles par messages Win32 ne prouvent pas les clics physiques.

Les fichiers `examples/media_probe.rs` (lecture seule, 8 s) et `examples/media_fixture.rs` (fournisseur silencieux synthétique et commandes limitées à son propre ID) sont des bancs de test, jamais des processus permanents du produit.

## Validation manuelle demandée avant clôture

1. **Opera + YouTube** : démarrer une vidéo, ouvrir l'encoche, vérifier titre/pochette/progression, pause et reprise. Changer de vidéo puis fermer l'onglet. Tester Échap pendant l'ouverture et après ouverture.
2. **Opera + HTML5** : utiliser votre lecteur habituel (film/série/animé possible). Noter s'il expose une session ; contrôler uniquement les boutons disponibles. Une session absente ne doit provoquer ni erreur ni métadonnées inventées.
3. **Spotify ou autre lecteur installé** : titre/pochette, pause/reprise, précédent/suivant selon les capacités. Ne rien installer pour ce test.
4. **Deux lecteurs simultanés** : choisir chacun par le titre de source ou le menu système, vérifier que les commandes concernent seulement le lecteur choisi ; revenir à « Sélection automatique ». Fermer le lecteur sélectionné.
5. **Concentration et masquage** : pendant une lecture, passer en concentration et cliquer dans les onglets derrière la ligne. Changer de piste depuis le lecteur : l'encoche ne doit ni s'ouvrir ni prendre le focus. Restaurer, tester Alt+Tab, clic extérieur, plein écran et retour.
6. **Arrêt et reprise** : quitter par le tray puis relancer ; position/raccourcis conservés, aucune double instance, sélection média automatique reconstruite.

**DPI/multi-écrans** : tests de raster à 100/125/150/200 % et logique de coordonnées ne valident pas les transitions physiques entre écrans. DPI dynamique, écrans multiples, jeux exclusifs et tous les lecteurs cités restent à vérifier sur matériel et applications disponibles. Accessibilité lecteur d'écran : pas encore de fournisseur UI Automation pour les boutons raster ; navigation clavier seulement dans cette phase.

## Sources et licences

API et signatures vérifiées avec Context7 et les bindings locaux versionnés. Sources primaires : [manager GSMTC](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager?view=winrt-26100), [session GSMTC](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssession?view=winrt-26100), [SMTC](https://microsoft.github.io/windows-docs-rs/doc/windows/Media/struct.SystemMediaTransportControls.html), [windows 0.62.2](https://docs.rs/crate/windows/0.62.2). GSMTC nécessite Windows 10 1809 ou ultérieur ; cible du projet Windows 11 x64. Les bindings Microsoft sont sous MIT ou Apache-2.0 ; aucune réutilisation GPL. Aucun outil système installé, dépendances Cargo verrouillées uniquement.
