# P3 — Modules et navigation native

## Périmètre livré

Deux onglets discrets, **Accueil** et **Médias**, dans le panneau existant de 384 × 168 DIP. Le compact, la concentration, la palette et les animations géométriques sont conservés. La source multimédia reste accessible à droite de l'en-tête ; son texte peut être ellipsé dans cette largeur réduite.

Accueil affiche uniquement le module réellement disponible : Multimédia, avec Activer/Désactiver. Les modules des phases suivantes ne sont ni simulés ni annoncés comme opérationnels. Pas de page Pomodoro, de lanceur ou d'informations système à ce stade.

Souris : cliquer sur les onglets ; un onglet Médias désactivé consomme son clic sans ouvrir la page ni replier le panneau. Tab/Maj+Tab parcourent les onglets disponibles et les contrôles de la page ; Entrée/Espace activent le contrôle ciblé. Échap conserve le comportement local existant. L'état de focus des onglets est effacé lors d'une ouverture/fermeture pour éviter de réactiver un ancien bouton par Entrée. Le menu système fournit également Accueil/Modules, Page Médias et activation/désactivation.

## Architecture minimale

- `island-core/modules.rs` : catalogue compilé, identifiants, page active et activation. Un seul module enregistré. Aucune API Windows, ressource native ou modification de visibilité. Ajouter un module ultérieur demandera un identifiant et un adaptateur explicites, pas un chargeur de plugins ou DLL arbitraires.
- `island-ui/navigation.rs` : géométrie partagée entre dessin et hit-testing, états survol/pression/focus. Le contenu d'une seule page est dessiné. Aucun timer de navigation ni transition décorative permanente.
- `island-windows/navigation.rs` : actions de page, routage clavier, activation et sauvegarde. L'état GSMTC demeure dans media_view/Worker et n'est pas recréé lors d'un simple changement de page.
- `preferences.rs` : persistance via le writer coalescé existant ; aucun accès disque synchrone pendant une interaction UI.
- `media.rs` : activation/désactivation coalescée sur la boîte aux lettres existante, sans nouveau thread par bascule et sans joindre le worker depuis le bouton.

Les événements sont routés explicitement : une notification GSMTC met à jour les données média, jamais la page active ni la présentation. Sur Accueil, les sessions restent suivies si le module est activé, mais aucune frame ou timer de progression n'est déclenché pour elles. La miniature reste bornée selon le contrat P2. Les prochains modules auront leur état et leur adaptateur propres ; pas de bus dynamique ou de généralisation inutilisée ajouté maintenant.

## Cycle de vie et priorités

**Démarrage désactivé :** aucun worker multimédia créé. **Première activation :** création du worker MTA existant et acquisition GSMTC. **Désactivation :** l'UI vide sa vue, ferme le sélecteur et arrête les timers média ; le worker annule ses traitements asynchrones, révoque les abonnements, libère sessions/manager/miniature/cache de noms, puis attend sur sa Condvar sans échéance. Son thread déjà créé et son appartement MTA restent dormants jusqu'à réactivation ou arrêt de l'application ; les DLL Windows chargées ne sont pas explicitement déchargées. La réactivation réacquiert les sessions et repart en sélection automatique. Elle ne lance ni n'arrête la musique d'une autre application.

Des bascules très rapides sont fusionnées vers la dernière demande ; les commandes sont refusées pendant la désactivation. Les limites existantes des appels COM synchrones restent applicables. Le fournisseur silencieux teste le retrait des sessions, le refus de commandes, l'absence de nouveaux snapshots suite à une notification et la réacquisition.

Priorité d'affichage : masquage volontaire/plein écran, puis concentration, puis page choisie explicitement. Un changement de page par le menu ne réaffiche pas une encoche masquée et ne quitte pas la concentration. En mode normal visible, ce menu ouvre la page si le panneau est compact. Activer le module depuis Accueil garde Accueil ; cela ne constitue pas une demande d'ouverture automatique de Médias.

## Préférences et retour à une version précédente

Les formats v1/v2 sont toujours lus : média activé et page Médias par défaut, position et raccourcis conservés. Le format v3 ajoute `media_enabled=true|false` et `active_page=home|media` aux six clés v2. Désactivé + page Médias est normalisé vers Accueil. Valeurs inconnues, doublons et fichiers de plus de 4 Kio restent refusés.

Le writer conserve le format v1 ou v2 si les modules sont aux valeurs par défaut. Pour revenir à la Release P2 avec des préférences compatibles, activer le multimédia, choisir Médias puis quitter normalement cette version : elle réécrit le format antérieur tout en conservant position/raccourcis. Ne pas lancer un ancien binaire directement sur un fichier v3 sans cette préparation, car il ne connaît pas ce schéma. Aucun réglage global de Windows n'est modifié.

## Vérifications du 10 novembre 2026

