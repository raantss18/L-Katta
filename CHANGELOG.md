# Journal des versions

Règle : chaque chiffre va de 0 à 9 (0.0.9 → 0.1.0, 0.9.9 → 1.0.0).

## 0.0.7 — 2026-11-08

- **Animation bien plus fluide**, même sur une machine modeste : une image coûte ~2 ms au
  lieu de ~36 (composition limitée au personnage, bulles mises en cache, conversion des
  couleurs par mots, surface déplacée seulement quand il le faut, rythme calé sur
  l'affichage du compositeur ; 60 images/s pour les mouvements rapides, 30 sinon).
  `LKATTA_PERF=1` affiche le temps par image.
- **Il grimpe à une liane** : quand il monte le long d'un bord ou vers l'écran du dessus,
  une liane se déroule depuis le haut ; il y grimpe debout, une main après l'autre, puis
  elle se rembobine. Au tronc de l'arbre, même façon de grimper.

## 0.0.6 — 2026-11-08

- **Rendu net et lisible** : dessin à la densité réelle de chaque écran (2× sur un écran à
  125 %), texte et bulles plus grands (× 1,2 par défaut, réglable : « Taille du texte »).
- **Plus rien de coupé** : la zone de dessin s'agrandit selon le contenu ; le panneau de
  réglages s'ajuste à chaque onglet, avec une vraie barre de défilement ; la bulle se pose
  au-dessus du personnage (oreilles comprises).
- **Il te suit d'un écran à l'autre** : sur l'écran où tu travailles (souris, fenêtre
  active selon KWin), par un saut si les écrans se touchent, sinon dans un petit nuage ;
  et tu peux l'emporter d'un écran à l'autre en le glissant. Réglable dans Apparence.

## 0.0.5 — 2026-11-08

- **Mails et agenda le plus simplement possible** : onglet Mails guidé (Gmail en
  2 étapes avec la page Google ouverte d'un clic ; Google Agenda par l'adresse iCal
  secrète). Le mot de passe d'application s'accepte avec ou sans espaces.
- **Lecture sans effort** : bulle « Nouveau mail » (bouton Résumer), agenda du jour dans
  l'accueil, alerte avant chaque rendez-vous (15 min, réglable) ; `/mails` et `/agenda`.
- **Relance fiable** après changement d'espèce ou de taille (le maki ne disparaît plus).
- Personnalité par défaut neutre sur l'espèce ; sudo dans un terminal visible.

## 0.0.4 — 2026-11-08

- **Panneau de réglages** dans la bulle (roue dentée ou `/reglages`) : Assistant, Bulles,
  Apparence, Démarrage, Mails, Clés d'API. Appliqué tout de suite.
- **Clés d'API comme dans Tontolo** : OpenRouter, Groq, Gemini, Mistral, Cerebras,
  GitHub Models, SambaNova, NVIDIA, Together, Hugging Face ; « Obtenir une clé », clé dans
  le trousseau, modèles ajoutés au moteur intégré.
- **Mails depuis le panneau** : ajouter, tester, retirer ; agenda et contacts.
- **Thème des bulles** : comme KDE, clair ou sombre.
- **Sécurité** : les clés ne sont plus jamais écrites dans le catalogue ; clé invalide
  ignorée ; clé refusée = fournisseur en pause 12 h.
- **AppImage** : plus de fenêtre AppImageLauncher (démarrage, relance, mise à jour).

## 0.0.3 — 2026-11-08

- **Indépendant de Tontolo** : moteur d'IA intégré, porté de Tontolo. Il choisit seul le
  meilleur modèle gratuit selon la demande, passe au suivant s'il est saturé et apprend
  lesquels sont fiables. Aucun compte requis : fournisseurs gratuits sans clé (LLM7,
  OVHcloud, Kilo, Pollinations), plus OpenRouter si une clé existe et Ollama en local.
  Catalogue de secours embarqué, rafraîchi en ligne toutes les 12 h (`lkatta catalogue`).
- **Sélecteur de modèles** dans la bulle, comme dans Tontolo : catégories triées par
  qualité, « Automatique » en premier, modèles en pause ou sans outils signalés.
- **AppImage** : tout en un seul fichier (`LKatta-0.0.3-x86_64.AppImage`) ; installation
  et mises à jour de l'AppImage elle-même.

## 0.0.2 — 2026-11-08

- **Six lémuriens au choix**, chacun avec son allure et sa démarche : maki catta, propithèque
  (danse de côté debout), indri (bonds debout, chant), aye-aye (doigt qui tapote, paupières
  lourdes le jour), microcèbe (trottine par à-coups), vari (collerette, cris).
  `/especes`, `/regle espece …` : il se transforme dans un nuage.
- **Plus sobre** : il s'arrête et s'assoit dès qu'il a quelque chose à dire ; moins de
  promenades, numéros espacés de 15 min, réactions sans bonds.
- **Bulles soignées** : accueil selon le moment, astuces sans répétition, rappels, alertes,
  nouvelles, réponses ; icône et couleur par genre, texte qui s'écrit, boutons utiles
  (« Dans 10 min », « Qu'en faire ? »…) ; expressions et réactions du maki.
- **Règles strictes** par machine (`/etc/lkatta`) et par utilisateur (`~/.config/lkatta`),
  en tête du prompt et appliquées par le code ; contenus externes marqués comme données.
- **Konsole** : lancer une commande dans un onglet visible (sudo tapé par l'utilisateur),
  lire le terminal, y écrire.
- **Mails** IMAP/SMTP (trousseau, lecture sans marquer lu, envoi validé), **agenda** iCal,
  **contacts** vCard.
- **Réglages** depuis la bulle et le terminal : `/reglages`, `/regle`, `/modeles`.
- **Mise à jour interne** vérifiée par SHA-256 (`lkatta maj`, bouton dans une bulle) et
  **démarrage automatique** réglable (`lkatta demarrage on|off`).

## 0.0.1 — 2026-11-08

Première release : le maki est là.

- Maki catta vectoriel qui vit sur le bureau (Wayland layer-shell, KDE Plasma) : marche,
  bonds, étirements, grattage, sommeil la nuit ; physique 2D avec rebonds et redressement.
- Numéros : arbre qui pousse et où il grimpe, escalade des bords, suspendu en haut de l'écran
  puis lâché, téléportation dans un nuage, salto. Glisser-lancer n'importe où.
- Reste près des bords pour laisser la zone de travail libre ; passe d'un écran à l'autre ;
  retient sa place sur chaque bureau virtuel KDE.
- Bulle de discussion sans fenêtre (le clavier seulement après un clic dedans).
- Cerveau : moteur Tontolo démarré tout seul, repli OpenRouter gratuit puis Ollama ;
  25 outils (système, santé, fichiers, commandes confirmées, applications, web, actualités,
  rappels, presse-papiers, mémoire, bureaux virtuels).
- Outils repris de mi-saina et portés en Rust : bilan de santé, diagnostics de commandes,
  garde-fous, applications par nom approximatif, mémoire des préférences.
