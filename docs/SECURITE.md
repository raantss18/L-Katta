# Sécurité — audit du 2026-11-14 (version 0.2.0)

Méthode : lecture du code par un relecteur indépendant (outils, règles, mails, mise à jour, socket,
index, agenda, web), puis `cargo audit` (base RustSec, 228 dépendances de `Cargo.lock`).

## Dépendances

- **Aucune vulnérabilité connue** dans ce qui est embarqué (`cargo audit`, 2026-11-14).
- Un avertissement sans faille : `ttf-parser` 0.25.1 n'est plus maintenu
  (https://rustsec.org/advisories/RUSTSEC-2026-0192) ; il ne lit que les polices fournies avec L Katta
  et celles du système. À remplacer quand `ab_glyph` proposera un successeur.

## Corrigé

- **Commandes dites « lecture seule » lancées sans accord.** `env`, `awk`, `fd`, `rg --pre`,
  `find -fprintf`, `sort -o`, `git --output`, `nmcli … off`… permettaient d'exécuter ou d'écrire
  n'importe quoi sans confirmation. La liste est réduite, chaque programme n'accepte que des
  arguments sûrs, et toute commande contenant variable, joker, substitution, redirection ou
  échappement est montrée à l'utilisateur. 31 contournements sont rejoués par les tests.
- **Dossiers interdits contournables** (`cat .ssh/x`, `~//.ssh`, `Documents/../.gnupg`). Les chemins
  sont résolus comme le programme les verrait (relatifs, `..`, `//`, liens). Une liste d'interdits
  s'applique toujours, quelles que soient les règles : clés SSH et GPG, trousseaux, identifiants
  git/AWS/Azure/gcloud/kube/docker, cookies et mots de passe des navigateurs, réglages et données
  de L Katta, démarrage automatique de la session.
- **Fuite par une adresse web.** Les adresses de cette machine et du réseau local (boucle locale,
  10/8, 192.168/16, lien local, Tailscale, `.local`) sont refusées, redirections comprises ; une
  adresse qui emporte beaucoup de données est montrée avant d'être ouverte.
- **Injection qui dure.** « Retenir » demande l'accord (ce qui est retenu revient dans chaque
  discussion). Le contenu d'un fichier, la sortie d'une commande et un résultat de recherche sont
  marqués comme des données, jamais des instructions.
- **Mails.** Plus aucun retour à la ligne dans un en-tête (un objet piégé ne peut plus ajouter de
  destinataire caché), copies validées aussi pour les brouillons, identifiants de fil filtrés,
  parties emboîtées bornées (un mail ne peut plus figer ni faire tomber le maki), fichier
  d'identifiants unique par appel et toujours retiré.
- **Demandes d'accord qui cachaient du contenu.** Le fichier à écrire est montré ; un texte long
  affiche son début, sa fin et le nombre de caractères passés. Le terminal visé est nommé.
- **Accord d'une seule touche.** Une touche tapée dans la seconde où la demande apparaît, ou en
  pleine frappe, est ignorée.
- **Côté Discussion** : les outils d'écriture sont refusés à l'exécution, pas seulement retirés
  de la liste.
- **Agenda distant** : intervalles bornés, plus de débordement de date.
- **Fichiers lisibles par les autres comptes** : dossiers de données et de réglages en 700,
  index et journaux en 600 ; fichiers temporaires uniques.
- **Socket** : un client muet ne bloque plus les suivants.
- **Intégration continue** : actions GitHub épinglées par empreinte de commit.

## Signature des versions (0.2.1)

- Chaque version publie `SHA256SUMS` et `SHA256SUMS.sig` : la signature Ed25519 des empreintes.
  La clé publique est dans le binaire (`src/maj.rs`) ; la clé privée est dans
  `release-keys/ed25519.pem`, hors du dépôt. Une mise à jour dont la signature ou l'empreinte
  ne correspond pas est refusée, comme une version sans signature.
- Publier : `tool/publier.sh notes.md` (signe et contrôle avec le binaire lui-même).
- **À faire par l'auteur : garder une copie de `release-keys/ed25519.pem` hors de
  l'ordinateur.** Si la clé est perdue, les L Katta installés ne pourront plus se mettre à
  jour seuls (il faudrait réinstaller à la main une version portant une nouvelle clé). Si elle
  est volée, quelqu'un pourrait signer une fausse version : il lui faudrait aussi le compte
  GitHub pour la publier.
- Les versions 0.2.0 et antérieures vérifient seulement l'empreinte : elles passeront à la
  0.2.1 comme avant, et seront protégées ensuite.

## Extension GNOME (0.3.0)

- Installée seulement à la demande de l'utilisateur (`/gnome`, `lkatta gnome`), dans
  ~/.local/share/gnome-shell/extensions ; `/gnome retirer` l'enlève.
- Elle ne lit pas le contenu des fenêtres, ne tape rien, n'ouvre aucun accès réseau. Elle envoie
  au seul service `org.lkatta.Maki` le titre, l'application, la position et le plein écran de la
  fenêtre active, sans démarrage automatique.
- Son objet D-Bus (`/org/lkatta/Gnome` chez org.gnome.Shell) liste les fenêtres et les
  active, réduit, agrandit ou ferme : comme kdotool sous KDE, tout programme de la session de
  l'utilisateur peut s'en servir. Pas d'accès depuis une autre session ni un autre utilisateur.