- Sauvegarde P2 : `e7eee43`, arbre propre avant P3. Commit local P3 autorisé le 11 novembre.
- 46 tests Rust Release réussis (19 core, 5 UI, 22 Windows) : migrations v1/v2/v3, onglet désactivé, absence de recouvrement avec les contrôles média, alpha aux quatre échelles, retrait des commandes/notifications en désactivation et timer de progression arrêté sur Accueil.
- Fournisseur GSMTC synthétique étendu réussi : désactivation/réactivation, plus tous les scénarios P2 ; arrêt worker de 1 ms. Aucun lecteur tiers commandé.
- 21 contrôles natifs réussis : clavier, messages souris, popover, activation, persistance après redémarrage, priorité concentration/masquage et arrêt. Ces messages programmatiques ne remplacent pas les gestes physiques.
- Raster Accueil à 125 % inspecté ; tests à 100/125/150/200 %. DPI et multi-écrans matériels ne sont pas déclarés validés.
- Fmt, Clippy `-D warnings` et compilation Release réussis. Une première compilation a été bloquée par l'instance du projet PID 22284 ; elle a été fermée normalement, puis la compilation a réussi. Aucun processus tiers interrompu.

Preuves : `artifacts/modules-p3-20261110/` (tests, fournisseur, rasters) et `artifacts/popover-20261110-202428/` (bureau et mesures). Inventaire après banc : aucun processus du projet restant.

## Relevés courts

Accueil déplié sans popover, préférences isolées ; dimensions, visibilité, page et activation vérifiées à chaque échantillon. Version mesurée : `66699D5FBD807AE48AB80B2E634AEA30167B3FA0F1157F6676DAD1979BD9EE2C`. Ces relevés précèdent uniquement la dernière correction de remise à zéro du focus d'onglet à l'ouverture/fermeture ; aucun chiffre nouveau n'est attribué à une autre empreinte.

| Module média | Durée / conformité | CPU enregistré | Mémoire de travail moyenne | Engagement privé moyen | Handles | GDI / USER |
|---|---|---|---|---|---|---|
| Activé | 30,64 s / 30 sur 30 | Δ 0 s | 32,79 Mio | 4,46 Mio | 330–340 | 17 / 21–24 |
| Désactivé | 30,51 s / 30 sur 30 | Δ 0 s | 32,69 Mio | 4,28 Mio | 327–332 | 17 / 21 |

Δ CPU nul signifie aucun incrément enregistré, pas une absence absolue de travail. Le résident inclut des pages partagées et les caches ; désactiver ne décharge pas tous les composants Windows. Pas de preuve d'endurance ni de GPU nul. Les fluctuations USER et limites GPU/plein écran exclusif/multi-écrans des rapports précédents restent ouvertes.

## Validation manuelle avant clôture

1. Ouvrir l'encoche, passer Accueil ↔ Médias à la souris puis avec Tab/Maj+Tab/Entrée ; vérifier le sélecteur de source à droite.
2. Désactiver le multimédia : Accueil reste affiché, Médias est inactif et un clic sur cet onglet ne replie pas le panneau. La musique de votre lecteur doit continuer indépendamment.
3. Quitter puis relancer : la page et la désactivation sont conservées. Réactiver puis choisir Médias : les sessions sont retrouvées.
4. Tester Échap, réouverture puis Entrée, Alt+Tab, déplacement, concentration et plein écran. Une mise à jour musicale ne doit jamais ouvrir une page ou reprendre le focus.

Retour reçu le 11 novembre : « oui tout marche tu peux continuer ». P3 validée, commit local et P4 autorisés.

API de révocation vérifiées via Context7 : [session GSMTC](https://microsoft.github.io/windows-docs-rs/doc/windows/Media/Control/struct.GlobalSystemMediaTransportControlsSession.html), [manager GSMTC](https://microsoft.github.io/windows-docs-rs/doc/windows/Media/Control/struct.GlobalSystemMediaTransportControlsSessionManager.html). Aucun nouveau paquet ni outillage installé.

## Reprise du 11 novembre 2026

Le dernier test du correctif de focus n'avait pas été exécuté : le contrôle automatique d'autorisation avait échoué faute de quota, sans conclure que l'action était dangereuse. Après reprise, le test Release `keyboard_tests::native_activation_focus_and_local_escape` réussit (1/1), avec 21 autres tests Windows filtrés intentionnellement. Preuve : `artifacts/modules-p3-resume-20261111/keyboard-test.txt`. Aucun nouveau changement de code produit dans cette reprise ; les 46 tests et 21 contrôles antérieurs ne sont pas recomptés comme une nouvelle campagne.

Release disponible vérifiée par SHA-256 : `2A0A6211A450C3E55D1ECDEC1975E3ECCB80B54339DE6F553B1CA09847E570B0`. Elle inclut la dernière remise à zéro du focus. Les mesures du tableau restent attribuées à l'empreinte précédente, qui précède cette seule correction. Rust 1.99.1 confirmé, sans mise à jour. Aucun processus du projet actif à la reprise.

À cette reprise, P3 était prête pour les essais manuels. Le retour suivant du 11 novembre (« oui tout marche tu peux continuer ») a validé P3 : commit local `7432872`, arbre propre avant le début de P4. Voir désormais [Pomodoro/minuteurs](timers.md) et PROGRESS pour la phase active ; aucune nouvelle certification DPI/multi-écrans par ce retour global.
