# Correction d'Échap et du focus — 3 novembre 2026

Suite : l'utilisateur a validé 5/6 essais manuels, avec un échec d'Échap pendant l'animation. Voir [le correctif complémentaire](escape-animation-fix.md). Ce document conserve les observations de la première correction.

## Signalement et cause

L'utilisateur a confirmé l'affichage compact, l'ouverture au clic, le repli au second clic et les animations. Il a reproduit l'échec d'Échap après ouverture au clic. Ces observations manuelles concernent la version précédente.

Deux chemins du code expliquaient le défaut :

1. La fenêtre porte WS_EX_NOACTIVATE et répond MA_NOACTIVATE à WM_MOUSEACTIVATE. WM_LBUTTONUP ne faisait que basculer la transition. L'ouverture ne demandait donc jamais l'activation ou le focus clavier ; les touches restaient destinées à l'application précédente, alors que le panneau attendait WM_KEYDOWN/VK_ESCAPE.
2. WM_ACTIVATE mettait un booléen visuel à jour puis retournait 0 sans DefWindowProc. Cela supprimait le traitement natif qui donne normalement le focus clavier à une fenêtre activée non minimisée. Marquer focused=true ou demander seulement le premier plan ne suffisait pas à établir un chemin clavier fiable.

Documentation consultée via Context7, puis complétée par les pages Microsoft lorsque les extraits Context7 ne répondaient pas à la sémantique précise : [WM_ACTIVATE](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-activate), [SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow), [SetFocus](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setfocus).

## Correction limitée

Dans crates/island-windows/src/lib.rs :

- Le clic d'ouverture, le tray et son menu utilisent toggle_explicit. Avant l'ouverture, la fenêtre précédente est mémorisée et SetForegroundWindow est demandé. SetFocus n'est appelé que si l'encoche a effectivement obtenu le premier plan. Un refus Windows n'est pas contourné, ni réessayé en boucle ; il peut être consigné dans le journal optionnel.
- WS_EX_NOACTIVATE, MA_NOACTIVATE et l'affichage sans activation restent en place. Le lancement, le retour depuis un plein écran et le survol ne demandent aucun focus.
- WM_ACTIVATE laisse le traitement Windows par défaut s'exécuter. WM_SETFOCUS/WM_KILLFOCUS suivent le focus réel, et WM_KEYDOWN n'agit que si GetFocus désigne le panneau.
- Échap replie et tente de restituer la fenêtre précédente. Même restitution au second clic ou au repli par clavier/menu, uniquement si l'encoche est encore au premier plan et si la destination existe, est visible et non minimisée. Aucun retour forcé après Alt+Tab ou clic extérieur. Si la destination a disparu ou si Windows refuse, le clic dans l'application voulue reste nécessaire.
- Les appels d'activation peuvent réentrer dans la procédure de fenêtre. Le RefCell conserve sa protection contre les emprunts simultanés ; les messages réentrants conservent leur traitement Win32 par défaut. L'état du focus est relu après la demande explicite.
- Aucun hook clavier global, raccourci global, AttachThreadInput, polling clavier, timer ou thread ajouté. Le hook souris existant n'est pas modifié.

Le constructeur sans effets de bord for_window permet de tester la procédure native sans créer d'icône tray ni de hook. Le nettoyage ne tente de retirer l'icône que si son HWND a été initialisé. Le moteur de dessin, les dimensions, les couleurs, l'alpha et la courbe/durée d'animation ne sont pas modifiés.

## Tests automatisés

- cargo fmt --all puis cargo fmt --all -- --check : réussite.
- cargo clippy --workspace --all-targets --locked --offline -- -D warnings : réussite, aucun avertissement masqué.
- cargo test --workspace --all-targets --locked --offline : **7 tests réussis**.
- cargo test --workspace --doc --locked --offline : réussite, pas de doctest exécutable.
- cargo test -p island-windows --lib --release --locked --offline : **2 tests natifs réussis**.
- cargo build --workspace --release --locked --offline : réussite, nouveau dynamic-island.exe.

