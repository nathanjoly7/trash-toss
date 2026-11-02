# Trash Toss et Bureau propre

Deux prototypes Bevy 0.20 + avian3d 0.8 dans le même bureau en 3D, un seul export web.

**Trash Toss** : dix déchets à jeter dans la poubelle verte depuis la ligne jaune.

**Bureau propre** (cybersécurité et RGPD) : 18 h, on part en dernier. Il faut faire le tour de l'open space, repérer cinq risques et les régler.

| Risque | Où | Action attendue |
| --- | --- | --- |
| Post-it avec un mot de passe | écran du bureau de gauche | le prendre, le jeter dans la broyeuse |
| Contrat client signé | sur l'imprimante | le prendre, le ranger dans l'armoire fermée à clé |
| Clé USB inconnue | par terre | la prendre, la remettre au service informatique |
| Session ouverte | écran du bureau de droite | la toucher pour la verrouiller |
| Tableau blanc du projet | mur du fond | le toucher pour l'effacer |

Un objet lancé dans le mauvais bac compte une erreur, ressort et affiche la règle. On ne prend qu'un objet à moins de 2,5 m. Le débrief rappelle les cinq règles ; chaque erreur coûte 300 points (2000 au départ, 500 au minimum).

Jouer en ligne :
- Trash Toss : https://nathanjoly7.github.io/trash-toss/
- Bureau propre : https://nathanjoly7.github.io/trash-toss/?jeu=bureau-propre

## Commandes

- PC (natif) : ZQSD pour marcher, souris pour regarder, clic pour prendre ou agir, maintenir puis relâcher pour lancer.
- Web à la souris : un clic dans le jeu capture la souris (vue FPS comme sur PC, Échap pour la libérer). Si le navigateur refuse trois fois, on revient au cliquer-glisser pour regarder.
- Web au doigt : pouce qui glisse depuis le tiers gauche de l'écran pour marcher (joystick ; un simple toucher à gauche reste un toucher), glisser ailleurs pour regarder, toucher un objet pour le prendre, appui long puis relâcher pour lancer.

## Lancer

```
cargo dev                                        # Trash Toss, Bevy en DLL
cargo dev -- bureau-propre                       # Bureau propre
cargo dev --features autoplay -- bureau-propre   # le jeu se joue tout seul, erreur volontaire comprise
.\build-web.ps1                                  # export web dans dist/ (wasm-bindgen-cli 0.2.129 requis)
```

## Intégration GeoGaming

Le jeu tourne dans une iframe d'une activité Widget et envoie à la fenêtre parente `{ completed: true, points }`, que l'app relaie pour valider l'étape. Trash Toss l'envoie 2 s après le dernier déchet ; Bureau propre quand le joueur a lu le débrief et touché l'écran. `web/geogaming-test.html` reproduit ce pont en local (`?src=` pour tester une autre adresse).

`.cargo/config.toml` partage le dossier de compilation avec `../bevy-sandbox` : à retirer sur une autre machine.

Police : Montserrat, licence SIL OFL (`assets/fonts/OFL.txt`).
