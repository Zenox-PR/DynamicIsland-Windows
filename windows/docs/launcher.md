# Raccourcis / espace développeur — P5

Implémentation originale Rust/Win32, en attente de validation manuelle. P4 sauvegardée dans `2fdce4e` avant les modifications ; aucun commit final P5 ni passage P6 autorisé à ce stade.

## Utilisation

Depuis Accueil, ouvrir **Raccourcis**, ou choisir cette page dans le menu de l'icône système. Trois entrées par page, pagination, filtre Tous/Favoris. Le bouton étoile change le favori ; `…` propose Modifier, Monter, Descendre, Supprimer le raccourci. L'ordre concerne tout le catalogue : utiliser Tous pour réorganiser des entrées filtrées. Supprimer ne touche jamais l'application ou le dossier d'origine.

Ajouter ouvre une fenêtre Windows indépendante, normale et non toujours au-dessus. Choisir nom, type et cible, éventuellement Parcourir pour un fichier/dossier, puis Enregistrer. L'enregistrement ne lance rien. Tab, Entrée et Échap utilisent le routage de dialogue Windows local. L'encoche se replie pendant l'édition ; les minuteurs restent actifs. Le formulaire apparaît normalement dans Alt+Tab, contrairement à l'encoche.

Types proposés : application `.exe` locale sans arguments ; dossier local ; lien HTTPS ; application Windows par AUMID `famille!Application` ; outil connu. Les dossiers de projet s'ouvrent dans l'Explorateur : il n'y a pas encore de commande « ouvrir ce projet dans tel éditeur » avec arguments.

Les six entrées initiales sont Codex, VS Code, GitHub, Windows Terminal, PowerShell et Explorateur. Elles sont des propositions de raccourcis, **pas une certification d'installation**. Codex et VS Code recherchent seulement des emplacements habituels ; modifier la cible vers un `.exe` ou un AUMID si nécessaire. Codex n'a pas été trouvé dans les emplacements sondés sur ce PC. Windows Terminal utilise son AUMID standard. PowerShell est lancé explicitement avec `-NoLogo -NoProfile` ; aucune commande n'est ajoutée. GitHub ouvre le navigateur par défaut seulement au clic. Les icônes sont vectorielles génériques par type, sans logos ni extraction de DLL tierces.

## Architecture et sécurité

- `island-core/launcher.rs` : catalogue pur, validation, CRUD, ordre/favoris et format versionné ; 32 entrées, nom de 48 caractères, cible de 2 048 octets UTF-8, fichier de 96 Kio au maximum.
- `island-windows/launcher.rs` : un worker STA créé à la première utilisation, file bornée à une opération, lecture/écriture et lancement hors du thread UI. Il dort en attente de messages, sans timer ni recherche périodique d'applications.
- `launcher_view.rs` : coordination, retour d'erreur et invalidation des anciennes pressions lorsque le catalogue change. Aucun résultat asynchrone ne prend le focus ni ne réouvre l'encoche.
- `launcher_editor.rs` : contrôles Win32 standards et sélecteur de fichier/dossier explicite ; police/brosse libérées, fenêtre fermée avant jointure du worker.
- `island-ui/launcher.rs`, navigation et surface : rendu sur demande dans la géométrie existante, pas d'animation ou rafraîchissement permanent.

Exécution uniquement sur action explicite : `CreateProcessW` avec application absolue et sans arguments libres ; `ShellExecuteExW` pour dossier/HTTPS ; `IApplicationActivationManager` pour AUMID. Les noms ne passent jamais à un interpréteur. Les scripts, `.lnk`, UNC, chemins relatifs/périphériques, flux alternatifs et redirections résolues vers le réseau sont refusés. Une application absente produit une erreur, sans recherche PATH ni repli vers une commande. La validation n'est ni une analyse antivirus ni une garantie de confiance du fichier choisi ; les associations Shell/HTTP restent celles de Windows. Une cible peut changer entre vérification et lancement : aucune garantie contre un acteur local capable de remplacer l'exécutable.

