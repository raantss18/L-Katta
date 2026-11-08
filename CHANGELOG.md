# Journal des versions

Règle : chaque chiffre va de 0 à 9 (0.0.9 → 0.1.0, 0.9.9 → 1.0.0).

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
