# P4 — Pomodoro et minuteurs

Autorisation du 11 novembre 2026 après validation P3 (« oui tout marche tu peux continuer »). Sauvegarde P3 : `7432872`, arbre propre avant P4. Retour du 11 novembre « tout est bien tu peux continuer » : validation globale P4, commit local et début P5 autorisés. Aucun détail de retest individuel supplémentaire communiqué. Le module est indépendant du **mode concentration visuel** déjà validé.

## Utilisation

Ouvrir l'encoche, choisir **Minuteurs**, puis **Minuteur** ou **Pomodoro**. Le menu de l'icône système donne aussi accès à cette page. Démarrer, Pause, Reprendre et Réinitialiser sont accessibles au clic et par Tab/Maj+Tab puis Entrée/Espace. Échap replie le panneau ; il n'annule pas le décompte.

**Durées…**, accessible avant démarrage ou après la fin, ouvre un menu Windows natif. Les quatre durées sont réglables séparément de 1 à 240 minutes : préréglages et ajustements ±1/5 minute. Pendant une session active ou en pause, il faut Réinitialiser avant de changer de mode ou de durée. Réinitialiser remet aussi le cycle Pomodoro à zéro. Ce menu suit le thème Windows ; le panneau et ses boutons gardent Noir satiné. Le sélecteur GSMTC conserve son popover personnalisé.

Par défaut : minuteur 25 min ; travail 25 min ; pause courte 5 min ; pause longue 15 min. Après chaque travail terminé, le bouton propose une pause courte, ou une pause longue après quatre travaux. Après une pause, il propose le travail suivant. **Chaque étape attend un démarrage explicite** : une longue absence ne lance ni ne comptabilise des sessions que l'utilisateur n'a pas commencées.

Le compteur utilise l'arrondi supérieur à la seconde ; 00:00 n'apparaît qu'à l'échéance. La progression est calculée à partir du temps restant. Le compteur de travaux terminés dans le cycle est affiché sur quatre.

Changer de page, replier, masquer, passer en concentration visuelle ou en plein écran laisse le décompte actif. Désactiver le module depuis Accueil ou l'icône système le **met en pause** ; le réactiver conserve cette pause jusqu'à Reprendre. Aucun changement automatique de page, expansion, activation ou restauration de fenêtre à la fin.

## Horloge et veille

`island-core::timer::Timer` reçoit des millisecondes monotones injectées. Il ne lit aucune horloge et n'additionne jamais les ticks d'affichage. L'adaptateur utilise `GetTickCount64`, incluant veille/hibernation et indépendant des corrections de l'heure civile. Windows peut retarder la livraison d'un message ; le prochain traitement recalcule l'état depuis l'échéance réelle.

La veille compte comme temps écoulé. Si l'échéance est passée au réveil, une seule fin est produite, sans enchaînement de phases. `WM_POWERBROADCAST` déclenche une réconciliation. Un indicateur coalescé conserve les messages reçus pendant un appel Win32 réentrant (menu), jusqu'à la libération de l'état UI. Aucun réveil forcé du PC, verrou de veille, modification d'heure ou test de mise en veille automatique.

## Ressources et notification

- Aucun thread supplémentaire, hook ou nouvelle dépendance. Seule la feature `Win32_System_SystemInformation` de `windows-sys` déjà installé est ajoutée.
- Un timer Win32 privé sur le HWND principal, supprimé puis réarmé à la prochaine échéance utile. Identifiant distinct de l'animation, du média et de la récupération de l'icône.
- Page visible et dépliée : au plus un changement de seconde, rendu de la surface à la demande. Les animations d'ouverture/fermeture gardent leur cadence finie existante.
- Compact, concentration, masqué ou autre page : échéance finale uniquement. Aucune interrogation par seconde hors de la page.
- Prêt, en pause, terminé ou désactivé : aucun timer Pomodoro actif. `KillTimer` dans le nettoyage normal ; les messages anciens reconsultent l'état et ne redémarrent rien.
- Fin : notification silencieuse via l'icône existante (`NIM_MODIFY`, `NIF_INFO`, `NIF_REALTIME`, `NIIF_NOSOUND`, `NIIF_RESPECT_QUIET_TIME`). Windows reste libre de ne pas l'afficher ; **Terminé** reste dans le panneau. La copie temporaire des données de notification évite un renvoi lors de la restauration de l'icône. Aucun abonnement aux notifications des autres applications.
- Pas de journal permanent par seconde. Le diagnostic optionnel du banc trace les actions et la fin uniquement.

## Préférences et arrêt