Le module désactivé ne peut plus lancer d'opération en attente ; une activation déjà transmise à Windows ne peut pas être annulée. Le worker est joint à l'arrêt et termine une écriture acceptée. Un accès fichier ou une activation Shell bloqués peuvent retarder cette jointure : aucun délai maximal dur n'est revendiqué. Aucune dépendance nouvelle ; seule la feature `Win32_System_Registry` de windows-sys expose le type HKEY requis par la structure Shell, sans lecture/écriture du registre.

Références consultées via Context7 : [ShellExecuteExW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw), [SHELLEXECUTEINFOW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-shellexecuteinfow), [ActivateApplication](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateapplication), [options du sélecteur](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/ne-shobjidl_core-_fileopendialogoptions), [IsDialogMessageW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-isdialogmessagew).

## Persistance

Catalogue : `%LOCALAPPDATA%\DynamicIsland\shortcuts.ini`, ou fichier voisin de settings.ini ; `ISLAND_SHORTCUTS_PATH` permet un stockage isolé de test. Format version 1 strict, champs texte encodés en hexadécimal UTF-8 : ce n'est pas du chiffrement. Remplacement atomique via fichier temporaire et synchronisation. Un catalogue illisible/invalide est conservé et signalé, jamais remplacé silencieusement par les valeurs initiales. Fermer l'application avant une correction manuelle ; aucun suivi des éditions externes.

Settings v5 conserve la page et l'activation du lanceur ; les versions antérieures restent lues. Pour revenir à l'ancien binaire, réactiver le lanceur, sélectionner une autre page et quitter proprement : le writer émet le schéma antérieur approprié. L'ancien binaire ignore shortcuts.ini. Ne pas lui confier directement des préférences v5.

## Validation physique demandée

Contrôles automatiques : 62 tests Rust Release réussis, Clippy sans avertissement, fmt et compilation réussis. Banc final 13/13 dans `artifacts/launcher-20261111-222947`, messages Win32 sur nos seules fenêtres. Ajout/persistance/favori et lancement de notre fixture vérifiés ; aucune application utilisateur ouverte par le banc. Mesures de 60 s : compact 31,26 Mio de mémoire de travail, déplié 32,02 Mio, aucun incrément CPU détecté ; GDI 22 constant. Dix cycles éditeur : GDI22/USER26 inchangés, handles326→327. Détails, empreinte et essais initiaux non réussis conservés dans [PROGRESS](PROGRESS.md).

1. Ouvrir Raccourcis depuis Accueil et l'icône système ; revenir aux Médias et Minuteurs. Vérifier la lisibilité et l'absence de changement du compact/concentration.
2. Ajouter un dossier de test via Parcourir ; Enregistrer ne doit rien ouvrir. Cliquer ensuite sur son entrée doit ouvrir ce dossier dans l'Explorateur. Ajouter aussi une application `.exe` de confiance choisie par vous.
3. Modifier son nom/cible, épingler, filtrer, déplacer l'ordre, changer de page, supprimer uniquement le raccourci. Vérifier que les originaux existent toujours.
4. Essayer un chemin absent et un script : obtenir une erreur sans lancement. Vérifier Codex/VS Code/Terminal selon vos installations, GitHub seulement si vous souhaitez ouvrir le site.
5. Dans l'éditeur : Tab, Entrée, Échap, Annuler, Alt+Tab ; dans l'encoche : clic extérieur, Échap pendant l'animation, concentration et plein écran. Aucun focus volé au retour d'un enregistrement.
6. Quitter puis relancer : catalogue, ordre et favoris conservés, rien lancé automatiquement. Désactiver/réactiver le module depuis Accueil ; les autres modules restent utilisables.

Les messages envoyés par le banc ne certifient pas ces interactions physiques. DPI dynamique, plusieurs écrans, GPU, veille matérielle, UI Automation complète et endurance longue restent non validés. Les mesures courtes ne remplacent pas un repos strict de cinq minutes. Le résultat historique P4 combinant fermeture du popover média et premier plan reste brut à 32/33 malgré la validation globale utilisateur ; aucune nouvelle certification de ce point.
