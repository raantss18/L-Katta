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
- Moteur d'IA intégré : src/moteur.rs (porté de Tontolo, catalogue assets/seed.json) ;
  moteur « integre » en premier, Tontolo optionnel. AppImage : scripts/appimage.sh.
- Tester l'AppImage avec APPIMAGELAUNCHER_DISABLE=1 et un HOME de test : sinon
  AppImageLauncher ouvre une fenêtre sur l'écran de l'utilisateur.
- Ne pas tester en changeant de bureau virtuel ni en injectant des touches : l'utilisateur
  travaille sur cette machine pendant les essais.
- Versions : chaque chiffre 0..9 (0.0.9 → 0.1.0). Feuille de route : docs/TODO.md.
- `lkatta apercu fichier.png` dessine les poses.
- Côtés et fils (src/fils.rs) : côté Discussion (sans outils d'écriture, `brain::OUTILS_CODE`)
  et côté Code (agent, dossier de projet via `paths::set_travail`). Un fichier par fil dans
  ~/.local/share/lkatta/fils/, index léger ; la bulle ne garde que le fil ouvert.
- Arrêt : `controle::nouvelle_demande()` numérote chaque demande, le fil de travail fait
  `controle::adopter(n)` ; `arreter()` n'arrête que la demande en cours (flux, moteur, outils).
- Mémoire : deux tampons Wayland réutilisés par surface (jamais `create_buffer` à chaque image) ;
  garde-fou src/garde.rs (relance au-delà de 500 Mo). `lkatta apercu-fils x.png` (HOME d'essai).
- Temps : `Maki::step` suit le temps réel par pas de 50 ms (jamais un seul pas tronqué, sinon
  la danse ralentit aux cadences lentes) ; la cadence ne descend jamais sous 4 × le coût moyen
  d'une image (`cout_image`), pour tenir sur une machine 8 Go / 2 Go de carte graphique.
- Index des fichiers : src/index.rs (SQLite FTS5 via rusqlite « bundled », fil nice 19 + E/S idle,
  base ~/.local/share/lkatta/index.db). Inventaire : src/inventaire.rs. À faire : src/afaire.rs.
  Agenda et carnet locaux (ce que L Katta crée) : agenda.ics / contacts.vcf dans le dossier de
  données, lus avec les sources configurées. Tout doit marcher sur n'importe quelle distribution :
  chaque outil externe (lspci, pdftotext, xdg-user-dir, kscreen-doctor…) est facultatif.
