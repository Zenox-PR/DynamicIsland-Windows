# Phase 0 et mode concentration — 4 novembre 2026

**Validation physique officielle reçue :** six scénarios réussis personnellement par l'utilisateur le 4 novembre 2026 : concentration, clics traversants Opera, retour normal, déplacement/Échap, Alt+Tab/plein écran, masquage/redémarrage. Aucune anomalie signalée. Le point d'arrêt avant multimédia est levé. Les limites DPI/multi-écrans physiques restent non validées. Fmt, Clippy et les 23 tests Rust ont été relancés avec succès avant le commit de cette étape. Les mentions d'attente ci-dessous décrivent la livraison initiale.

## Audit préalable

Référence vérifiée : `114a8f3`, dépôt propre avant intervention, aucun processus du projet actif. Ce commit suffit comme sauvegarde ; aucun commit vide supplémentaire. Rust stable 1.99.1 MSVC x64. Les seules dépendances externes sont windows-sys 0.61.2 et windows-link 0.2.1. Toutes les API de la phase 1 sont déjà activées : aucun ajout de dépendance, téléchargement ou installation.

Les quatre crates restent en place : island-core porte les transitions et le placement sans Windows ; island-ui calcule géométrie et pixels ; island-windows possède les HWND, DIB, focus, tray, abonnements et préférences ; island-app lance et remonte les erreurs. L'état de référence, ses sept essais manuels et ses mesures restent dans [stabilization-floating.md](stabilization-floating.md).

## Référence et choix natif

