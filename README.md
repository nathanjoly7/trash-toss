# Trash Toss

Prototype Bevy 0.20 + avian3d 0.8 : un bureau en 3D, dix déchets à jeter dans la poubelle verte depuis la ligne jaune.

- Bureau (Windows) : vue FPS, ZQSD pour marcher, clic pour ramasser, maintenir puis relâcher pour lancer.
- Web et téléphone : glisser pour regarder, toucher un déchet pour l'attraper, appui long puis relâcher pour lancer.

Jouer en ligne : https://nathanjoly7.github.io/trash-toss/

## Lancer

```
cargo dev                        # Bevy en DLL, recompilation en quelques secondes
cargo dev --features autoplay    # le jeu se joue tout seul, pour vérifier lancer et score
.\build-web.ps1                  # export web dans dist/ (wasm-bindgen-cli 0.2.129 requis)
```

## Intégration GeoGaming

Le jeu tourne dans une iframe d'une activité Widget. Quand les dix déchets sont jetés, il envoie à la fenêtre parente `{ completed: true, points: 2000 }`, que l'app relaie pour valider l'étape. `web/geogaming-test.html` reproduit ce pont en local.

`.cargo/config.toml` partage le dossier de compilation avec `../bevy-sandbox` : à retirer sur une autre machine.

Police : Montserrat, licence SIL OFL (`assets/fonts/OFL.txt`).