Le nouveau test native_activation_focus_and_local_escape crée une fenêtre cachée sur son propre thread et appelle la vraie procédure via les messages Win32. Il vérifie MA_NOACTIVATE, l'absence de repli sur Échap sans focus, l'attribution de focus par WM_ACTIVATE, le repli sur Échap avec focus, le refus d'Échap après retrait du focus et le maintien du repli au clic. Il n'injecte aucune touche physique et n'active aucune application utilisateur. Les tests natifs sont sérialisés afin que la création de la fenêtre de test ne perturbe pas les compteurs GDI/USER du test de ressources.

Les tests existants de transparence, géométrie, inversion, fin de transition et animations réduites passent. Le moteur réalise 100 cycles, 3 300 dessins en mémoire, sans augmentation GDI/USER après libération. Résultat Release : 2,240 s ; médiane 0,441 ms, p95 1,608 ms, maximum 4,118 ms. C'est cohérent avec le relevé précédent (p95 1,624 ms), sans constituer un benchmark comparatif contrôlé ni mesurer la cadence affichée.

## Limites et validation manuelle requise

Les tests natifs vérifient le dispatch et le focus de la file locale ; ils ne simulent pas l'autorisation de premier plan accordée par Windows à un véritable clic utilisateur. L'outil graphique ne pouvant pas cibler l'encoche dans la session précédente, aucun succès de clavier physique n'est prétendu ici.

Procédure pour le nouveau binaire :

1. Lancer target/release/dynamic-island.exe depuis le dépôt. Éviter une application plein écran au premier plan pour rendre l'encoche visible.
2. Activer une application normale, puis cliquer une fois sur l'encoche. Sans cliquer ailleurs, appuyer sur Échap : le panneau doit se replier. La fenêtre précédente doit retrouver le premier plan si elle est encore disponible.
3. Répéter, en appuyant sur Échap pendant l'ouverture : l'animation doit s'inverser sans saut.
4. Ouvrir puis replier par un second clic ; vérifier que cette alternative fonctionne toujours.
5. Ouvrir, puis cliquer dans une autre application : elle doit recevoir le clic et le panneau doit se replier sans réactiver une troisième fenêtre.
6. Ouvrir, faire Alt+Tab vers une autre fenêtre normale, puis appuyer sur Échap : la touche doit agir seulement dans cette application, pas sur l'encoche. Le panneau peut ensuite être replié au clic.
7. Passer simplement la souris sur l'encoche pendant une saisie ailleurs : aucun changement de focus. Vérifier aussi les coins transparents et le menu du tray.
8. Quitter via le menu de l'icône système.

Aucun changement de design ni nouvelle fonctionnalité, installation ou configuration globale. Les anciens chiffres de cinq minutes ne sont pas attribués au correctif : un contrôle court du nouveau binaire est consigné ci-dessous.

## Contrôle court du binaire corrigé

Binaire Release SHA-256 : `58B23592C98881989C08FB6D65F424CA7DCDF6ED9DC4C50439D2B3C0ECE081DF`.

Collecte de 30,588 secondes / 30 échantillons, sans entrée automatisée, encoche masquée par le plein écran aux contrôles initial/final :

- Delta CPU mesuré : 0 s (0 % aux résolutions de ces compteurs, pas garantie physique de consommation strictement nulle).
- Working set total moyen : 11,60 Mio ; maximum 11,63 Mio.
- Handles : 157 → 158 ; objets GDI 5 constants, USER 7 constants.
- Engagement privé : 1,77 → 1,88 Mio ; le contrôle court ne permet pas de conclure à l'absence de fuite à long terme.
- Arrêt natif propre, code 0, journal `run result=Ok(())`, aucun processus du projet restant.

Preuves : `artifacts/validation-20261103-211744/` et `artifacts/escape-fix-startup.log`. Ce contrôle confirme le repos dans le scénario masqué et le cycle de vie ; il ne mesure pas les nouvelles interactions de focus, la latence physique ou le coût GPU. Aucun nouveau rendu continu, timer ou polling n'est introduit par le correctif.
