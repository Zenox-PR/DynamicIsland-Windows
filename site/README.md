# Site de démonstration

**Adresse publique : https://zenox-pr.github.io/DynamicIsland-Windows/**

GitHub Pages sert l'export statique de la branche `gh-pages`, dossier racine. Pour le reconstruire dans PowerShell : `$env:NEXT_PUBLIC_BASE_PATH='/DynamicIsland-Windows'; npm run build`. Copier uniquement le contenu de `out/` vers la branche de déploiement, avec un fichier `.nojekyll` à sa racine. La branche `main` conserve les sources ; un changement de sources ne déploie pas automatiquement une nouvelle version. Pour un build local à la racine, enlever cette variable d'environnement avant de compiler.

[Accueil du dépôt](../README.md) · [Application Windows](../windows/README.md)

Site indépendant en Next.js, React et TypeScript strict. Il simule l'application dans le navigateur : aucun contrôle réel de Windows, aucun lancement de programme, aucune lecture de vos fichiers ou sessions GSMTC.

## Démarrer

Node.js compatible avec Next.js 16 requis ; développement vérifié avec Node 26.13.0.

```powershell
cd site
npm ci
npm run dev
```

Ouvrir http://127.0.0.1:3011. Le serveur écoute uniquement sur la boucle locale. Ctrl+C l'arrête.

```powershell
npm run build
npm run preview
```

Le build produit `out/`, prêt pour un hébergement statique. Le serveur preview sert uniquement ce dossier. Ne pas lancer dev et preview simultanément. Aucune publication automatique.

## Pages

| Route | Contenu |
|---|---|
| `/` | Présentation et démonstration interactive |
| `/demo/` | Bureau et code synchronisés |
| `/sous-le-capot/` | Architecture et choix techniques |
| `/code/` | Extraits Rust, lignes, coloration et copie |
| `/histoire/` | Chronologie et limites connues |

Les sources Rust sont lues dans `../windows/crates/` par `scripts/extract.mjs`, sur liste blanche. Les dix extraits générés incluent leur numéro de ligne et empreinte SHA-256. Cloner le dépôt entier, pas seulement le dossier site, pour régénérer les extraits. Ne jamais servir la racine du dépôt comme un dossier public.

## Tests

```powershell
npm test
npm run typecheck
```

`tests/browser-checks.js` contient le parcours Playwright à transmettre à un outil compatible avec une fonction `(page) => …`, après démarrage du serveur. Les contrôles locaux ont couvert 360/768/1440 pixels, réduction des animations, médias fictifs, minuteurs, catalogue, navigation, code et graphe. Comparaison visuelle native/web et audit complet d'accessibilité restent à approfondir.

Médias silencieux et titres inventés. Fond et pochettes originaux en CSS, police système. Le catalogue fictif est stocké dans sessionStorage ; les autres réglages sont limités à la session du composant. La télémétrie Next est désactivée par le lanceur local.

## Licence

[PolyForm Noncommercial 1.0.0](../LICENSE) pour le code original et les visuels originaux. Dépendances tierces sous leurs licences respectives.
