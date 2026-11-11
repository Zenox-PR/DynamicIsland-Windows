# P2 — Finalisation multimédia, 10 novembre 2026

## Autorisation et point de départ

Après « tout est bon tu peux continuer », la refonte P1 est validée et enregistrée localement dans `dec6186`. Ce commit inclut le socle GSMTC préexistant, nécessaire à la refonte, sans le déclarer universellement validé. L'arbre était propre immédiatement après le commit. La présente P2 reste non commitée, en attente de validation manuelle finale. Aucun travail P3.

## Corrections ciblées

1. **Commande devenue périmée pendant un traitement.** L'identité/génération de session était contrôlée, mais un événement arrivé après le vidage de la boîte aux lettres pouvait encore attendre son traitement. Chaque commande porte désormais aussi l'époque des événements. Juste avant l'appel au fournisseur, le worker vérifie le lecteur sélectionné, l'époque et l'absence d'une invalidation en attente. Les capacités sont toujours relues. Aucun verrou n'est conservé pendant l'appel COM et aucune commande refusée n'est redirigée vers un autre lecteur.
2. **Anciennes informations pendant un rechargement.** Le worker retire titre, artiste, album, miniature, progression et commandes de la piste invalidée, puis publie cet état avant les lectures asynchrones. L'identité et le nom de la source restent conservés. Les nouveaux champs sont affectés ensemble après lecture complète et contrôle de fraîcheur ; un échec ne conserve pas un titre partiellement renouvelé. L'UI peut afficher brièvement ses libellés d'indisponibilité existants pendant ce chargement.
3. **Résultat asynchrone et miniature.** Nouvelle vérification de fraîcheur après GetResults ; Actualiser invalide aussi le travail antérieur. Le contrôle de cohérence de la miniature compare maintenant l'album en plus du titre et de l'artiste. Les bornes d'image, l'annulation et le cache d'une miniature sont conservés.

Le moteur conserve son worker MTA, sa boîte aux lettres bornée et ses abonnements. Aucun nouveau thread, timer, hook, dépendance ou réglage Windows. Aucun changement de rendu, de dimensions ou de mode concentration. Un snapshot supplémentaire, remplaçable dans la même boîte aux lettres, est publié lors d'une invalidation ; aucune publication périodique n'est ajoutée au repos.

## Résultats vérifiés

Release MSVC x64 : `6933670FE9E38439DB599BD8CB1493902CD6A90619F8AB5E63594D9DB2876EF3` (SHA-256). Preuves locales : `artifacts/media-p2-20261110/`.

- Cargo fmt réussi ; Clippy workspace/all-targets avec `-D warnings` réussi ; build workspace/all-targets Release réussi, sans installation ni mise à jour.
- **41 tests Rust Release : 18 core, 4 UI, 19 Windows, aucun échec.** Trois tests supplémentaires vérifient l'invalidation du contenu tout en gardant l'identité, les commandes après événements concurrents et l'interruption d'un résultat asynchrone périmé. Les tests existants de timers de progression, concentration, Échap, transparence et ressources sont conservés et passent.
- Banc GSMTC réel avec fournisseur silencieux propre au projet : détection, sélection, timeline/miniature, pause/reprise, suivant, 30 changements rapides, commande périmée, pochette/artiste absents, commande Suivant désactivée, deux sessions musique/vidéo, disparition, rejet d'une commande vers une session retirée, puis réapparition de la même source avec une nouvelle identité : réussis. Les commandes ciblent exclusivement le fournisseur de test.
- Diagnostic de huit secondes en lecture seule : Opera GX détecté, nom Shell lisible, titre présent, miniature décodée, lecture et timeline disponibles. Suivant était disponible dans cette session, Précédent non disponible. Cela décrit la session observée, pas une compatibilité certifiée avec un site particulier. Aucun titre personnel journalisé ni commande envoyée à Opera.
- Arrêt worker observé : 2 ms dans le banc et le diagnostic. Inventaire final : aucun processus du projet restant. Aucun arrêt forcé.

## Performances et limites conservées

