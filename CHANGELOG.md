# Journal des versions

Règle : chaque chiffre va de 0 à 9 (0.0.9 → 0.1.0, 0.9.9 → 1.0.0).

## 0.1.5 — à venir

- **Correction : la clé Anthropic et « payants en premier »** donnaient « Aucun moteur n'a
  répondu » : les modèles Claude récents refusent le réglage `temperature` (il n'est plus
  envoyé), la chaîne d'essais était faite seulement de modèles payants (désormais 2 au plus, puis
  des gratuits), et le modèle local sans outils (gemma3) est réinterrogé sans outils.
- **Toutes les petites bulles se ferment quand on travaille ailleurs** (autre fenêtre, bureau,
  changement de bureau virtuel, pointeur qui quitte la bulle après l'avoir survolée).
- **Lire l'écran** : `/ecran [question]`, `lkatta regarder`, et l'outil `regarder_ecran` (avec
  accord) ; capture Spectacle, maki invisible pendant la capture, modèle de vision gratuit.
- **Notifications** écoutées localement et gardées 24 h : `/notifs` les résume ; au retour d'une
  absence ou d'une visio, leur compte par application.
- **Texte copié** : Traduire, Résumer, Corriger, Expliquer d'un clic dans la bulle (et `/traduis`,
  `/resume`, `/corrige`, `/explique`).
- **Mémoire de discussion** (idées de Tontolo et mi-saina, docs/MEMOIRE.md) : fiche qui résume le
  début des longues discussions ; souvenirs durables notés tous les 4 tours, rappelés seulement
  quand ils concernent la question (`/souvenirs`, `/oublie`) ; anciens échanges proches retrouvés
  par le sens, sans réseau ; `/historique` trouve aussi les mots voisins. Jamais de clé, mot de
  passe ni numéro de carte en mémoire ni dans le journal.
- **Moteur plus robuste** : délais `Retry-After` respectés, crédit épuisé reconnu, conversation trop
  longue sans pénalité, réponse vide = échec, hors ligne → modèle local directement, abandon après
  200 s perdues ; appels d'outils identiques non refaits, dernière étape sans outils, discussion
  ajustée à un budget sans séparer un outil de son résultat.
- Réflexion écrite sur une version Android (docs/ANDROID.md).

## 0.1.4 — 2026-11-09

- **Nouvelle allure de la bulle : une page de carnet de naturaliste.** Papier vieilli aux
  bords légèrement déchirés, rousseurs et grain ; encre ferrogallique. Thème sombre « à la
  veillée ». Un seul accent, le vert du ravinala, pour ce qui agit ; le rouge latérite ne
  sert qu'aux erreurs. Les petites bulles de message sont sur le même papier.
- **Plus lisible** : texte en Atkinson Hyperlegible (conçue pour la lisibilité), titres en
  Alegreya, polices intégrées (même rendu partout) ; tailles et espacements agrandis,
  contrastes AA (texte principal à plus de 12:1).
- Réglages : onglets soulignés au lieu de pastilles, titres de section en minuscules,
  champs écrits sur une ligne comme dans un carnet.
- **La bulle se referme quand on clique ailleurs** (autre fenêtre, bureau). Elle reste
  ouverte pendant une question Oui/Non, pendant l'accueil, et après avoir ouvert une page
  ou un fichier dont on doit revenir avec un copier-coller.
- Correction : le surveillant du presse-papiers ne reste plus orphelin après une relance.

## 0.1.3 — 2026-11-09

- **Assistant de premier lancement** dans la bulle, en cinq étapes : langue, prénom,
  personnage (vignettes des six lémuriens), discrétion, mails et agenda. « Passer » ou la
  croix le referment pour de bon ; `/accueil` (`/welcome`) le relance. Les installations
  existantes ne le voient pas.
- **Rappels en anglais** : « in 10 minutes », « tomorrow at 9am », « 6:30 pm », « 2 hours »,
  « 3 days », « 1 week » (et « semaine » en français), dans la bulle comme en ligne de commande.
- **Intégration continue GitHub** : tests à chaque envoi ; à chaque étiquette de version,
  l'AppImage est fabriquée sur une machine propre (artefact du flux).
- **Recherche approfondie** (`/cherche question`, pastille « Recherche » de la bulle,
  `lkatta chercher`) : plusieurs requêtes, pages lues en parallèle (deux au plus par site),
  un second tour si une information manque, réponse qui cite ses sources numérotées.
- **Mode agent** (`/agent tâche`, pastille « Agent », `lkatta agent`) : plan, enchaînement des
  outils jusqu'au bout (30 étapes), vérification, compte rendu. Les confirmations et règles
  strictes restent les mêmes.
- **Modèle local de secours** choisi d'après la machine (mémoire, carte graphique NVIDIA ou
  AMD) : `qwen3` de 0.6b à 14b via Ollama, téléchargé d'un clic (réglages › Modèles d'IA,
  étape « Cerveau » de l'accueil, `/local`). Il est essayé en dernier quand les modèles en
  ligne ne répondent pas, ou sans réseau.
