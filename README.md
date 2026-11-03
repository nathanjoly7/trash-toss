# Trash Toss, Bureau propre et Boîte mail piégée

Trois prototypes Bevy 0.20 + avian3d 0.8 dans le même bureau en 3D, un seul export web.

**Trash Toss** : dix déchets à jeter dans la poubelle verte depuis la ligne jaune.

**Bureau propre** (cybersécurité et RGPD) : 18 h, on part en dernier. Il faut faire le tour de l'open space, repérer cinq risques et les régler. Un clic sur un objet proche (moins de 2,5 m) ouvre un menu de choix près du curseur, dont un piège plausible ; on choisit au clic, au doigt ou avec les touches 1 et 2.

| Risque | Où | Bon choix | Piège |
| --- | --- | --- | --- |
| Post-it avec un mot de passe | écran du bureau de gauche | Prendre, puis le lancer dans la broyeuse | Le cacher sous le clavier |
| Contrat client signé | sur l'imprimante | Prendre, puis le ranger dans l'armoire | Le retourner face cachée |
| Clé USB inconnue | sur le bureau de droite, à côté du PC allumé | L'envoyer au service informatique | La brancher pour trouver son propriétaire |
| Session ouverte | écran du bureau de droite | Verrouiller la session | Éteindre l'écran |
| Tableau blanc du projet | mur du fond | Effacer | Le prendre en photo pour plus tard |

On ne lance plus qu'à la broyeuse. La grande armoire, contre le mur du fond, ouvre ses portes quand on s'approche ; un objet en main, un clic sur l'armoire le pose sur une étagère. Un piège, un objet rangé au mauvais endroit (le post-it dans l'armoire, le contrat dans la broyeuse) compte une erreur et affiche la règle. Le débrief rappelle les cinq règles ; chaque erreur coûte 300 points (2000 au départ, 500 au minimum).

**Boîte mail piégée** (hameçonnage) : 9 h, on est assis à son poste, l'écran en face, l'imprimante à droite, la broyeuse à gauche, la corbeille sous le bureau. On tourne la tête sans se lever. Un clic sur l'écran zoome sur la messagerie de Camille Martin, chez Nordhal (nordhal.fr). Huit mails arrivent, les trois premiers d'un coup, puis un toutes les 12 s (ou tout de suite si la boîte est vide). Pour chacun : Répondre, Classer, Supprimer ou Signaler, ouvrir son lien ou sa pièce jointe. Un lien affiche sa vraie adresse avant de s'ouvrir (au survol, et dans le dialogue de confirmation).

| Mail | Expéditeur | Bon geste | Pièges |
| --- | --- | --- | --- |
| Planning de la réunion | j.roche@nordhal.fr | Imprimer la pièce jointe (elle sort de l'imprimante) | Supprimer, Signaler (fausse alerte) |
| Mot de passe qui expire | support@nordhal-it.com | Signaler | Ouvrir le lien (.ru), Répondre |
| Coupure du réseau | dsi@nordhal.fr | Classer ou Supprimer | Signaler (fausse alerte) |
| Facture impayée | factures@bureautik-paiement.com | Signaler | Ouvrir `Facture_2025-118.pdf.exe` : rançongiciel |
| Partage Microsoft 365 | partage@micros0ft-office.com | Signaler | Ouvrir le lien, Répondre |
| Séminaire RH | rh@nordhal.fr | Ouvrir le lien (intranet.nordhal.fr) | Signaler (fausse alerte) |
| Demande du PDG | pdg.nordhal@gmail.com | Signaler | Répondre (fraude au président) |
| Publicité | offres@fournitures-pro.fr | Supprimer ou Signaler | aucun |

Les gestes sans danger mais inutiles (supprimer un mail piégé sans le signaler, répondre à une information) donnent un conseil sans compter d'erreur. Le rançongiciel met l'écran au rouge et imprime la demande de rançon ; « Payer la rançon » compte une erreur. On ne se lève pas : on baisse les yeux vers la tour, au sol à gauche de la chaise, et on choisit entre « L'éteindre » (erreur) et « Débrancher le câble réseau ». Le service informatique restaure alors le poste en 3,5 s et supprime le mail piégé. Même barème que Bureau propre.

Jouer en ligne :
- Trash Toss : https://nathanjoly7.github.io/trash-toss/
- Bureau propre : https://nathanjoly7.github.io/trash-toss/?jeu=bureau-propre
- Boîte mail piégée : https://nathanjoly7.github.io/trash-toss/?jeu=boite-mail

## Commandes

- PC (natif) : ZQSD pour marcher, souris pour regarder, clic pour prendre ou agir, maintenir puis relâcher pour lancer.
- Web à la souris : un clic dans le jeu capture la souris (vue FPS comme sur PC, Échap pour la libérer). Si le navigateur refuse trois fois, on revient au cliquer-glisser pour regarder.
- Web au doigt : pouce qui glisse depuis le tiers gauche de l'écran pour marcher (joystick ; un simple toucher à gauche reste un toucher), glisser ailleurs pour regarder, toucher un objet pour le prendre, appui long puis relâcher pour lancer.

## Lancer

```
cargo dev                                        # Trash Toss, Bevy en DLL
cargo dev -- bureau-propre                       # Bureau propre
cargo dev --features autoplay -- bureau-propre   # le jeu se joue tout seul : 2 erreurs volontaires, 1400 points
cargo dev -- boite-mail                          # Boîte mail piégée
cargo dev --features autoplay -- boite-mail      # tout seul : facture piégée puis câble débranché, faux partage Microsoft : 1400 points
.\build-web.ps1                                  # export web dans dist/ (wasm-bindgen-cli 0.2.129 requis)
```

## Intégration GeoGaming

Le jeu tourne dans une iframe d'une activité Widget et envoie à la fenêtre parente `{ completed: true, points }`, que l'app relaie pour valider l'étape. Trash Toss l'envoie 2 s après le dernier déchet ; Bureau propre et Boîte mail quand le joueur a lu le débrief et touché l'écran. `web/geogaming-test.html` reproduit ce pont en local (`?src=` pour tester une autre adresse).

`.cargo/config.toml` partage le dossier de compilation avec `../bevy-sandbox` : à retirer sur une autre machine.

Police : Montserrat, licence SIL OFL (`assets/fonts/OFL.txt`).
