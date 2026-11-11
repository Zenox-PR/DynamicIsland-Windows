# Échap pendant l'ouverture — correctif et validation

3 novembre 2026. L'utilisateur a validé 5 des 6 essais manuels : le fonctionnement principal, le clavier après ouverture et les comportements de focus. Le seul échec signalé concerne Échap pendant l'ouverture. La refonte graphique reste en attente d'une validation distincte.

## Diagnostic et portée de la preuve

La procédure Win32 protégeait App avec RefCell::try_borrow_mut. Un message imbriqué reçu pendant un emprunt de l'état (notamment un appel natif de rendu) tombait sur DefWindowProc sans exécuter l'action Échap. Une fois le traitement fini, le même message fonctionnait. Un test reproduit cette perte en envoyant WM_KEYDOWN/VK_ESCAPE à notre fenêtre possédant le focus pendant un emprunt de l'état d'ouverture.

Ce nouveau test a été exécuté sur le code précédent et a échoué avec « Escape during a native callback must not be lost ». Il passe après correction. Il s'agit d'une réentrance synthétique réelle dans la procédure native, pas d'une capture de l'appui physique de l'utilisateur. Les messages clavier ordinaires sont normalement mis en file ; on ne prétend donc pas avoir établi que cette réentrance est l'unique cause de l'observation sur son bureau. Une confirmation manuelle reste nécessaire.

Une seconde faiblesse pouvait rendre un repli précoce peu réactif : toute inversion reprenait 240 ms, quelle que soit la distance restante. Des commandes répétées pouvaient aussi redémarrer une animation vers une cible inchangée.

Référence : [documentation Microsoft windows-window sur la réentrance](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-window.md), consultée via Context7. Le projet continue d'utiliser windows-sys 0.61.2, sans ajouter windows-window.

## Correction livrée

- Échap est reconnu avant l'emprunt de l'état, uniquement lorsqu'il est livré à notre fenêtre avec son focus clavier. Aucun hook clavier, raccourci global ou interrogation des touches d'autres applications.
- Une Cell<bool> locale au thread mémorise une demande de fermeture. Les demandes simultanées se regroupent en un seul bit : aucune file croissante ou allocation.
- La demande est consommée dès que l'état redevient disponible, au retour du callback et après DispatchMessageW. Aucun timer, thread ou polling supplémentaire.
- Le repli est engagé avant la restitution du focus, pour qu'un appel d'activation n'en diffère pas le changement de cible. La restitution garde les protections existantes concernant le premier plan et la validité de la fenêtre précédente.
- La progression est échantillonnée au moment de l'inversion : continuité de position, pas de remise à zéro. La nouvelle durée vaut 240 ms × distance normalisée restante. Une fermeture après 60 ms d'ouverture part de 15,625 % et dure donc 37,5 ms, hors cadence du timer.
- Une demande répétée vers la même cible ne repousse plus l'échéance. Les animations complètes conservent 240 ms et la courbe smoothstep existante. La continuité de vitesse à l'inversion n'est pas garantie ; aucun ressort ni nouveau style d'animation n'est introduit dans cette correction.
- L'ouverture au clic, le second clic, le focus explicite, Alt+Tab, le survol sans activation, le dessin et la transparence ne sont pas remplacés.

Fichiers de production modifiés : crates/island-core/src/lib.rs et crates/island-windows/src/lib.rs. Aucun changement dans island-ui ni dans le dessin des surfaces.

## Tests

- fmt : réussi.
- Clippy workspace/all-targets, -D warnings : réussi.
- Tests workspace/all-targets : **10 réussis**, aucun échec final.
- Doctests : réussis (aucun exemple exécutable).
- Tests natifs Release : réussis ; le test clavier/réentrance a été réexécuté avec succès sur le code final après le placement de la restitution du focus après le changement de cible.
- Nouvelle compilation Release : réussie.

Les tests du cœur couvrent maintenant les inversions à 0, 1, 15, 60, 120 et 239 ms, la continuité de position, le retour sans dépassement, l'échéance malgré les demandes répétées et 100 inversions alternées. Les tests précédents de réduction des animations, DPI et alpha continuent de passer. Le test natif vérifie aussi le focus, le refus d'Échap sans focus et le repli au clic.

Le test de ressources du moteur garde ses compteurs après 100 cycles : GDI 1 et USER 0, dans le processus de test sans fenêtre affichée. Sur la première compilation Release du correctif : p95 de génération de surface 1,963 ms, maximum 4,575 ms, pour 3 300 dessins. La variation par rapport aux relevés précédents n'est pas un benchmark comparatif contrôlé. Ce test ne mesure pas la latence visuelle ou DWM. Aucune nouvelle campagne CPU/RAM de cinq minutes n'est attribuée à cette version.

## Validation manuelle ciblée

Lancer le nouveau target/release/dynamic-island.exe, puis ouvrir au clic et appuyer sur Échap dès le début de l'élargissement. Répéter plusieurs fois à différents instants, y compris à mi-ouverture et en maintenant brièvement Échap. Le panneau doit revenir à sa taille compacte sans terminer d'abord toute l'ouverture, sans saut de taille et sans rester actif indéfiniment. Recontrôler un second clic et Alt+Tab, puis Quitter. Si le problème physique persiste, conserver son statut ouvert et relever la réception de la touche et le focus sur le bureau ; ne pas ajouter de hook global pour le contourner.

## Binaire livré

SHA-256 final : `EF9BC14E8D74F5DF5AD9979068431368F17E93B2A4DF1035F4EDC96CE27BBDFB`. Le démarrage a été contrôlé puis WM_CLOSE demandé : arrêt avec code 0, aucun processus du projet restant. Preuves locales : artifacts/escape-animation-final.json et artifacts/escape-animation-final.log. Le contrôle s'est fait avec l'encoche masquée par une application plein écran, sans manipulation des applications de l'utilisateur.