Les collectes CPU/RAM P1 restent des références historiques, pas des mesures de cette nouvelle empreinte Release. Elles ne sont pas relancées systématiquement pour économiser le quota : le chemin au repos et les timers n'ont pas été modifiés, et leurs conditions d'activation sont testées. Aucun chiffre de performance nouveau n'est supposé. GPU non mesuré ; dernière mesure dépliée partielle et fluctuations USER toujours documentées dans visual-refresh.md. Une endurance longue reste utile avant distribution.

Un événement peut arriver juste après le dernier contrôle ou le fournisseur peut changer sans notifier Windows : GSMTC ne rend pas la vérification et l'exécution atomiques. La protection est conservatrice ; une modification d'une autre session peut faire refuser une commande concomitante, à renouveler explicitement. Les appels synchrones Windows et l'ordre de traitement des sessions restent des limites de latence ; pas de garantie de délai global en cas de broker bloqué. Un composant Windows qui ignore Cancel peut poursuivre son travail interne, même si son résultat est ignoré.

Une durée inconnue ne produit pas de progression inventée. Les sources identiques ambiguës restent non associées arbitrairement. Un lecteur sans GSMTC reste absent ; aucune inspection d'onglet ni extension. Absence de toutes les sessions testée dans la logique pure, pas sur le bureau réel puisque la session Opera n'a pas été interrompue. DPI dynamique, multi-écrans physiques, jeux exclusifs, veille/verrouillage, UI Automation et redémarrage perturbateur d'Explorer ne sont pas certifiés.

## Protocole manuel de clôture P2

Lancer `target/release/dynamic-island.exe`, sans lancer de seconde instance. Utiliser les applications déjà installées ; une session non exposée par Windows doit être notée « non disponible », pas « réussie ».

| Essai | Vérification attendue | État |
|---|---|---|
| Opera + YouTube | Titre, pochette, progression, pause/reprise ; changements rapides sans ancienne pochette persistante | À confirmer |
| Opera + vidéo HTML5 | Lecteur habituel de film/série/animé ; session et commandes selon capacités réellement exposées | À confirmer |
| Autre lecteur compatible | Application ou navigateur déjà présent ; informations et commandes disponibles | À confirmer |
| Deux lecteurs simultanés | Choisir chacun puis Automatique ; aucune commande vers le mauvais lecteur, choix manuel conservé pendant une pause | À confirmer avec lecteurs réels |
| Source retirée/renouvelée | Fermer l'onglet de test ou le lecteur choisi, puis le rouvrir ; ancienne session et ancien choix retirés, nouvelle sélection possible | À confirmer avec lecteurs réels |
| Données manquantes | Sans pochette ou durée, remplacement propre ; bouton indisponible inactif | Synthétique réussi, réel selon disponibilité |
| Concentration/plein écran | Pas d'ouverture ou de prise de focus lors des changements de piste ; retour correct, Échap et Alt+Tab préservés | À confirmer sur cette Release |
| Quitter/redémarrer | Arrêt normal, position conservée, reconstruction des sessions et sélection automatique | À confirmer sur cette Release |

La validation P1 générale ne remplace pas ces scénarios multimédias. Attendre le retour utilisateur avant de déclarer P2 terminée, créer son commit final ou commencer P3.

## Références API

Context7 et bindings verrouillés utilisés pour vérifier [les événements et méthodes GSMTC](https://microsoft.github.io/windows-docs-rs/doc/windows/Media/Control/struct.GlobalSystemMediaTransportControlsSession.html) et [ClearAll/Update du fournisseur de test](https://microsoft.github.io/windows-docs-rs/doc/windows/Media/struct.SystemMediaTransportControlsDisplayUpdater.html). Les garanties de fraîcheur ci-dessus sont des contrôles de notre code, pas une garantie d'atomicité offerte par Windows.

Retour utilisateur du 10 novembre 2026 : « tu peux continuer ». Accord global de clôture P2 et poursuite P3 enregistré ; aucun détail supplémentaire par lecteur communiqué. Les scénarios non documentés restent non certifiés. Commit local P2 autorisé.