- Raccourci global sous GNOME : un raccourci personnalisé (`custom-keybindings` de
  org.gnome.settings-daemon.plugins.media-keys, chemin `.../custom-keybindings/lkatta/`) qui
  lance `lkatta raccourci`. Une combinaison déjà prise par un autre raccourci personnalisé n'est
  jamais remplacée ; `lkatta desinstaller` le retire.

## Paquets (0.3.1)

- **Flatpak : pas de bac à sable.** L Katta a besoin des programmes de la machine (terminal,
  gdbus, presse-papiers, gestionnaire de paquets) : il les lance par `flatpak-spawn --host`
  (`--talk-name=org.freedesktop.Flatpak`, bus de session entier, dossier personnel). Ses droits
  sont ceux de l'utilisateur, comme l'AppImage ; le manifeste et la fiche AppStream le disent.
  Les règles de l'administrateur sont lues dans /run/host/etc/lkatta (accès host-etc en lecture).
- **Installé par pacman ou Flatpak** : la mise à jour intégrée ne remplace rien (`hote::gestionnaire`),
  le gestionnaire de paquets garde la main sur /usr/bin et /app.
- **Dépôt Flatpak** (site, `/flatpak/`) : versions et sommaire signés par la clé GPG du dépôt
  (`tool/depot-flatpak.sh`) ; la clé publique est dans le fichier `.flatpakrepo`, la clé privée
  ne quitte pas le trousseau de la personne qui publie (jamais dans Git ni dans la CI).
  L'archive du dépôt figure aussi dans le SHA256SUMS signé de la version.
- **AUR** : `lkatta-bin` vérifie l'archive par son SHA-256, pris dans le SHA256SUMS signé.

## Ménage et sauvegardes (src/menage.rs, src/sauvegarde.rs)

- **Ménage** : l'analyse ne fait que lire. Nettoyer demande toujours l'accord, avec la liste
  exacte et ce qui arrivera à chaque élément. Seul ce qui se recrée tout seul est supprimé
  (liste fermée de caches sous ~/.cache ; dossiers de compilation reconnus par leur fichier de
  projet, projet inactif depuis 30 jours) ; le reste va à la corbeille freedesktop (restaurable) ;
  vider la corbeille est annoncé « irréversible ». Avant chaque action : chemin dans le dossier
  personnel, pas un lien symbolique, pas de `.stfolder` (Syncthing) au-dessus, pas un dossier
  interdit par les règles ; jamais dans une arborescence système (rootfs, chroot, usr…).
  Journal `menage.log`.
- **Sauvegardes** : rien sans destination réglée. Refus si la destination est dans un dossier
  sauvegardé (ou l'inverse). Copies écrites sous un nom `.partiel`, renommées à la fin ; seules
  les copies de L Katta (`lkatta-sauvegardes/AAAA-MM-JJ_HHMM`) sont effacées par l'élagage.

## Autonomie du côté Code (src/autonomie.rs)

Un accord, demandé une fois par dossier de projet et valable jusqu'à l'arrêt de L Katta, remplace
la question posée à chaque fichier et à chaque commande. Ce que le code garantit, quel que soit
le modèle :

- **Jamais pour tout le disque** : ni `/`, ni le dossier personnel, ni un de ses parents.
  `autonomie = false` ou `confirmer_tout = true` (règles de la machine ou de l'utilisateur)
  suppriment l'accord.
- **Fichiers** : sans question seulement dans le dossier, liens symboliques résolus ; jamais dans
  `.git` ni sur un fichier de secrets (`.env`, `*.pem`, `*.key`, `id_*`…). Un fichier remplacé
  est d'abord copié dans `sauvegardes/` (300 dernières copies).
- **Commandes** : sans question seulement dans un bac à sable bubblewrap : disque en lecture
  seule sauf le projet et `~/.cache`, pas de réseau, pas de bus de session ni d'agent SSH,
  dossiers interdits remplacés par du vide. Sans bubblewrap, chaque commande est demandée.
- **Toujours demandé** (`autonomie::gardee`) : suppression et écrasement sans retour, Git qui
  jette ou qui sort (`reset`, `clean`, `checkout`, `push`, `pull`…), réseau, paquets, services,
  autres processus, publication ; et toute commande que le modèle veut lancer hors du bac
  (`hors_bac`).
- **Limite connue** : dans le projet, une commande du bac peut écraser un fichier (`mv`, `>`,
  `sed -i`) sans copie de sauvegarde ; la liste des commandes gardées est une liste de noms, le
  bac est la vraie barrière pour ce qui est hors du projet.

## Non corrigé, à décider

- **Bac à sable hors accord.** Les commandes acceptées une par une par l'utilisateur tournent
  avec ses droits, hors du bac.
- **Presse-papiers et notifications** : le modèle peut les lire sans demande d'accord (les mots de
  passe signalés par les gestionnaires sont déjà écartés).

## Reste à tester

- Sur un vrai compte : réponse, brouillon et pièces jointes (aucun compte n'était branché sur la
  machine d'essai ; ces fonctions sont couvertes par des tests sur des messages d'exemple).
- Sur GNOME, Sway et Hyprland : inventaire des écrans, dossier Téléchargements localisé.
