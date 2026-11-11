# Dynamic Island pour Windows

Une encoche native et discrète pour Windows 11, développée en Rust, accompagnée d'un site pour l'essayer et comprendre son code.

> **Deux projets distincts dans ce dépôt.** L'application contrôle des fonctionnalités Windows ; le site en propose une simulation locale, sans accès aux sessions multimédias ou fichiers de votre ordinateur.

| Projet | Emplacement | Technologies | Démarrer |
|---|---|---|---|
| **Application Windows** | [`windows/`](windows/) | Rust, Win32, WinRT/GSMTC, GDI | [README application](windows/README.md) |
| **Site de démonstration** | [`site/`](site/) | Next.js, React, TypeScript | [README site](site/README.md) |

```text
windows/                 Application native Windows 11 x64
  crates/island-core/    États et logique métier
  crates/island-ui/      Géométrie et rendu
  crates/island-windows/ Adaptateurs Windows
  crates/island-app/     Exécutable
  docs/                 Architecture et rapports techniques
site/                    Site web indépendant
  app/                   Accueil, démo, documentation, code, histoire
  simulation/            Modèle de simulation navigateur
  generated/             Extraits du véritable code Rust
LICENSE                  PolyForm Noncommercial 1.0.0
```

## Ce que fait l'application

Encoche flottante Noir satiné, ouverture/repli, déplacement Ctrl + glisser, masquage/restauration, position mémorisée, concentration sous forme de ligne traversante. Sessions multimédias GSMTC, sélection de source et commandes selon les capacités du lecteur. Navigation modulaire, minuteurs/Pomodoro et catalogue de raccourcis.

Le lanceur est implémenté, avec validation physique détaillée encore attendue. Les informations système sont un socle expérimental sans page ni collecte active. Les fonctions futures ne sont pas annoncées comme disponibles.

## Ce que montre le site

Bureau interactif, modes de l'encoche, médias fictifs silencieux, minuteurs et raccourcis simulés. Explorateur Rust avec lignes d'origine et empreintes, explications synchronisées avec les gestes, architecture interactive et chronologie du développement.

Le site n'est pas déployé automatiquement par ce dépôt. Aucun binaire Windows précompilé ni installateur n'est fourni à ce stade.

## État et vérifications

Projet en développement. Campagne native documentée : 67 tests Rust réussis sur le socle courant. Campagne web locale : build statique, TypeScript strict, 6 tests Node et 28 contrôles navigateur réussis. Ces résultats ne certifient pas tous les matériels ou lecteurs multimédias.

Limites : DPI dynamique, multi-écrans, jeux exclusifs, endurance et accessibilité native UI Automation restent à approfondir. Le banc P4 conserve un résultat brut de 32/33 malgré la validation globale de cette phase. Voir les [documents techniques](windows/docs/).

## Licence — usage non commercial

Le code original de l'application et du site est distribué sous [PolyForm Noncommercial 1.0.0](LICENSE). Les usages non commerciaux, modifications et redistributions sont permis dans les conditions de cette licence. Aucun droit d'exploitation commerciale n'est accordé par cette licence ; une autorisation distincte du titulaire est nécessaire pour un usage non couvert.

Le code est **source disponible**, et n'est pas présenté comme une licence open source OSI. Les dépendances tierces conservent leurs propres licences : voir [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Le texte anglais de LICENSE fait foi ; ce résumé ne le remplace pas.

Projet indépendant, sans affiliation à Apple ou Microsoft. Les noms de produits appartiennent à leurs titulaires respectifs.