L4 est une référence ergonomique, sans copie de code ni ressources. Son [manifeste](https://github.com/Linux5Real/L4-Windows-NotchBar/blob/main/src-tauri/Cargo.toml) annonce GPL-3.0-only (blob consulté `51b0687cef55cd6b710a196b52b54f3e74a43439`). Son [hit_test.rs](https://github.com/Linux5Real/L4-Windows-NotchBar/blob/main/src-tauri/src/hit_test.rs), blob `feab2db81a29189eb7c32c8af30e95c49ce31d7f`, adapte les événements d'une WebView Tauri par un thread de sondage souris à 16/50 ms ; il compte trois clics rapprochés pour sortir du mode traversant. Le tray offre aussi une bascule. Nous retenons uniquement la possibilité de traverser l'encoche et de la restaurer volontairement.

Notre fenêtre layered emploie WS_EX_TRANSPARENT pour laisser passer les événements souris même sur les pixels visibles, conformément à [Microsoft, Layered Windows](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows). HTTRANSPARENT seul ne suffit pas pour les fenêtres d'autres threads ([WM_NCHITTEST](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nchittest)). Aucun SetLayeredWindowAttributes : la chaîne UpdateLayeredWindow/BGRA est conservée. Context7 a été interrogé ; ses résultats sur le hit-testing étant hors sujet, les pages Microsoft exactes ont été consultées directement.

## Contrat de la phase 1

- Présentation exclusive Normal / Concentration, indépendante des causes de masquage volontaire et plein écran. Priorité : masquage > concentration > panneau normal. Aucune notification future ne pourra annuler ces choix.
- Ligne 96 × 4 DIP, arrondie, centrée sur le compact mémorisé (centre vertical à 16 DIP). Position préférée inchangée. Aucun texte, pulsation, transparence animée ou nouvelle animation permanente.
- Bascule immédiate et déterministe : annule déplacement/capture, arrête l'animation en cours et retire le hook de clic extérieur ; retour au compact, jamais réouverture automatique. Les animations normales et Échap restent inchangés.
- Menu : activer/désactiver la concentration. Clic gauche tray, Afficher, Recentrer et afficher restaurent un compact interactif. Déplacer restaure d'abord le normal. Le plein écran reste prioritaire.
- Deux raccourcis optionnels indépendants : masquage existant et concentration, via RegisterHotKey/MOD_NOREPEAT. Aucun hook clavier ni triple-clic global. Conflit de concentration non bloquant signalé dans l'infobulle du tray. Le format v1 est conservé par défaut ; v2 ajoute concentration_hotkey seulement lorsqu'il est configuré, sans changer position ni raccourci existant. Concentration non persistée, démarrage normal. Pour revenir à l'ancien binaire, désactiver concentration_hotkey (none), lancer/quitter la nouvelle version pour réécrire v1, puis seulement revenir à 114a8f3. Ne pas donner un fichier v2 directement à l'ancien parseur strict.
- Les changements de DPI/écran réutilisent le placement borné existant. Aucun paramètre système changé, aucune AppBar.

## Interfaces et risques

Le cœur expose le mode de présentation et une projection de visibilité. Le moteur d'animation ne pilote que l'ouverture normale ; en concentration il est arrêté à zéro. L'adaptateur Windows synchronise le style traversant et le mode ; un échec Win32 suit l'arrêt ordonné existant plutôt que de laisser une ligne capturant les clics. Le renderer choisit une surface de ligne ou le rendu normal. Pas de nouveaux threads ni abonnements.

Régression à surveiller : capture restant active, animation obsolète réouvrant la ligne, style transparent conservé au retour, restauration supprimant le masquage plein écran, préférence de position remplacée par la géométrie de ligne, raccourcis en conflit, callbacks réentrants et icône perdue. Un test de style ne prouve pas le routage des clics : validation physique obligatoire.

Pour le futur multimédia, interface prévue seulement : modèle sans Windows côté core ; snapshots immuables et identifiants de génération pour ignorer les réponses obsolètes ; worker Windows propriétaire des objets/abonnements WinRT, file bornée et réveil par message Win32 ; rendu piloté par état visible sans ouverture automatique. Aucun binding WinRT, worker média, modèle média ou commande média n'est implémenté avant l'accord après phase 1. La version des bindings et les appartements GSMTC seront étudiés à cette étape, sans supposer windows-sys suffisant pour WinRT.

## Validation

La phase multimédia reste bloquée par le point de validation explicitement demandé dans la mission. Aucun résultat manuel du socle précédent n'est présenté comme une validation physique de cette version.

### Contrôles automatiques

- `cargo fmt --all -- --check`, Clippy workspace/all-targets avec `-D warnings`, tests workspace/all-targets Release et compilation Release : réussis, offline avec verrou de dépendances conservé. Tests de documentation : aucun doctest présent.
- 23 tests Rust réussis : 13 core, 2 UI, 8 Windows. Tous les états de visibilité, les transitions/inversions existantes, l'annulation d'un déplacement en cours, la capture, les hooks temporaires, les styles rétablis et les erreurs de HWND sont couverts. Le test natif réserve une combinaison libre pour provoquer son propre conflit de raccourci ; il libère cette réservation puis vérifie l'inscription réussie. Aucune combinaison d'une application tierce n'est supprimée.
- Géométrie/raster de la ligne testés à 100/125/150/200/300 %, y compris alpha prémultiplié et centre. Aucun changement réel de DPI Windows effectué. Le moniteur disponible est à 125 %.
- `scripts/verify-concentration.ps1` : 23/23 contrôles. Démarrage normal, pas de demande de premier plan par la bascule, interruption d'ouverture, masquages indépendants, retour tray, raccourci, plein écran du banc, déplacement armé, secours, position et redémarrage. Après échauffement puis 100 allers-retours : handles 259→259, GDI 16→16, USER 20→20, engagement privé 2 846 720→2 846 720 octets. Preuve locale : `artifacts/concentration-20261104-194904/report.json`.
- Banc flottant : 18/18 contrôles, 30 cycles, aucune AppBar, zone de travail (0,0)–(1920,1020) identique avant/pendant/après. `artifacts/floating-20261104-195053/report.json`. Raccourci de masquage existant : 5/5 contrôles (`artifacts/shortcut-20261104-195151/report.json`).
- 100 cycles raster existants / 3300 surfaces : GDI/USER 12/3 constants aux quatre points de contrôle. Dernier passage médiane 2,072 ms, p95 5,398 ms, max 8,864 ms, hors présentation et latence physique. Ce n'est pas une promesse de fréquence d'affichage ; conditions de charge différentes du rapport stable.

Les tests utilisent des messages dirigés vers nos HWND et des API réelles. **Ils ne prouvent pas le passage physique des clics vers Opera.** Les styles et les transitions sont vérifiés séparément de ce critère obligatoire.

Premier essai sandbox : échecs des tests de tray et d'écriture atomique temporaire, puis mutex de test empoisonné pour deux tests suivants. Les mêmes tests réussissent avec l'accès au bureau et au répertoire temporaire de l'environnement natif. Un avertissement de hard-link du cache incrémental est évité en fixant CARGO_INCREMENTAL=0 pour le processus de validation, sans désactiver d'avertissement de code ni modifier la configuration globale.

Binaire Release final : 269312 octets (+8192, soit +3,14 % par rapport à la référence), SHA-256 `661A0D220266AE718434CE77BA13521242D8813A60BED63AF19D59EEE37810D6`. Cargo.toml et Cargo.lock inchangés. Les bancs externes ci-dessus précèdent un dernier ajustement : le message de secours en cas d'échec de restauration du tray considère désormais aussi le raccourci concentration. Fmt, Clippy et les 23 tests Rust ont été relancés avec succès après cet ajustement, avant la recompilation finale. Leurs comportements normaux sont inchangés. La première mesure `validation-20261104-195220` a été interrompue volontairement et écartée afin de mesurer le binaire final.

### Fichiers concernés

- `crates/island-core/src/visibility.rs` : modes exclusifs, projection visible et priorités ; déplacement inchangé dans placement.rs, qui ne porte plus Visibility.
- `crates/island-ui/src/lib.rs` : dimensions et raster de ligne ; aucun changement aux dimensions/couleurs/animations normales.
- `crates/island-windows/src/concentration.rs` : styles, capture, annulation et tests natifs ; lib.rs raccorde menu/raccourci/rendu et secours tray.
- `crates/island-windows/src/preferences.rs` : lecture v1/v2 et raccourci facultatif ; surface.rs expose le dessin sans texte.
- `crates/island-windows/examples/inspect.rs`, `scripts/verify-concentration.ps1`, `scripts/measure-idle.ps1` : commande de banc, cycles et état de mesure.
- README, architecture, ce rapport, lien historique et AGENTS : périmètre, fonctionnement, limites et validation. island-app et les dépendances restent inchangés.

### Mesures

Paramètres de mesure isolés dans artifacts/, aucune préférence réelle remplacée. Même PC que la référence : Windows 11, i5-12450HX / 12 processeurs logiques, écran unique 1920 × 1080 à 125 %. Applications tierces conservées. Accord utilisateur pour le repos et annonce au début de chaque mesure. Aucune compilation pendant les collectes finales.

Compact final : 60 s de stabilisation puis 300,235 s ; 294 relevés, 294 visibles et conformes. Delta CPU 0 s détecté, moyenne/pic normalisés machine 0 % à la résolution du compteur. Résident moyen 16,12 Mio (maximum 16,41), engagement privé moyen 2,27 Mio (maximum 2,39). Handles 221–227, début 227, fin 225 ; threads 4–8 ; GDI 9 et USER 10 constants. Preuve : `artifacts/validation-20261104-195530/`.

Concentration : même processus, bascule puis 60 s de stabilisation et 300,129 s ; 296 relevés, 296 visibles et conformes, ligne 120 × 5 pixels à 125 %. Delta CPU 0 s détecté, moyenne/pic normalisés 0 % à la résolution du compteur. Résident moyen 16,32 Mio (maximum 16,34), engagement privé moyen 2,27 Mio (maximum 2,30). Handles 221–225, début 225, fin 221 ; threads 4–6 ; GDI 9 et USER 10 constants. Preuve : `artifacts/validation-20261104-200201/`. Le résident est d'environ 0,20 Mio supérieur à la collecte compacte précédente : la réduction de la surface ne garantit pas une réduction proportionnelle du working set. Aucun signe de croissance répétée dans les cycles, sans preuve absolue d'absence de fuite.

Working set privé (relevés CIM ponctuels, distincts de l'engagement privé) : compact 1 781 760→1 376 256 octets ; concentration 1 302 528→1 265 664 octets. Attribution GPU indisponible dans les deux collectes (listes vides).

Masquage : 15 s de stabilisation puis 60,860 s, 60/60 relevés conformes et aucun visible. Delta CPU 0 s détecté ; résident 16,32 Mio, engagement privé 2,24 Mio, 221 handles, 4 threads, 9 GDI / 10 USER, tous constants. Working set privé final 1 265 664 octets ; GPU non attribuable. Preuve : `artifacts/validation-20261104-200825/`. Cette collecte courte, après autorisation de reprendre le bureau, ne remplace pas une endurance de cinq minutes ou d'une heure.

Fermeture finale normale de la seule instance testée : code de sortie 0, environ 55 ms mesurées entre la demande du banc et l'attente de fin, journal WM_QUIT puis Ok(()). Aucun processus du projet restant, aucun HWND AppBar, zone de travail toujours (0,0)–(1920,1020). Preuves : `artifacts/concentration-shutdown.json` et `artifacts/concentration-idle-final.log`. Ce temps de fermeture n'est pas une mesure de latence d'interaction/rendu.

Référence `114a8f3` : compact 16,16 Mio résidents moyens, 2,30 Mio privés, CPU sans progression détectée, GDI/USER 9/10. Les écarts compacts observés sont faibles et ne démontrent pas une amélioration ; bruit d'allocation, caches, état de Windows et charge des autres applications ne sont pas identiques. Un compteur CPU sans progression ne prouve pas une consommation absolument nulle. La mesure GPU par processus ne représente pas le coût partagé de DWM ; une liste vide est une indisponibilité, pas un zéro.

### Protocole physique requis avant phase 2

Lancer `target/release/dynamic-island.exe` depuis le dossier du projet (une seule instance). Aucun raccourci concentration n'est imposé ; le menu suffit à tous les secours.

1. Dans Opera maximisé, placer le compact sur une zone d'onglets avec Ctrl + glisser. Activer la concentration depuis le menu de l'icône système : une ligne fine reste au centre du compact, aucun espace AppBar.
2. Cliquer sur les onglets situés derrière le centre et les deux extrémités de la ligne, puis juste autour. L'onglet doit recevoir le clic, sans ouverture de l'encoche. Vérifier aussi clic droit/molette lorsque le contrôle derrière les accepte. Un clic reçu ailleurs que derrière les pixels visibles ne suffit pas à valider la traversée.
3. Menu tray → Désactiver : compact à la même position, clic pour ouvrir/replier, Échap y compris pendant l'ouverture. Réactiver et tester aussi clic gauche tray, Afficher, puis Recentrer et afficher (ce dernier recentre volontairement).
4. Déplacer avant/après concentration, puis choisir Déplacer pendant la concentration : retour au compact, prochain glissement sans Ctrl ; Échap annule. Vérifier le survol sans activation et Alt+Tab vers Opera.
5. Pendant l'ouverture, activer la concentration depuis le tray : pas de panneau restant, pas de capture ; cliquer immédiatement dans Opera. Répéter plusieurs fois.
6. Avec la ligne active, passer une application en véritable plein écran puis revenir : ligne masquée puis restaurée. Masquer volontairement avant le plein écran : elle doit rester masquée à la sortie. Afficher depuis le tray rétablit le normal dès qu'aucun plein écran n'est prioritaire.
7. Optionnel : quitter, conserver toutes les valeurs existantes dans settings.ini, passer à `version=2` et ajouter `concentration_hotkey=Ctrl+Shift+F9`. Relancer, essayer la combinaison depuis Opera, aller-retour sans ouverture automatique. Un conflit est indiqué par l'infobulle ; le menu doit rester utilisable. Utiliser deux combinaisons distinctes pour hotkey et concentration_hotkey.
8. Quitter depuis la concentration, puis relancer : compact normal, position conservée, pas de seconde instance ni icône résiduelle.
9. DPI/multi-écrans : seulement si du matériel approprié est disponible ; déplacer vers un écran déjà configuré à une autre échelle, sans modifier les réglages Windows. Sinon déclarer non testé.

Consigner séparément chaque résultat, en particulier les clics réels derrière la ligne et le focus. L'application de test est arrêtée en fin de mesures ; la relancer pour ces essais.

### Limites et état Git

Les trois abonnements WinEvent du socle restent nécessaires au plein écran événementiel ; aucun hook d'entrée global permanent ni surveillance de souris n'est ajouté. Le hook extérieur normal existe seulement lorsque le panneau est ouvert et est retiré en concentration. GPU/DWM partagés, plein écran exclusif, vrai changement de DPI/multi-écrans, accessibilité du dessin et restauration après redémarrage réel d'Explorer restent limités par le matériel et les essais autorisés. Pas de crash forcé ni de redémarrage Explorer effectué.

Audit phase 0 enregistré localement dans `a113e64`. Référence stable `114a8f3` intacte. Le code de phase 1, le README et les résultats restent dans le répertoire de travail pour validation physique ; leur commit d'étape attend ce retour, conformément à la demande de commits pour les étapes validées. Aucune modification utilisateur préexistante n'était présente et aucune n'a été écrasée. Aucune publication. Les mesures musique/vidéo ne sont pas commencées ; elles dépendent de la phase 2, expressément conditionnée à l'accord utilisateur après cette phase.