Schéma v4 lorsque nécessaire, lecture v1/v2/v3 conservée. Clés ajoutées : `timer_enabled`, `timer_mode=timer|pomodoro`, `timer_minutes`, `work_minutes`, `short_minutes`, `long_minutes` ; `active_page=timer` ajouté. Les bornes et champs requis sont contrôlés, le fichier reste limité à 4 Kio et sauvegardé par le writer dormant existant. Position et raccourcis sont conservés.

Durées, mode, activation et page survivent au redémarrage. **Un décompte en cours n'est pas repris après Quitter** : le lancement suivant est Prêt, sans notification d'une ancienne session. La conservation de l'état requise entre pages est en mémoire, indépendante des préférences persistantes. Il n'y a pas d'exécution en arrière-plan après fermeture ni d'autostart.

Avant retour à un ancien binaire : sauvegarder son fichier de préférences. Pour retrouver le schéma P3, rétablir les durées 25/25/5/15, le mode Minuteur, le module activé et la page Accueil ou Médias, puis quitter normalement. Un ancien binaire ne sait pas lire v4.

## Vérifications et limites

Première campagne : 54 tests Rust Release réussis (23 core, 6 UI, 25 Windows), dont temps simulé, quatre travaux/pauses, messages anciens, activation/désactivation, migrations strictes et transparence à 100/125/150/200 %. Test additionnel du clic Pause tardif réussi : une touche/clic destiné à Pause ne doit pas commencer la phase suivante si la fin est survenue entre deux images. Fmt, Clippy `-D warnings`, compilation Release disponibles dans `artifacts/timers-p4-20261111/`.

Rasters inspectés à 125 % : boutons/typographie lisibles, progression et coins transparents. Cela ne valide pas des transitions DPI physiques.

Dernier correctif de réentrance : 25 tests Windows repassés avec succès, Release finale compilée. Le banc obtient 32/33 contrôles ; les douze contrôles propres aux minuteurs réussissent. L'assertion combinant fermeture du popover média et retour du premier plan au banc a échoué : cause non isolée, contrôle manuel requis. Un premier essai s'était arrêté sur un refus d'activation Windows. Aucun test automatique ne certifie les gestes réels utilisateur, la présence effective d'une notification visuelle, la veille matérielle, tous les navigateurs, le multi-écrans ou le plein écran exclusif. Aucun test perturbateur d'Explorer.

Les trois relevés de 60 s sont détaillés dans [PROGRESS](PROGRESS.md) avec leur empreinte de binaire (avant le dernier correctif de réentrance) : compact/concentration sans incrément CPU détecté, déplié 0,53125 s CPU sur 60,95 s, mémoire de travail moyenne 32,03–32,85 Mio. 20 cycles sans augmentation finale GDI/USER/handles. Ces relevés courts ne valident pas une endurance de cinq minutes et ne sont pas attribués artificiellement au binaire final. Aucun processus résiduel après fermeture normale.

## Validation manuelle demandée avant clôture

1. Choisir Minuteurs, régler Minuteur sur 1 min dans Durées, démarrer ; vérifier Pause, attente, Reprendre, Réinitialiser et le clavier Tab/Entrée/Échap.
2. Relancer 1 min, passer aux Médias puis replier/concentration ; revenir avant la fin : le temps a continué. Attendre la fin : une notification silencieuse si Windows l'autorise, Terminé conservé, aucun focus volé.
3. Démarrer, désactiver le module depuis Accueil, attendre, réactiver : il reste en pause. Reprendre puis réinitialiser.
4. Régler des durées courtes Pomodoro, vérifier Travail → fin → Pause courte uniquement après clic, puis Travail suivant ; la quatrième fin de travail doit proposer Pause longue (le cycle complet est aussi couvert par tests déterministes).
5. Vérifier que les durées choisies persistent après Quitter/relancer, sans reprise automatique du décompte ; vérifier les interactions multimédia, masquage et concentration déjà validées. **Ouvrir le sélecteur de lecteurs puis Alt+Tab ou cliquer à l'extérieur : il doit se fermer sans reprendre le focus.** Ce dernier essai doit lever le contrôle combiné non concluant du banc.
6. À l'occasion d'une veille normale volontaire, lancer un minuteur qui expire pendant cette veille ; au réveil, il doit être terminé sans série de notifications. Ce contrôle matériel est facultatif pour la présente manipulation et reste non validé tant qu'il n'est pas réalisé.

## Sources techniques

Documentation consultée via Context7, complétée par les pages Microsoft : [temps d'interruption et veille](https://learn.microsoft.com/en-us/windows/win32/sysinfo/interrupt-time), [GetTickCount64](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount64), [SetTimer](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-settimer), [KillTimer et messages déjà en file](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-killtimer), [NOTIFYICONDATAW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyicondataw). Les caractéristiques d'économie de ressources sont des choix de conception ; seuls les relevés nommés dans PROGRESS sont des mesures.
