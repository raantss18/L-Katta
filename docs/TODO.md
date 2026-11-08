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

## 0.0.2 — confort

- [ ] Éviter la fenêtre active (script KWin → géométrie des fenêtres) au lieu de simples bandes
- [ ] Marcher sur le bord haut des fenêtres, sauter de l'une à l'autre
- [ ] HiDPI : rendu à l'échelle de l'écran (`wp_fractional_scale_v1`) — l'écran du portable est à 125 %
- [ ] Glisser le maki d'un écran à l'autre à la souris (aujourd'hui : il y va tout seul)
- [ ] Réponses en flux (streaming), Markdown simple dans la bulle, curseur de saisie mobile
- [ ] Raccourci clavier global pour ouvrir la bulle
- [ ] Démarrer Tontolo dès le lancement (premier message plus rapide)

## 0.0.3 — notifications et presse-papiers intelligents

- [ ] Écouter les notifications (D-Bus) et les résumer ; respecter « Ne pas déranger »
- [ ] Se taire pendant un plein écran, une présentation, une visio
- [ ] Actions sur une copie : traduire, résumer, corriger
- [ ] Inactivité (`ext-idle-notify-v1`) : il s'endort quand tu t'absentes

## 0.0.4 — mails, agenda, contacts

- [ ] IMAP (non-lus résumés), SMTP (brouillon → envoi confirmé)
- [ ] CalDAV / ICS (agenda du jour dans la salutation), CardDAV
- [ ] KDE Akonadi / Thunderbird ; identifiants dans le trousseau (Secret Service / KWallet)

## 0.0.5 — lire l'écran

- [ ] Capture sur demande (portail / Spectacle) → modèle de vision via Tontolo ; OCR en repli
- [ ] Arbre d'accessibilité AT-SPI ; « explique-moi cette erreur » sur la fenêtre active

## 0.0.6 — connaître toute la machine

- [ ] Index local léger (SQLite FTS5) : documents, fichiers récents
- [ ] Inventaire : paquets, services, tâches planifiées, périphériques
- [ ] Tâches récurrentes (comme le planificateur de mi-saina), liste de choses à faire

## 0.0.7 — maintenance assistée

- [ ] Bilan hebdomadaire, nettoyage guidé (reprise de home-cleaner), sauvegardes

## 0.0.8 — personnages

- [ ] Autres animations (bâille, s'étire au soleil face à toi), thèmes / autres animaux

## 0.0.9 — distribution

- [ ] AUR, AppImage, Flatpak ; repli X11 et GNOME ; anglais et malgache ; mise à jour auto

## 0.1.0 — première version grand public

- [ ] Assistant de premier lancement, documentation et site
