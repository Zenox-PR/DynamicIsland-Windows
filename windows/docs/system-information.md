# P6 — préparation du module Informations système

État : premier socle implémenté, pas encore de page ni de collecte activée dans l'encoche. L'utilisateur a clarifié : continuer localement, aucun commit sauf demande explicite. Aucun essai manuel P5 supplémentaire n'est présumé.

## Premier jalon implémenté

`island-core/system.rs` calcule le CPU par différences, valide la mémoire et distingue batterie absente/inconnue/pourcentage inconnu. Les premières mesures, resets, erreurs et compteurs incohérents restent indisponibles plutôt que 0 %. `island-windows/system_info.rs` lit uniquement les indicateurs demandés, sans thread, timer, handle persistant ni mutation système. Sur plusieurs groupes de processeurs, le CPU est explicitement indisponible : aucune moyenne de groupe n'est présentée comme celle de toute la machine. Le module est une bibliothèque utilisable par le futur worker ; aucun appel depuis App à ce jalon.

Feature `Win32_System_Power` ajoutée au windows-sys déjà présent, sans dépendance supplémentaire ni téléchargement. `examples/probe_system.rs` est un banc borné de 100 appels par indicateur, en lecture seule. Preuves dans `artifacts/system-p6/`.

Relevé Release ponctuel : 100 lectures CPU en 12 855 µs, mémoire en 54 µs, batterie en 85 µs (temps mural cumulé, **pas consommation CPU du processus**, aucune conclusion d'endurance). 100 réponses disponibles pour chaque indicateur. Batterie observée à 100 %, secteur présent, pas en charge. Ce résultat matériel ponctuel ne valide pas les transitions branchement/débranchement. La fonction désactivée renvoie uniquement des absences de valeur et n'appelle aucune API.

Reste : worker borné, politique de visibilité et invalidation des résultats tardifs, indicateur réseau avec sélection non ambiguë d'interface, page/native navigation, préférences, tests de cycle de vie puis mesures en conditions réelles. Les limites de cadence et d'interface ci-dessous sont des décisions de conception, pas des fonctionnalités déjà livrées.

## Contrat de collecte proposé

Page native dans le panneau existant ; aucun service, pilote, dépendance lourde ou historique. Activer séparément CPU, mémoire, batterie et réseau. Ne créer aucune collecte lorsque la page est absente, le panneau replié, masqué, en concentration ou le module désactivé. Arrêter aussi pendant les transitions ; reprendre après stabilisation.

Cadence initiale envisagée : deux secondes pour CPU/RAM/réseau visibles ; batterie sur notifications d'alimentation, avec lecture initiale. Un worker dormant et une requête en vol au maximum ; snapshot borné transmis au thread UI. Ne jamais faire d'énumération réseau sur le thread UI. Un résultat tardif ne doit ni rouvrir le panneau ni réarmer une collecte désactivée.

CPU : différences entre compteurs cumulatifs idle/kernel/user, avec refus d'une différence négative ou d'un total nul. Premier relevé présenté comme indisponible, pas comme 0 %. Remettre la référence à zéro après masquage/reprise pour ne pas moyenner une longue période invisible. Vérifier expressément la portée des groupes de processeurs ; ne pas annoncer une mesure machine entière sur un matériel non couvert.

Mémoire : mémoire physique totale/disponible, utilisation calculée avec bornes vérifiées ; distinguer cette valeur de la mémoire du processus Dynamic Island. Batterie : absence de batterie et valeurs inconnues représentées explicitement. Réseau : débits par différences d'octets et temps monotone réel, sans noms d'hôtes ni contenu des communications. Définir le traitement des interfaces virtuelles/VPN avant toute somme pour éviter le double comptage ; ne pas prétendre mesurer le trafic Internet exclusivement.

## Découpage et contrôles à réaliser

1. Logique pure de différences, validité, remise à zéro et cadence ; tests déterministes (reset, overflow, temps nul, données absentes).
2. Adaptateurs Windows documentés, ressources RAII, arrêt joint et erreurs non fatales. Contrôler les API exactes via Context7 avant leur utilisation.
3. Coordination de visibilité et activation, puis page Noir satiné et préférences versionnées. Préserver concentration, GSMTC et minuteurs.
4. Vérifier qu'aucune requête ne subsiste après repli/désactivation et qu'un résultat ancien ne déclenche pas de rendu inutile.
5. Mesurer chaque indicateur isolément puis l'ensemble en Release ; distinguer durée CPU, mémoire, handles, GDI et cadence réelle. Mesurer aussi le compact pour confirmer le coût nul des collectes désactivées, sans supposer un CPU strictement nul.

## Recherche effectuée

Context7, référence Win32 Microsoft : [GlobalMemoryStatusEx](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex), [SYSTEM_POWER_STATUS](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-system_power_status). La réponse Context7 CPU étant incomplète, lecture directe de la [documentation GetSystemTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes) : kernel inclut idle et la portée est limitée au groupe primaire au-delà de 64 processeurs. Réseau à documenter avant son implémentation. Aucun résultat de performance P5 n'est attribué à P6.
