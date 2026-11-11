# P1 — Refonte native Noir satiné, 10 novembre 2026

## Périmètre et architecture minimale

Conserver 172 × 32 DIP compact, 96 × 4 concentration et 384 × 168 déplié. Raffiner le contenu : marges de 24 DIP, pochette plus discrète, titre 17–18 DIP, secondaire 12, source 11, progression fine et commandes équilibrées. Couleurs centralisées, blanc doux sur noir/anthracite, accent neutre remplaçable ultérieurement. Aucun fichier de police ou image propriétaire.

Le sélecteur est un popover Win32 modeless séparé, layered/toolwindow/topmost, dessiné dans une DIB. Il utilise la boucle UI existante et une liste bornée de choix par ID stable. Pas de boucle modale ajoutée, pas de polling, pas de hook supplémentaire. Les notifications vers le HWND principal sont postées sans pointeur et validées par HWND source, pour éviter les emprunts réentrants et les messages d'un ancien popover.

Focus seulement à l'ouverture explicitement demandée. Échap ferme d'abord le popover ; clic extérieur/désactivation ferme sans réactiver une application tierce. La sélection utilise l'ID capturé, jamais un numéro de ligne devenu périmé. Les changements GSMTC mettent à jour la liste ; suppression d'une session annule sa pression/sélection clavier si nécessaire. Navigation flèches/Tab/Entrée, défilement pour les listes longues. Bornage dans la zone de travail du moniteur ; fermeture/recréation au changement DPI/écran plutôt que conserver un rectangle périmé. Aucun changement de zone de travail.

Nom affiché distinct de l'AUMID : résolution Shell AppsFolder / IShellItem.GetDisplayName sur le worker existant, cache borné ; fallback explicite « Lecteur Windows » si inconnu. Identifiant technique conservé dans Session.source et dans le diagnostic en lecture seule, pas de titres ajoutés aux journaux. Pas de scan de fichiers personnels ni de recherche réseau.

Animations courtes, timers uniquement pendant leur durée ; alpha natif du popover, survol/pression locaux via WM_MOUSEMOVE/WM_MOUSELEAVE. Pas d'effets blur/Mica, pas de rendu 60 Hz permanent. Réutiliser les ressources graphiques et respecter le réglage de réduction d'animation. Priorité plein écran/masquage/concentration préservée ; aucun événement média n'ouvre le sélecteur.

## Références vérifiées

Context7 : `/websites/microsoft_github_io_windows-docs-rs_doc_windows` pour [SHCreateItemInKnownFolder](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Shell/fn.SHCreateItemInKnownFolder.html) et [IShellItem.GetDisplayName](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Shell/struct.IShellItem.html). `/microsoft/windows-rs` pour la boucle de messages et UpdateLayeredWindow ; certains exemples utilisent une branche plus récente, les signatures utilisées doivent être confirmées dans les bindings locaux verrouillés 0.62.2/0.61.2.

## Contrôles à effectuer

Logique pure : choix stable, disparition pendant pression, navigation/défilement, coins transparents, titres longs et ratios. Natif : vie du HWND, focus/Échap, activation seulement explicite, fermeture hors panneau, changement d'état, ressources après cycles, arrêt. Raster réel exporté à 100/125/150/200 %. Fmt/Clippy/tests/Release après modifications, sans répéter les longues collectes historiques ; mesures courtes nouvelles clairement identifiées. Validation physique utilisateur obligatoire avant P2.

## Vérifications réalisées pendant cette reprise

- 38 tests Rust Release : 17 core, 4 UI, 17 Windows ; fmt et Clippy sans avertissement. Les tests natifs nécessitent l'accès au bureau et au répertoire temporaire : un passage isolé a échoué sur le tray et l'écriture de préférences, puis les 38 tests ont réussi dans l'environnement autorisé. Aucun avertissement masqué.
- Rasters du lecteur et du sélecteur inspectés ; contrôles de géométrie/transparence à 100/125/150/200 %. 100 créations/destructions de popover dans le test unitaire conservent les compteurs GDI/USER. Ce test ne prouve pas le focus ou les clics physiques.
- Le diagnostic GSMTC réel résout la source Opera en « Navigateur Opera GX » (métadonnées Shell), avec titre présent, miniature, progression et capacités lecture/pause. Aucun titre de média journalisé. Arrêt du worker : 2 ms dans cet essai.
- Banc SMTC silencieux : détection, sélection, commandes vers sa propre session, miniature, progression, 30 changements rapides, rejet des générations périmées, deux sessions musique/vidéo, disparition et rejet de commande vers session retirée : réussis. Arrêt : 2 ms. Cela ne certifie pas tous les sites ou lecteurs.
- Premier essai du bureau masqué par le plein écran, puis réouverture synthétique refusée par la protection du premier plan. Les essais concernés sont conservés comme non concluants. Le banc peut désormais accorder le premier plan au seul PID du prototype, via [AllowSetForegroundWindow](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/WindowsAndMessaging/fn.AllowSetForegroundWindow.html). Le produit ne contient pas ce mécanisme et respecte le refus de Windows.

## Limites et validation physique demandée

