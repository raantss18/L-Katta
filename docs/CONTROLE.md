# Manipuler l'ordinateur — garde-fous

L Katta peut agir sur ton bureau KDE à ta demande. Les garde-fous ci-dessous sont **appliqués par
le code** (src/controle.rs, src/tools.rs) : un modèle d'IA qui désobéirait ne peut pas les franchir.

## Ce qu'il sait faire

| Outil | Ce qu'il fait | Niveau |
|---|---|---|
| `fenetres_lister` | liste les fenêtres ouvertes | regarder |
| `fenetre` | premier plan, réduire, agrandir, mettre sur un bureau virtuel | réversible |
| `fenetre` (fermer) | demande la fermeture (l'application propose d'enregistrer) | risqué |
| `taper` | tape un texte dans une fenêtre (collé, accents compris) | risqué |
| `touches` | envoie un raccourci (« ctrl+s », « entrée », « f5 ») | risqué |
| `son` | volume, plus, moins, muet | réversible |
| `lecteur` | lecture/pause, suivant, précédent (musique, vidéo) | réversible |
| `luminosite` | luminosité de l'écran | réversible |

Pas de souris pour l'instant : cliquer « au bon endroit » d'après une capture n'est pas assez sûr
avec les modèles gratuits.

## Les garde-fous

1. **Interrupteurs.**
   - Le réglage *Contrôle de l'ordinateur* (Réglages › Assistant) vaut « Me demander »,
     « Confiance » ou « Jamais ».
   - L'administrateur peut l'interdire pour tous les comptes avec `controle = false` dans
     `/etc/lkatta/regles.toml`. Un utilisateur ne peut pas assouplir ce réglage.
2. **Ton accord.**
   - Regarder ne demande rien.
   - Une action réversible est demandée, sauf en « Confiance ».
   - Fermer une fenêtre, taper et les raccourcis sont **toujours** demandés, quel que soit le
     réglage. La question montre la fenêtre exacte (titre et application) et le texte exact.
3. **La bonne fenêtre.**
   - Avant de taper ou d'envoyer des touches, il met la fenêtre visée au premier plan.
   - Il vérifie ensuite qu'elle a bien le clavier ; sinon il n'envoie rien.
   - Il ne tape jamais dans sa propre bulle.
4. **Fenêtres protégées.** Rien n'est tapé et aucune touche n'est envoyée dans les fenêtres
   dont le titre ou l'application contient l'un de ces mots :
   - gestionnaires de mots de passe : keepass, bitwarden, 1password, kwalletmanager, seahorse ;
   - banques et argent : banque, bank, BNI, BFV, BOA, PayPal, MVola, Orange Money, Airtel Money ;
   - pages de connexion : connexion, se connecter, sign in, login, authentification.

   La liste se complète avec `fenetres_interdites` dans `~/.config/lkatta/regles.toml`.
5. **Jamais de secret.** Un texte qui ressemble à un mot de passe, une clé ou un numéro de
   carte n'est jamais tapé.
6. **Touches refusées.** Ctrl+Alt+Suppr, Ctrl+Alt+Retour arrière, Alt+Syst et Ctrl+Alt+F1…F12
   sont toujours refusées. On peut en ajouter avec `touches_interdites`.
7. **Arrêt.**
   - **Échap** dans la bulle pendant qu'il travaille : plus aucune action pour cette demande, et
     une question en attente reçoit « Non ».
   - Au plus **25 actions** par demande.
8. **Traçabilité.**
   - Chaque action s'affiche dans la bulle au moment où elle est faite.
   - Elle est notée avec la réponse dans le journal des échanges (`/historique`).
   - Le presse-papiers utilisé pour taper est remis comme avant.

## Technique

- Fenêtres : `kdotool` (scripts KWin).
- Clavier : `ydotool`, avec son démon lancé à la demande, **souris coupée** ; il s'arrête avec
  le maki.
- Son : `wpctl` (PipeWire).
- Lecteurs : MPRIS par `qdbus6`.
- Luminosité : le service d'énergie de KDE.
