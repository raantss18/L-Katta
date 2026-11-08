# L Katta — feuille de route

Versions : chaque chiffre va de 0 à 9 ; **0.0.9 → 0.1.0**, **0.9.9 → 1.0.0**.
Une release = un chiffre de plus. Les jalons ci-dessous sont indicatifs.

## ✅ 0.0.1 — le maki est là

- [x] Dépôt, licence Apache-2.0 (dérivé de Tontolo, outils repris de mi-saina), Rust 2024
- [x] Maki vectoriel : marche, bonds, assis, salut, s'étire, se gratte, dort, vol, escalade,
      suspendu ; contour clair lisible sur fond sombre
- [x] Physique 2D : gravité, rebonds, écrasement/étirement, queue à ressort, redressement en l'air
- [x] Numéros : arbre qui pousse (il grimpe, s'assoit sur la branche, saute), escalade des
      bords, suspendu en haut puis lâché, téléportation dans un nuage, salto
- [x] Glisser-lancer n'importe où ; agrippé aux bords ; suspendu en haut
- [x] Zone de travail libre : il reste sur les bandes de bord (`zone = "bords"`)
- [x] Plusieurs écrans : passage par les côtés, chute vers l'écran du dessous, montée
- [x] Bureaux virtuels KDE : place retenue par bureau ; outils lister / changer de bureau
- [x] Bulle de discussion sans fenêtre : historique, saisie, Oui/Non, copie, commandes « / »
- [x] Cerveau : Tontolo (démarré tout seul) → OpenRouter gratuit → Ollama ; 25 outils
- [x] Outils mi-saina portés : bilan de santé, diagnostics, garde-fous, applications, mémoire
- [x] Fiche machine (dossiers réels, écrans, bureaux) + prompt système éditable par ordinateur
- [x] Rappels + notifications ; presse-papiers (mots de passe exclus) ; veille (santé, Arch)
- [x] CLI : `dire`, `demander`, `rappel`, `etat`, `arbre`, `ecran`, `installer`…

## ✅ 0.0.2 — personnages, bulles, règles, mails

- [x] Six lémuriens (catta, sifaka, indri, aye-aye, microcèbe, vari) et démarches propres
- [x] Mouvements sobres : immobile quand il parle, numéros espacés
- [x] Bulles typées, boutons d'action, accueil et astuces, expressions du maki
- [x] Règles strictes machine + utilisateur appliquées par le code
- [x] Konsole (lancer, lire, écrire) ; mails IMAP/SMTP ; agenda iCal ; contacts vCard
- [x] Réglages depuis la bulle ; mise à jour interne ; démarrage automatique réglable

## ✅ 0.0.3 à 0.0.7 — autonomie, réglages, confort

- [x] Moteur d'IA intégré (porté de Tontolo), sans compte ; sélecteur de modèles
- [x] AppImage tout-en-un, mise à jour interne vérifiée (SHA-256), démarrage automatique
- [x] Panneau de réglages dans la bulle ; clés d'API (10 fournisseurs) ; thèmes
- [x] Mails et agenda guidés ; bulle « nouveau mail » ; agenda du jour ; alerte avant RDV
- [x] Rendu net (densité de l'écran), texte réglable, rien de coupé
- [x] Suit l'écran où l'on travaille ; glisser d'un écran à l'autre
- [x] Animation fluide (~2 ms/image) ; lianes pour grimper

## À vérifier en conditions réelles

- [ ] Fluidité et mémoire sur le Dell (8 Go, MX250) : `LKATTA_PERF=1 lkatta`
- [ ] Suivi d'écran et glisser entre écrans, à la souris
- [ ] Un vrai compte Gmail et un vrai agenda branchés ; alerte avant un rendez-vous
- [ ] Konsole : lancer, lire, écrire ; une commande sudo tapée dans le terminal

## 0.0.8 — discrétion intelligente ✔

- [x] Se taire et se faire petit pendant un plein écran, une présentation, une visio
- [x] Respecter « Ne pas déranger » de Plasma
- [x] S'endormir quand on s'absente (`ext-idle-notify-v1`), se réveiller au retour
- [x] Éviter la fenêtre active (script KWin → géométrie des fenêtres) plutôt que des bandes fixes

## 0.0.9 — discussion au quotidien ✔

- [x] Réponses qui s'écrivent au fil de l'eau (streaming)
- [x] Markdown simple dans la bulle (gras, listes, code), liens cliquables
- [x] Curseur de saisie mobile, sélection et copie de texte dans la bulle
- [x] Raccourci clavier global pour ouvrir la bulle (KGlobalAccel)
- [x] Historique des discussions retrouvable (`/historique`)

## 0.1.0 — première version grand public

- [ ] Assistant de premier lancement (prénom, personnage, mail, agenda, discrétion)
- [ ] Intégration continue GitHub (tests + AppImage à chaque version)
- [ ] Anglais dans l'app : langue suivie automatiquement (`LANG`), réglable (`langue = auto|fr|en`) ;
      bulles, réglages, astuces, commandes, prompt système (réponses dans la langue choisie)
- [ ] Site en anglais : page `/en/` avec sélecteur FR · EN, générée et mise à jour comme la page française
- [x] Site de présentation (GitHub Pages, à jour à chaque version)

## Ensuite (0.1.x)

- **Notifications et presse-papiers** : résumer les notifications ; traduire, résumer,
  corriger une copie d'un clic
- **Lire l'écran** : capture sur demande → modèle de vision ; texte de la fenêtre active
  (AT-SPI) ; « explique-moi cette erreur »
- **Mails et agenda, suite** : brouillons de réponse, pièces jointes, créer un événement,
  Google sans mot de passe d'application (OAuth), contacts synchronisés
- **Connaître la machine** : index local léger (SQLite FTS5), inventaire, tâches
  récurrentes et liste de choses à faire
- **Maintenance** : bilan hebdomadaire, nettoyage guidé (reprise de home-cleaner),
  sauvegardes
- **Personnage** : marcher sur le haut des fenêtres et sauter de l'une à l'autre,
  nouvelles animations
- **Distribution** : AUR, Flatpak ; repli X11 et GNOME (sans layer-shell)
