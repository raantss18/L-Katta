# L Katta sur Android — réflexion (2026-11-09)

À relire et trancher avant d'écrire du code. Rien n'est commencé.

## Ce que L Katta est sur le bureau, et ce qui ne passe pas sur un téléphone

| Sur le bureau KDE | Sur Android |
|---|---|
| Maki qui se promène par-dessus toutes les fenêtres (layer-shell) | Seulement avec la permission « afficher par-dessus les autres applis » (bulles façon *chat heads*). Réglage manuel, mal vu du Play Store, gourmand en batterie. |
| Bulle sans fenêtre, clavier prêté à la demande | Possible dans l'appli ou dans une bulle flottante ; pas de « clic ailleurs » à surveiller. |
| Outils machine : fichiers, commandes, Konsole, santé, nettoyage, sudo | **Ne s'appliquent pas** (bac à sable Android). |
| KWin, bureaux virtuels, écrans multiples, discrétion en visio | Ne s'appliquent pas ; « Ne pas déranger » existe et se lit. |
| Lire l'écran (Spectacle) | MediaProjection : accord demandé **à chaque capture** (bandeau système). Faisable, mais lourd. |
| Notifications (dbus-monitor) | `NotificationListenerService` : **bien adapté**, accord donné une fois dans les réglages. |
| Presse-papiers surveillé | **Interdit en arrière-plan** depuis Android 10. On le remplace par le **menu Partager** (« Partager vers L Katta » → traduire, résumer, corriger, expliquer) et par le menu de sélection de texte (`PROCESS_TEXT`). |
| Moteur d'IA intégré, modèles gratuits, clés dans le trousseau | **Passe tel quel** (Rust) ; clés dans l'Android Keystore. Modèle local : seulement très petit (qwen3 0.6b–1.7b), le téléphone chauffe. |
| Rappels, mails, agenda | Rappels : oui (notifications programmées). Mails/agenda : Android les a déjà ; lire l'agenda du téléphone (permission Calendrier) plutôt que l'iCal. |

**Conclusion** : une copie du maki de bureau n'a pas de sens. Ce qui en a : un **compagnon de poche**
qui garde la personnalité (le lémurien, la bulle de papier, le moteur gratuit, la mémoire) et qui
vit là où Android laisse de la place : l'appli elle-même, le menu Partager, la sélection de texte,
les notifications, un widget, une tuile des réglages rapides.

## Pistes techniques

1. **Rust egui comme Fanoratana (recommandé)**. Même montage déjà éprouvé chez Antsa : coquille
   Gradle/Kotlin minimale + bibliothèque Rust compilée avec cargo-ndk. Le dessin du maki
   (`mascot/dessin.rs`, tiny-skia) et la bulle « carnet » se réutilisent en texture egui.
   Les points d'entrée Android (Partager, `PROCESS_TEXT`, écoute des notifications, tuile) sont de
   petites classes Kotlin qui appellent Rust par JNI.
2. Flutter + flutter_rust_bridge : interface Android plus « native », mais deux langages d'interface
   à maintenir et un maki à redessiner. Non retenu.
3. Kotlin pur : tout réécrire, y compris le moteur. Non retenu.

## Préalable utile au bureau aussi : un cœur partagé

Sortir dans un crate `lkatta-core` ce qui ne dépend ni de Wayland ni de KDE : moteur (`moteur.rs`,
`flux.rs`), cerveau sans outils machine, `markdown.rs`, `recherche.rs`, `memoire.rs`,
`reminders.rs` (analyse des dates), `lang/`, dessin du maki. Le binaire de bureau ne change pas de
comportement ; Android n'aurait plus qu'une coquille et ses outils propres.

## Étapes proposées (si Antsa dit oui)

0. `lkatta-core` (bureau inchangé, tests verts).
1. **Compagnon 0.1** : appli egui avec la bulle et le maki animé, moteur gratuit, mémoire,
   recherche approfondie ; « Partager vers L Katta » et sélection de texte → 4 actions.
2. Notifications : écoute avec accord, `/notifs` résumé, résumé au retour d'un « Ne pas déranger ».
3. Widget d'accueil (le maki qui dort / salue) et tuile « Demander à L Katta ».
4. Facultatif, plus tard : bulle flottante par-dessus les applis ; lecture d'écran par MediaProjection.

Coût estimé : étape 0 ≈ une session ; étape 1 ≈ deux à trois sessions (en reprenant la coquille de
Fanoratana) ; 2 et 3 ≈ une session chacune.

## Questions à trancher

- Appareil d'essai : la tablette Samsung R8YYA0CZ5ZV, un téléphone, ou l'émulateur ?
- Distribution : APK sur GitHub (comme Fanoratana) ou aussi le Play Store (règles plus strictes sur
  les permissions d'accessibilité et de superposition) ?
- Faut-il partager la mémoire et les réglages entre le bureau et le téléphone (synchronisation
  chiffrée), ou deux compagnons indépendants ?