Le contenu média apparaît à la fin de la transition géométrique ; le fond s'anime sans afficher les anciens libellés du prototype. La disparition du popover est immédiate, son apparition utilise un fondu borné. Les titres restent ellipsés dans une largeur fixe. L'accent est centralisé dans le code, pas encore exposé dans des paramètres utilisateur (phase dédiée ultérieure).

DPI dynamique, passage entre plusieurs écrans, lecteurs d'écran/UI Automation et jeux exclusifs ne sont pas validés. La résolution Shell reste un appel synchrone hors UI ; aucun temps maximal garanti en cas de blocage d'un composant système. Les mesures courtes ne remplacent pas une endurance de plusieurs heures et n'isolent pas la part GPU de DWM.

Après lancement de target/release/dynamic-island.exe :
1. Ouvrir le panneau sur un média réel : nom de source, miniature proportionnelle, ellipses, survol et boutons.
2. Cliquer sur la source : fond sombre, session et coche cohérentes ; flèches/Tab/Entrée puis Échap. Le premier Échap ferme le sélecteur, le suivant replie le panneau.
3. Cliquer dans une autre application : fermeture du sélecteur sans détourner le clic. Tester Alt+Tab, Échap pendant l'ouverture et déplacement du panneau.
4. Ouvrir le sélecteur depuis le tray en concentration, puis le fermer : la ligne doit conserver ses clics traversants. Vérifier masquage, plein écran et retour.
5. Si deux lecteurs sont déjà disponibles, sélectionner chacun puis fermer le lecteur sélectionné ; aucune commande ne doit partir vers un autre lecteur par erreur. La validation complète des fournisseurs relève ensuite de P2, après accord.

Aucun commit final et aucun passage automatique à P2.

## Bilan des essais du bureau et mesures Release

Binaire mesuré : SHA-256 `2028144030EE14566D2E73517C93440B3E41798E842E40478D7BF8091E4A9CCB`. Preuves : `artifacts/popover-20261110-194742/` (mesures) et `artifacts/popover-20261110-195159/` (60 cycles). Les préférences utilisées sont isolées de celles de l'utilisateur. Aucun processus du projet restant à l'inventaire final.

Dix contrôles fonctionnels natifs réussis : ouverture explicite avec premier plan, Échap, sélection automatique au clavier, désactivation sans reprise du focus, fermeture à l'entrée en concentration, accès explicite depuis la concentration, Échap conservant la ligne, masquage volontaire, fermeture sur notification de changement d'écran, arrêt normal. La notification d'écran est envoyée à notre HWND seulement : ce n'est pas un changement DPI matériel.

Le critère d'égalité stricte GDI/USER du banc **échoue** : premier essai 17/24 → 17/25 après 20 cycles ; second essai après stabilisation 17/23 → 17/20 après 60 cycles. Les 60 relevés intermédiaires gardent GDI=17, USER varie de 21 à 24, handles de 328 à 339 ; avant/après, handles 338 → 328. Les compteurs USER diminuent aussi au repos. Aucune accumulation persistante observée ; cela ne permet pas de certifier une absence de fuite à long terme. L'origine exacte des fluctuations USER du processus complet n'est pas isolée. Le test unitaire isolant les ressources du popover réussit, lui, l'égalité stricte sur 100 cycles. Le script visible conserve son code de sortie 1 et les rapports bruts : le critère échoué n'est pas effacé.

| État | Observation retenue | CPU du processus | Mémoire de travail moyenne | Engagement privé moyen | Handles | GDI / USER |
|---|---|---|---|---|---|---|
| Compact | 60/60 échantillons conformes, 60,92 s | Δ 0 s enregistré | 32,18 Mio | 4,10 Mio | 327–342 | 17 / 21–24 |
| Concentration | 60/60 conformes, 60,89 s | Δ 0 s enregistré | 32,05 Mio | 3,95 Mio | 321–329 | 17 / 19–21 |
| Déplié + sélecteur | 43/60 conformes seulement | Sur l'intervalle conforme de 42,62 s : Δ 0,28125 s, soit 0,66 % d'un cœur | 33,79 Mio sur les échantillons conformes | 4,77 Mio | 323 | 23 / 23 |

Le dernier intervalle est **partiel**, le sélecteur s'étant fermé avant la fin. La moyenne des 60 secondes mélangées n'est pas présentée comme une mesure stricte dépliée. Les deux premiers Δ CPU nuls signifient « aucun incrément enregistré à la résolution du compteur », pas une absence absolue de travail. La mémoire de travail inclut les pages partagées et ne représente pas la RAM privée. Session Opera en lecture observée avant les relevés ; les applications tierces restent ouvertes et leur activité n'est pas contrôlée. GPU non mesuré dans cette reprise. Pas de comparaison normalisée avec l'ancienne mesure d'environ 29 Mio, qui diffère en état et durée.

Phase 1 implémentée, avec ces limites de validation ; pas déclarée terminée. Prochaine étape : essais physiques ci-dessus et retour utilisateur avant tout commit final ou nouvelle phase.

Retour du 10 novembre 2026 : l'utilisateur confirme « tout est bon tu peux continuer ». P1 validée manuellement dans ce périmètre ; limites ci-dessus maintenues. La finalisation multimédia P2 est autorisée, sans validation implicite de tous les fournisseurs.