- **Fournisseurs payants** : Anthropic (Claude), OpenAI, DeepSeek, xAI (Grok) ; clé dans le
  trousseau. Leurs modèles ne servent que choisis dans la liste, ou en premier si l'on active
  « Utiliser mes modèles payants en premier » : jamais de facturation par surprise.
- Correction : un changement de personnage pendant que le maki réfléchissait n'était jamais
  appliqué.

## 0.1.2 — 2026-11-09

- **Correction : le maki ne monte plus au milieu de l'écran** quand on ouvre sa bulle ou ses
  réglages. Avec deux écrans l'un au-dessus de l'autre, la bulle qui grandit jusqu'au bord
  « touchait » l'écran voisin et le maki prenait le sol de cet écran-là.
- `LKATTA_DEBUG=1` journalise la géométrie (écran, zone, surface, position du maki).

## 0.1.1 — 2026-11-09

- **Chaque lémurien a sa personnalité**, d'après ses mœurs :
  - **nocturnes** (aye-aye, microcèbe) : siestes et paupières lourdes le jour, plus vifs le soir ;
  - **arboricoles** (aye-aye, indri, vari, microcèbe, sifaka) : ils retournent dans leur arbre
    au bord de l'écran et y restent perchés ; le catta, lui, vit surtout au sol ;
  - **leurs émotions à leur façon** : le catta danse et met les mains sur les hanches, le
    sifaka danse de côté, l'indri chante, l'aye-aye tapote de son long doigt, le vari crie,
    le microcèbe ouvre de grands yeux ; seuls catta, sifaka et indri se tiennent debout —
    les autres parlent et boudent assis (oreilles couchées, sourcils froncés).

## 0.1.0 — 2026-11-09

- **L Katta parle anglais** : bulles, réglages, astuces, rappels, mails, agenda, mises à
  jour, confirmations et ligne de commande. Réglage « Langue » (onglet Assistant) :
  comme le système, français ou English ; le maki répond alors dans cette langue. Les
  commandes ont aussi leur nom anglais (`/settings`, `/history`, `/help`, `lkatta help`…).
  Une installation existante reste en français.
- **Nouvelle allure** : le maki est redessiné au style de l'identité visuelle de L Katta —
  contour sombre épais, grosse tête de trois quarts, grands yeux orangés dans leurs taches
  sombres, masque et ventre clairs, queue épaisse ; les six espèces suivent le même style.
- **Ses émotions** : debout, il parle quand une bulle s'affiche ou qu'une réponse s'écrit ;
  mains sur les hanches quand il a fini ; il danse si tu le caresses longtemps ; il boude,
  bras croisés, quand tu refuses ce qu'il propose.
- **À propos** : nouvel onglet des réglages, `/apropos`, `lkatta apropos` — version,
  licence et coordonnées de l'autrice.
- **Site en anglais** : https://raantss18.github.io/L-Katta/en/ (sélecteur FR · EN).

## 0.0.9 — 2026-11-09

- **Il ne bouge plus pendant qu'on discute** : bulle ouverte ou réponse en cours, il reste
  assis (plus de marche, de numéro, de changement d'écran ou de bureau, de relance).
- **Plus de discussion perdue** : la question et chaque commande lancée sont gardées au fur
  et à mesure ; si le maki s'arrête en plein travail, la bulle les remontre à son retour.
- **Réponses au fil de l'eau** : le texte s'écrit dans la bulle pendant qu'il arrive (et
  dans `lkatta demander`), appels d'outils compris.
- **Markdown dans la bulle** : titres, listes, **gras**, `code`, blocs de code en police
  fixe, liens soulignés qui s'ouvrent d'un clic. Un clic sur une réponse copie le texte
  sans les marques.
- **Vraie saisie** : curseur déplaçable (flèches, Début, Fin, Ctrl + flèches), sélection
  (Maj, Ctrl+A, clic), copier, couper, coller, Suppr.
- **Meta+K** ouvre la bulle, clavier prêt (réglable : `maki.raccourci`, vide = aucun ;
  visible dans Configuration › Raccourcis).
- **/historique** liste les derniers échanges ; `/historique mots` cherche plus loin.
- Correction : les moniteurs `gdbus` ne restent plus orphelins à chaque relance.
- Correction : en ouvrant les réglages, le maki ne remonte plus vers le milieu de l'écran.
- Site de présentation : https://raantss18.github.io/L-Katta/

## 0.0.8 — 2026-11-09 (incluse dans 0.0.9)

- **Discrétion intelligente** : un petit script KWin lui dit quelle fenêtre est active.
  Plein écran sur son écran → il se cache ; visio, présentation, « Ne pas déranger » ou
  micro en cours → il se tait et garde astuces, mails et nouvelles pour plus tard.
- **Il dort quand tu t'absentes** (5 min sans clavier ni souris) et, à ton retour, résume
  ce qu'il a mis de côté.
- **Il s'écarte de la fenêtre active** : il prend le bord qu'elle ne couvre pas.

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
