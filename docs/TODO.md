# L Katta — feuille de route

Versions : chaque chiffre va de 0 à 9 ; **0.0.9 → 0.1.0**, **0.9.9 → 1.0.0**.
Une release = un chiffre de plus. Les jalons ci-dessous sont indicatifs.

## Priorités (demandées par Antsa le 2026-11-09, dans cet ordre)

1. **Toutes les bulles se ferment quand on s'occupe d'autre chose** (0.1.5) : la petite
   bulle automatique et celle du clic droit se ferment quand une autre fenêtre ou le bureau
   devient actif, au changement de bureau virtuel, ou quand le pointeur la quitte après
   l'avoir survolée. Limite de Wayland : un clic *dans* la fenêtre déjà active n'est pas
   visible sans prendre le clavier (ce qu'une bulle automatique ne fait jamais).
2. **Lire l'écran** (0.1.5) : capture sur demande (portail ou spectacle), modèle de vision,
   « explique-moi ce qui est à l'écran / cette erreur » ; texte de la fenêtre active.
3. **Notifications et presse-papiers** (0.1.6) : résumer les notifications reçues ;
   traduire, résumer, corriger, expliquer le dernier texte copié d'un clic.
4. **Système plus robuste, mémoire de discussion** (0.1.7) : idées reprises de mi-saina et
   de Tontolo (voir docs/MEMOIRE.md une fois l'étude faite) : souvenirs durables, résumés
   des longues discussions, rappel des échanges pertinents, reprise après coupure.
   ✔ fait (0.1.5) : fiche, souvenirs, rappel par le sens, garde-fous, moteur robuste, suite des
   réponses coupées, messages en attente.
5. **Version Android à étudier** (réflexion écrite d'abord, docs/ANDROID.md) : ce qui a
   du sens sur un téléphone, ce qui n'en a pas, technique possible, coût.

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

## 0.1.0 — anglais et nouvelle allure ✔

- [x] Anglais dans l'app : langue suivie automatiquement (`LANG`), réglable (`langue = auto|fr|en`) ;
      bulles, réglages, astuces, commandes, prompt système (réponses dans la langue choisie)
- [x] Site en anglais : page `/en/` avec sélecteur FR · EN, générée et mise à jour comme la page française
- [x] Site de présentation (GitHub Pages, à jour à chaque version)
- [x] Nouveau maki au style de l'identité visuelle (contour épais, grands yeux) et émotions
      (parle, danse, boude, fier)
- [x] « À propos » avec les coordonnées de l'autrice

## 0.1.1 — personnalités ✔

- [x] Chaque espèce : rythme (nocturne), habitat (arboricole), émotions à sa façon

## 0.1.2 — correction ✔

- [x] Le maki ne monte plus quand la bulle ou les réglages s'ouvrent (deux écrans superposés)

## 0.1.3 — premier lancement ✔

- [x] Assistant de premier lancement (langue, prénom, personnage, mail, agenda, discrétion)
- [x] Intégration continue GitHub (tests + AppImage à chaque version)
- [x] Rappels en anglais dans la bulle (« tomorrow », « in 10 min »)
- [x] Modèle local de secours conseillé selon la machine (Ollama, qwen3)
- [x] Fournisseurs payants (Anthropic, OpenAI, DeepSeek, xAI), jamais choisis par surprise
- [x] Recherche approfondie avec sources citées ; mode agent (plan, actions, compte rendu)

## 0.1.4 — carnet de naturaliste ✔

- [x] Bulle sur papier de carnet, polices lisibles intégrées, thème sombre « à la veillée »
- [x] La bulle se referme quand on clique ailleurs

## 0.1.8 — mémoire, arrêt, côtés et fils ✔

- [x] Fuite de mémoire corrigée (tampons Wayland bornés) + garde-fou 500 Mo
- [x] Bouton Arrêter, question rendue à la saisie
- [x] Interrupteur Discussion · Code ; dossier de projet côté Code
- [x] Fils de discussion : titres automatiques, historique par date ou par projet, recherche
      par le sens, mémoire propre à chaque fil

## 0.1.9 — danse fluide, cadence réglée sur la machine ✔

## 0.2.0 — connaître la machine, mails et agenda, sécurité, vidéo ✔

- [x] Index local des fichiers (SQLite FTS5) : noms et contenu (textes, code, PDF), fil de
      priorité minimale, seuls les fichiers changés sont relus ; réglage pour le couper
- [x] Inventaire (toute distribution, toute carte graphique, tout bureau)
- [x] Rappels qui reviennent (jour, semaine, mois, jours ouvrés) ; liste de choses à faire
      avec échéances (rappelées dans l'accueil)
- [x] Répondre à un mail (à tous, citation, fil) ; brouillons rangés sur le serveur ; pièces
      jointes : envoyer, voir, enregistrer
- [x] Ajouter et retirer des événements (agenda de L Katta, alerte avant le rendez-vous),
      invitation .ics ; ajouter un contact

## 0.2.1 — versions signées ✔

## 0.2.2 — joindre des fichiers ✔

- [x] Trombone dans la saisie (Ctrl+O), portail du bureau ou zenity/yad/kdialog, pastilles

## 0.2.3 — glisser-déposer de fichiers ✔

## Ensuite (0.2.x)

- **Lire l'écran, suite** : texte de la fenêtre active (AT-SPI) sans capture d'image
- **Comptes en ligne** : Google et Microsoft sans mot de passe d'application (OAuth : il faut
  un identifiant d'application enregistré chez eux) ; CalDAV/CardDAV (Nextcloud, Fastmail…)
  pour écrire dans l'agenda en ligne et synchroniser les contacts
- **Maintenance** : ✔ bilan hebdomadaire (0.1.5) ; nettoyage guidé (reprise de home-cleaner),
  sauvegardes
- **Personnage** : ✔ monter sur la fenêtre active et y flâner (0.1.5) ; ensuite sauter d'une
  fenêtre à l'autre (il faudrait la géométrie de toutes les fenêtres), nouvelles animations
- **Distribution** : AUR, Flatpak ; repli X11 et GNOME (sans layer-shell)
