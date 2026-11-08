# L Katta — analyse du projet

> Un petit maki catta qui vit sur le bureau, connaît **cette** machine et aide sans déranger.
> Dans l'esprit du trombone d'Office, mais général, utile et discret ; cerveau : le moteur de Tontolo.

## 1. Ce qui est demandé

| Besoin | Traduction technique |
|---|---|
| Personnage qui se balade, fait des acrobaties, dit bonjour | Surcouche transparente au-dessus des fenêtres, animée, cliquable seulement sur le personnage |
| Moteur de Tontolo, personnalisé par ordinateur | Client de l'API compatible OpenAI de Tontolo (`/v1/chat/completions`, `model="auto"`), prompt système = personnalité + fiche machine mesurée |
| Maintenance, recherche fichiers/Internet, actualité, création de fichiers | Outils appelables par le modèle (*function calling*) exécutés localement |
| Notifications, presse-papiers, rappels | Fils de fond du démon + D-Bus (notifications) + wl-clipboard |
| Mails, calendrier, contacts | Connecteurs (IMAP/CalDAV/CardDAV, Akonadi pour KDE) |
| Lire l'écran et son contenu | Capture (portail / Spectacle) + OCR ou modèle de vision ; arbre d'accessibilité AT-SPI |
| Gestion de tâches, connaît tout de la machine | Index local (fichiers, paquets, services, applis) + outils d'action avec confirmation |
| Discret, agréable, intelligent ; léger pour 8 Go | Démon Rust ~12 Mo, panneau séparé lancé à la demande, aucune IA locale obligatoire |

## 2. Choix d'architecture

```
┌──────────────────────────── lkatta (un seul processus, ~15 Mo) ────────────────────────────┐
│ surcouche Wayland layer-shell : une sonde invisible par écran (zone utile hors panneaux)   │
│   + une petite surface transparente qui suit le maki (marges), clics hors maki ignorés     │
│ maki : physique 2D (gravité, rebonds, redressement), arbre, escalade, téléportation         │
│ bulle de discussion : dessinée par le maki, clavier seulement après un clic dedans          │
│ cerveau (fil) : boucle d'outils → moteurs compatibles OpenAI, dans l'ordre :               │
│   1. Tontolo local :8765 (démarré tout seul)  2. OpenRouter gratuit  3. Ollama             │
│ fils de fond : socket de commande, rappels, presse-papiers, veille santé, bureaux KDE      │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Rust**, comme Fanoratana/mahay : binaire unique, sans Python ni boîte à outils graphique.
- **Pas de fenêtre** : le panneau egui des premiers essais (~100 Mo) a été remplacé par une
  bulle dessinée par le maki lui-même.
- **layer-shell** (KWin, Sway, Hyprland…) : la surface flotte au-dessus des fenêtres, sur
  tous les bureaux virtuels ; sa *région d'entrée* se limite au maki et à ses bulles.
- **Écrans** : une sonde par écran mesure la zone utile ; la géométrie des écrans
  (`wl_output`) donne les voisins ; pour changer d'écran, la surface est recréée sur l'autre.
- **Bureaux virtuels** : suivis par le D-Bus de KWin (`gdbus monitor`) ; le maki retient sa
  place sur chaque bureau.
- **Tontolo reste le moteur** ; **mi-saina** a fourni ses outils, portés en Rust (plus de
  dépendance à son serveur) : bilan de santé, diagnostics de commandes, garde-fous,
  applications par nom approximatif, mémoire des préférences.
- **Sécurité** : lecture seule directe ; modification = confirmation ; `sudo` jamais exécuté ;
  commandes destructrices refusées ; mots de passe copiés ignorés ; fichiers en 600.

## 3. Discrétion (règles du maki)

1. Il se tient près des bords gauche et droit (zone `bords`) : le milieu reste à toi.
2. Une remarque spontanée au plus toutes les 25 min, jamais en mode discret ni caché.
3. Les rappels passent toujours (bulle + notification).
4. Pas de numéro (arbre, escalade, téléportation…) les 2 premières minutes, puis au plus
   un toutes les 6 min environ (`numeros_min`).
5. Il dort la nuit (23 h–7 h par défaut) et se réveille pour un rappel.
6. Survolé, il s'arrête ; clic droit = discret ; il ne prend jamais le clavier de lui-même.

## 4. Risques et réponses

| Risque | Réponse |
|---|---|
| GNOME ne gère pas layer-shell | Repli prévu : fenêtre sans décoration toujours au-dessus (extension GNOME en option) |
| X11 | Repli prévu : fenêtre *override-redirect* + forme (XShape) |
| Écrans HiDPI (fractionnaire) | Rendu à l'échelle de la sortie (`wp_fractional_scale`) — à faire |
| Plusieurs écrans | Fait : passage par les bords jointifs ; écrans non jointifs : téléportation |
| Cache la zone de travail | Fait : bandes de bord ; à venir : éviter la fenêtre active (script KWin) |
| Moteur lent au premier démarrage | Tontolo démarré en tâche de fond au lancement du démon (à faire), repli OpenRouter |
| Modèles sans appel d'outils | Tontolo choisit des modèles compatibles ; repli « sans outils » à prévoir |
| Lecture d'écran = vie privée | Uniquement sur demande explicite, image jamais conservée |
| Mails / agenda = données sensibles | Lecture d'abord, envoi toujours confirmé, identifiants dans le trousseau (Secret Service) |
