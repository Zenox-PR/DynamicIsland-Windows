# Validation du prototype — protocole prévu

Statut : protocole de référence, après autorisation de B et de la reprise des vérifications. Voir [les résultats du prototype](prototype-report.md) pour les essais réellement effectués. Les scénarios ci-dessous ne sont pas tous validés.

## Compilation et analyse

Après création du workspace : formatage, Clippy sur les cibles pertinentes, tests unitaires ciblés et build release. Consigner la version Rust, les dépendances verrouillées, les commandes, codes de sortie et avertissements. Ne pas supprimer globalement les avertissements pour obtenir un résultat favorable.

## Tests fonctionnels

| Scénario | Critère attendu |
|---|---|
| Démarrage | Une seule encoche, au bon emplacement, sans console parasite |
| Transparence | Clics utilisables autour de la forme visible, y compris dans les coins |
| Clic | Ouverture/fermeture stable, aucun vol de focus ; survol reporté conformément au périmètre demandé |
| Clavier | Accès explicite par le menu tray, commandes locales, Échap et focus visible |
| Animations | Interruptions et inversions rapides correctes, progression indépendante du nombre d'images |
| Animations réduites | Réglage Windows respecté |
| Plein écran | Encoche masquée, application plein écran non minimisée ; retour correct |
| DPI/écran | Position et zones de clic cohérentes à 100/125/150/200 % lorsque testable |
| Veille/verrouillage | Retour utilisable, pas de consommation active pendant le masquage |
| Quitter | Fenêtres et icône retirées, aucun processus du prototype restant après deux secondes |
| Lancement répété | Comportement d'instance unique documenté, pas de processus orphelin |

Ne pas déclarer un cas validé s'il manque le matériel ou si un scénario n'a pas été exercé. Les changements de réglages système utilisés pour un test devront être annoncés et restaurés.

## Mesures

Utiliser le binaire release sans débogueur. Relever Windows, CPU, RAM, GPU, écrans, DPI, fréquence, alimentation et charge de fond. Si une information est inaccessible, le signaler.

1. Observer le système sans prototype comme référence.
2. Lancer le prototype et laisser une minute de stabilisation.
3. Mesurer cinq minutes au repos, à une cadence de collecte raisonnable (une seconde).
4. Effectuer 100 cycles d'ouverture/fermeture puis revenir au repos ; comparer mémoire et handles au niveau stabilisé.
5. Mesurer la réactivité d'entrée et le temps des images pendant les transitions.
6. Essai d'endurance d'une heure avec interactions intermittentes, veille/verrouillage si disponibles.
7. Quitter et vérifier l'absence de processus restant.

CPU normalisé machine = 100 × delta du temps CPU processus / (delta temps réel × nombre de processeurs logiques). Publier moyenne et pics ; ne pas comparer des pourcentages définis différemment.

Mémoire : distinguer working set, working set privé et private bytes (engagement privé). Relever handles et threads. Ne pas présenter WorkingSet64 comme une mesure privée. Les compteurs nécessaires pourront être collectés via les API Windows ou les outils de performance disponibles.

Réactivité : distinguer délai événement → soumission du rendu et latence visuelle réelle. Sans capture appropriée, ne pas affirmer que la seconde a été mesurée. Surveiller aussi DWM/GPU lorsque les outils le permettent ; une faible charge du processus seul ne prouve pas un coût graphique nul.

## Objectifs initiaux, à confronter au prototype

- Repos : aucun rendu continu, moyenne CPU machine ≤ 0,1 % sur cinq minutes.
- Mémoire de travail privée : cible 30–60 Mio, engagement privé consigné séparément.
- Réactivité locale : cible < 100 ms ; méthode de mesure explicitée.
- Animation : cadence régulière à 60 Hz ; temps d'image p95 et interruptions relevés.
- Ressources : pas de croissance répétée non bornée des handles, threads ou mémoire après les cycles et le retour au repos.
- Plein écran : pas de perturbation de l'application au premier plan ni de rendu inutile caché.

Ces chiffres sont des objectifs, pas des garanties. Une régression doit être expliquée et corrigée avant l'ajout de fonctionnalités.

## Compte rendu attendu

Pour chaque scénario : configuration, méthode, résultat, preuve et limite. Présenter séparément les essais réussis, échoués et non effectués. Les mesures locales ne garantissent pas le comportement sur tous les PC Windows 11.
