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

## Non corrigé, à décider

- **Bac à sable.** Les commandes acceptées par l'utilisateur tournent avec ses droits. Un bac à
  sable (bubblewrap, dossiers interdits invisibles) fermerait ce qui reste d'astuces de shell.
- **Presse-papiers et notifications** : le modèle peut les lire sans demande d'accord (les mots de
  passe signalés par les gestionnaires sont déjà écartés).

## Reste à tester

- Sur un vrai compte : réponse, brouillon et pièces jointes (aucun compte n'était branché sur la
  machine d'essai ; ces fonctions sont couvertes par des tests sur des messages d'exemple).
- Sur GNOME, Sway et Hyprland : inventaire des écrans, dossier Téléchargements localisé.
