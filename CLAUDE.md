# L Katta — notes pour les agents

- Rust 2024, binaire unique `lkatta` (src/main.rs). `cargo test` doit rester vert.
- Surcouche Wayland dans src/mascot (mod.rs : surfaces layer-shell, une sonde par écran ;
  behavior.rs : physique et comportement, testable ; draw.rs : dessin tiny-skia).
- Pas de fenêtre : la discussion est une bulle (src/chat.rs). Ne JAMAIS passer la surface en
  `KeyboardInteractivity::Exclusive` : la bulle volerait le clavier de l'utilisateur.
- Cerveau (src/brain.rs) : API compatible OpenAI, moteur Tontolo d'abord (~/GitHub/TONTOLO).
- Outils dans src/tools.rs : lecture seule directe, sinon `Host::confirm`. Jamais de sudo.
  Outils portés de mi-saina : src/sante.rs, src/apps.rs, src/memoire.rs. KDE : src/kde.rs.
- Discrétion avant tout (docs/ANALYSE.md §3) : il reste sur les bords, mouvements sobres,
  il s'arrête et s'assoit dès qu'une bulle s'affiche (`Maki::hush`). Cible : machine 8 Go.
- Espèces : src/mascot/especes.rs (palette, morphologie, démarche, numéro) ; le dessin lit
  l'espèce via `draw::set_species`. Bulles typées : src/conseils.rs (`Kind`, `Message`).
- Règles strictes : src/regles.rs (machine /etc/lkatta + utilisateur), vérifiées dans
  `tools::execute`. Konsole : src/terminal.rs. Mails : src/mail.rs (curl, trousseau).
  Agenda/contacts : src/agenda.rs. Mise à jour : src/maj.rs (releases GitHub, SHA-256).
- Ne pas tester en changeant de bureau virtuel ni en injectant des touches : l'utilisateur
  travaille sur cette machine pendant les essais.
- Versions : chaque chiffre 0..9 (0.0.9 → 0.1.0). Feuille de route : docs/TODO.md.
- `lkatta apercu fichier.png` dessine les poses.
