<h1 align="center">L Katta</h1>

<p align="center"><b>Lemur Kata — un petit maki catta qui vit sur ton bureau Linux.</b><br>
Il connaît <i>ta</i> machine, t'aide sans te déranger et se balade d'un écran à l'autre.</p>

<p align="center"><img src="docs/apercu.png" alt="Les poses du maki" width="900"></p>

Dans l'esprit du trombone des anciens Office, mais utile et discret. Son cerveau est le
moteur de [Tontolo](https://github.com/raantss18/tontolo) (le meilleur modèle d'IA gratuit,
choisi automatiquement) ; ses outils de maintenance viennent de
[mi-saina](https://github.com/raantss18/mi-saina).

## Ce qu'il fait

- **Il vit sur l'écran, sans fenêtre** : il marche, bondit, s'étire, se gratte la tête,
  dort la nuit. Il fait pousser un arbre et y grimpe, escalade les bords de l'écran, se
  suspend en haut puis se laisse tomber, disparaît dans un nuage et réapparaît ailleurs.
- **Il laisse ta zone de travail libre** : il se tient près des bords gauche et droit.
- **Plusieurs écrans, plusieurs bureaux** : il passe d'un écran à l'autre (par le côté,
  ou en sautant dans le vide vers l'écran du dessous) et retrouve sa place sur chaque
  bureau virtuel.
- **Tu l'attrapes et tu le lances** : il se redresse en l'air comme un chat, rebondit,
  s'agrippe à un bord ou se suspend en haut de l'écran.
- **On discute dans sa bulle** : clic sur le maki. Il cherche des fichiers, fait le bilan
  de santé de la machine, lance une application, lit l'actualité, cherche sur Internet,
  crée des fichiers, programme des rappels, gère le presse-papiers, change de bureau.
- **Il ne fait rien de risqué sans toi** : lecture seule directe ; écrire ou lancer une
  commande qui modifie = bouton Oui/Non ; jamais de `sudo` (commande copiée pour toi) ;
  commandes destructrices refusées.
- **Léger** : un seul binaire, ~15 Mo de mémoire, rendu logiciel, aucune IA locale exigée.

## Installation

```bash
cargo build --release
./target/release/lkatta installer   # ~/.local/bin + démarrage de session + lanceur
lkatta &
```

Ou depuis une [release](https://github.com/raantss18/L-Katta/releases) : décompresser puis
`./lkatta installer`.

Prérequis : bureau Wayland avec *layer-shell* (KDE Plasma, Sway, Hyprland…), `wl-clipboard`.
Pour l'IA : Tontolo (démarré tout seul s'il est installé), sinon une clé OpenRouter
gratuite, ou Ollama.

## Souris et commandes

| Geste | Effet |
|---|---|
| clic | ouvre / ferme la bulle de discussion |
| glisser-lâcher | l'attraper et le lancer |
| clic droit | mode discret (pas de numéros, pas de remarques) |
| clic molette | un numéro au hasard |
| clic sur une bulle | la fermer ; sur une réponse : la copier |

Dans la bulle : `/aide`, `/rappels`, `/presse`, `/sante`, `/etat`, `/apps`, `/moteurs`,
`/retiens …`, `/nouveau`. Clique dans le champ pour écrire : la bulle ne prend jamais le
clavier d'elle-même.

```bash
lkatta dire "Coucou"        lkatta demander "quel dossier prend le plus de place ?"
lkatta rappel 18h30 appeler Hery     lkatta arbre | teleporter | grimper | ecran
lkatta discret on           lkatta etat | actualites | profil | quitter
```

## Personnaliser

- `~/.config/lkatta/config.toml` : prénom, taille, zone (`bords` ou `libre`), fréquence des
  numéros, heures de sommeil, moteurs d'IA, flux d'actualité.
- `~/.config/lkatta/prompt.md` : sa personnalité et ses règles **pour cet ordinateur**.
- `~/.config/lkatta/machine.md` : la fiche mesurée de la machine (`lkatta profil`).
- `~/.config/lkatta/profil.md` : ce qu'il a retenu de toi.

## Versions

Chaque chiffre va de 0 à 9 : 0.0.9 → 0.1.0, 0.9.9 → 1.0.0.
Feuille de route : [docs/TODO.md](docs/TODO.md) · analyse : [docs/ANALYSE.md](docs/ANALYSE.md).

## Licence

Apache-2.0, comme Tontolo. Voir [NOTICE](NOTICE).
