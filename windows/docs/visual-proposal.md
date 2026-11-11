# Proposition visuelle — Noir satiné choisi et implémenté

3 novembre 2026. Document de spécification conservant la comparaison initiale. L'utilisateur a choisi et autorisé Noir satiné ; cette variante est désormais implémentée nativement. Ligne discrète reste une proposition non implémentée. Voir [le rapport de réalisation](noir-satine-report.md) pour les tests, mesures et limites. Aucun module avancé, ressource tierce copiée, dépendance ou logiciel ajouté.

## Références et intention

[WindyLand](https://github.com/shabab239/WindyLand/blob/main/README.md) présente une petite pilule et des panneaux de contenu ; [Nusa](https://rama2555.github.io/nusa/) montre un élément sombre discret qui s'agrandit selon le contexte. Retenir la sobriété, la continuité de la silhouette et une hiérarchie visuelle nette. Conserver notre ouverture au clic et nos règles de focus déjà validées, sans reprendre le survol d'ouverture ni les modules de ces références. Aucun chiffre de performance annoncé par ces projets n'est transposé au nôtre.

## Deux directions

| Paramètre | Noir satiné — recommandée | Ligne discrète |
|---|---|---|
| Compacte | 172 × 32 DIP | 160 × 28 DIP |
| Rayon compact | 16 DIP | 14 DIP |
| Dépliée | 384 × 168 DIP | 352 × 128 DIP |
| Rayon déplié | 26 DIP | 22 DIP |
| Marge au haut de l'écran | 8 DIP | 8 DIP |
| Marges internes | 24 DIP | 20 DIP |
| Titre | Segoe UI 20 DIP, semi-gras | Segoe UI 18 DIP, semi-gras |
| Texte secondaire | 14 DIP, normal | 13 DIP, normal |
| Aide de fermeture | 12 DIP, raccourci 11 DIP | 12 DIP, raccourci 11 DIP |
| Organisation | Titre/texte alignés à gauche ; pied séparé par un trait très discret | Texte plus serré ; pied sans séparateur |
| Intention | Équilibre et lisibilité, place pour une structure future | Occupation minimale, contenu très court |

La maquette affiche l'état compact puis un panneau déplié interactif. Le clic dans le panneau le replie/rouvre, Échap agit uniquement lorsqu'il a le focus. La comparaison est une démonstration de proportions, pas une simulation du focus Win32. Les dimensions sont des cibles en DIP ; la prévisualisation s'adapte aux écrans étroits.

Vérification de la maquette : captures inspectées à 780 et 320 pixels de largeur, sans débordement horizontal, et contrôles locaux clic/Échap réussis pour les deux variantes. Ces vérifications de présentation ne comptent pas comme tests de l'application Rust. Le fichier de comparaison est séparé du dépôt applicatif, dans l'espace de visualisation de cette conversation ; aucun serveur local n'a été lancé.

Noir satiné : fond opaque #0B0C0F, texte principal #F3F4F6, secondaire #A5A7AF, contour #2A2C32 ; rayon harmonisé, petite poignée discrète. Ligne discrète : noir #08090B et mêmes niveaux de contraste. Pas de halo, d'ombre large ou d'effet de verre nécessaire. L'opacité concerne le panneau ; l'extérieur reste transparent.

Le panneau actuel reste un état minimal avec titre, phrase courte et aide de fermeture. Aucun onglet vide, lecteur musical, jauge CPU, bouton de configuration non fonctionnel ou faux contenu métier n'est proposé. Le raccourci dessiné « Échap » est un repère, pas un nouveau contrôle. Toute la surface conserve son action de clic déjà validée.

## Mouvement retenu pour Noir satiné

- Ancrage du centre horizontal et du bord supérieur ; largeur et hauteur évoluent ensemble.
- Essai initial : ouverture 220 ms, fermeture 180 ms, ralentissement progressif sans rebond ni dépassement.
- Repartir de la position courante à l'inversion, adapter la durée à la distance restante ; maintenir l'interruption par Échap et clic pendant tout le trajet.
- Le contenu apparaît progressivement près de la fin d'ouverture, sans se déformer avec le fond. Pas de calcul de mise en page coûteux à chaque frame.
- Respect du réglage Windows d'animations réduites : changement immédiat.
- Objectif : cadence régulière proche de 60 Hz, à mesurer. Le timer Win32 actuel ne garantit pas la synchronisation avec l'écran 144 Hz. N'adopter une autre cadence ou une chaîne Composition/Direct2D que si une mesure montre le besoin ; ne pas modifier globalement la résolution des timers.

La transition du navigateur dans la maquette illustre seulement le mouvement. Elle ne prédit pas la cadence, le coût CPU/GPU ni la latence de la future version native.

## Réalisation native retenue

Conserver le renderer Win32/GDI et UpdateLayeredWindow pour une première itération. Pas de WebView, serveur ou framework additionnel dans l'application. Segoe UI déjà utilisée ; pas de police à installer. La prévisualisation préfère Segoe UI Variable si disponible, avec repli Segoe UI ; la réalisation devra être jugée sur son rendu natif réel.

Réalisation après validation : constantes de dimensions, espacements et couleurs côté UI, alignement aux pixels physiques après conversion DPI, cache des polices et chaînes par DPI. Les quatre rectangles de disposition restent un calcul arithmétique par image, sans allocation. Le texte n'est pas transformé à chaque image. La réutilisation des surfaces reste à évaluer si les allocations intermédiaires deviennent un coût problématique. Une seule chaîne de rendu est conservée.

L'extérieur de la forme doit rester à alpha nul ; une future ombre ne devra pas étendre la surface qui intercepte les clics. Aucun flou d'arrière-plan ou matériau Acrylic/Mica dans la première refonte. DWM compose toujours la fenêtre : « pas de D3D explicite » ne signifie pas « GPU nul ».

## Estimations initiales de surface

Avec des pixels BGRA de 4 octets, hors allocations système, polices, cache, DWM et éventuels doubles buffers :

| Surface dépliée | Actuelle 400 × 160 | Noir satiné 384 × 168 | Ligne discrète 352 × 128 |
|---|---:|---:|---:|
| Pixels à 100 % | 64 000 | 64 512 | 45 056 |
| Variation de surface | référence | +0,8 % | −29,6 % |
| Une DIB à 125 % | 390,6 Kio | 393,8 Kio | 275,0 Kio |
| Une DIB à 200 % | 1 000 Kio | 1 008 Kio | 704 Kio |

Noir satiné ne devrait donc presque pas changer la mémoire d'une surface. La version plus petite réduit les pixels à traiter, mais cela ne signifie pas une réduction proportionnelle de la RAM totale ou du CPU. La typographie, les transitions et les copies DWM restent à mesurer.

Au repos : aucun rendu périodique, calcul d'effet ou animation décorative. Pas de raison architecturale d'augmenter les réveils CPU, mais la cible ≤ 0,1 % de moyenne machine sur cinq minutes doit être revalidée avec le panneau visible. Le seuil mémoire privé de 60 Mio reste un plafond, pas une allocation à remplir.

Critères de validation : essais à 125 % disponibles, lisibilité sur fonds clairs/sombres, clics dans les coins, clavier, inversion très précoce, 100 cycles de fenêtres, cinq minutes visibles au repos, handles et objets graphiques stabilisés, CPU/latence avant-après sur une charge comparable. Voir le rapport de réalisation pour les mesures effectivement obtenues et les critères restant à vérifier. Le coût GPU et DWM sera déclaré indisponible si les compteurs ne permettent toujours pas son attribution.

## Décision reçue

Noir satiné choisi explicitement par l'utilisateur. Cette autorisation couvre uniquement la refonte du socle visuel et de son animation, les tests et mesures. Les modules avancés restent exclus. L'utilisateur testera le design et Échap avant de valider la suite.
