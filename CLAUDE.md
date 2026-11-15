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
- Versions : chaque chiffre 0..9 (0.0.9 → 0.1.0), toujours par `tool/version.py --bump`. Feuille de route : docs/TODO.md.
- `lkatta apercu fichier.png` dessine les poses. Dessin du maki : src/mascot/dessin.rs ; pas de
  dégradé tiny-skia (calculés pixel par pixel, ils doublaient le coût) : le volume se fait par
  ovales unis superposés (`volume`). Coût : `cargo test --release cout_dessin -- --ignored --nocapture`.
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
- Publier une version : `tool/publier.sh notes.md` (signe SHA256SUMS avec release-keys/ed25519.pem,
  hors Git ; clé publique dans src/maj.rs). Jamais de version sans SHA256SUMS.sig : les L Katta
  installés la refuseraient. Seulement après un « oui » d'Antsa.
- Essayer la mise à jour : HOME et dossiers XDG de test, `LKATTA_FAUX_VERSION=0.2.0 lkatta maj`
  (avec cette variable, le maki en marche n'est ni arrêté ni relancé).
- Relancer le maki installé : TOUJOURS `tool/relancer-maki.sh <AppImage>` (refuse si le service
  contient autre chose que le maki ; un `paru -Syu` lancé depuis la bulle y a été coupé le
  2026-11-14). Depuis 0.2.3 les terminaux ouverts par la bulle vivent dans leur propre scope.
- Lire une fenêtre : src/atspi.rs (bus d'accessibilité). Essai sans toucher à l'écran ni au réglage
  de session : KWrite dans un KWin virtuel (`kwin_wayland --virtual`, XDG_RUNTIME_DIR court) avec
  QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1, puis `lkatta lire-fenetre`. Ne jamais tuer kwrite/kwin par
  nom (ceux de l'utilisateur) : laisser `timeout` les arrêter.
- Hors KDE : fenêtres par wlr-foreign-toplevel (src/toplevels.rs ; écouteur dans mascot/mod.rs,
  actions déposées par les outils et exécutées au tick), hyprctl et swaymsg en priorité.
  Environnement détecté : src/environnement.rs (environnement.toml).
- X11 (en cours, docs/TODO.md §3) : src/mascot/x11.rs. Essai : un script qui lance
  `lkatta x11-essai --capture x.png --duree 4`, passé SEUL à `kwin_wayland --virtual --xwayland`
  (kwin lance chaque argument après les options comme un programme séparé).
- Affichage : mascot/mod.rs = logique commune + Wayland (`App.wl`, facultatif) ; mascot/x11.rs =
  affichage X11 (`App.x11`). Essai X11 du vrai maki : script passé à `kwin_wayland --virtual
  --xwayland` qui lance lkatta avec LKATTA_AFFICHAGE=x11, XDG_* de test et
  LKATTA_X11_CAPTURE=image.png (dernière image envoyée).
- Essais dans un KWin virtuel : XDG_RUNTIME_DIR COURT (`mktemp -d /tmp/lkx.XXXX` ; un chemin long
  empêche le socket Wayland, kwin reste muet) et lkatta lancé par `dbus-run-session --` : sinon
  l'instance d'essai parle au vrai KWin et au vrai bus de session de l'utilisateur. X11 sans
  clics simulés : fenêtres de test par xterm (`-fullscreen` pour le plein écran), LKATTA_DEBUG=1
  affiche « fenêtre : … ». Clavier X11 : `Clavier::depuis_noms` (libxkbcommon) testable sans X.
- Glisser-déposer X11 : `examples/xdnd_source.rs` joue le gestionnaire de fichiers. Après un
  essai sous `dbus-run-session`, les portails activés (xdg-desktop-portal, xdg-document-portal
  et leur montage FUSE dans XDG_RUNTIME_DIR) survivent : les arrêter par PID après avoir vérifié
  dans /proc/PID/environ qu'ils parlent au bus d'essai, puis vérifier qu'aucun
  target/debug/lkatta ne reste.
- Programmes externes : TOUJOURS `crate::hote::commande(..)` (ou `commande_env`), jamais
  `Command::new` (sauf le binaire lui-même) : dans le Flatpak ils passent par
  `flatpak-spawn --host`. `config::which` regarde sur la machine. Paquets : packaging/aur
  (`tool/aur.sh`), packaging/flatpak. Glibc : `tool/glibc-max.sh` (2.35 max, src/glibc.rs).
  Fichiers de /etc : `hote::etc(..)` (/run/host/etc dans le Flatpak). Publier exige aussi
  `<release version="X.Y.Z">` dans packaging/flatpak/*.metainfo.xml. Essai du Flatpak :
  `dbus-run-session -- kwin_wayland --virtual …` (KWin et le Flatpak sur le bus d'essai),
  `flatpak install --user` du paquet puis `flatpak run --env=HOST_XDG_CONFIG_HOME=…` (dossiers
  d'essai sous ~ : /tmp n'est pas vu) ; désinstaller ensuite. `flatpak build` ne suffit pas
  (ni WAYLAND_DISPLAY ni $XDG_RUNTIME_DIR/app partagé).
- Dépôt Flatpak : `tool/depot-flatpak.sh` signe repo-flatpak (clé GPG du dépôt, publique dans
  packaging/flatpak/lkatta-depot.gpg, privée dans le trousseau d'Antsa) et écrit
  dist/flatpak-depot.tar.gz, joint à la version ; site.yml le dépose sous /flatpak/. Flathub :
  exclu (ils refusent les manifestes et demandes faits avec une IA, et les apps qui dépendent de l'hôte).
- Maki vivant : `bords_visibles` (behavior.rs) calcule les hauts de fenêtres non recouverts ; le
  script KWin (bureau.rs), l'extension GNOME (Version 3) et x11.rs envoient `toits` (fenêtres
  visibles, ordre d'empilement). Briefing : src/briefing.rs. Ménage : src/menage.rs (analyse en
  lecture seule, essais sur une arborescence temporaire). Sauvegardes : src/sauvegarde.rs
  (`lancer` n'écrit pas l'état : les essais ne touchent pas aux vraies données).
- Autonomie du côté Code : src/autonomie.rs (un accord par dossier de projet, en mémoire ; fichiers
  libres dans le dossier ; commandes libres dans le bac bubblewrap sans réseau ; `gardee()` =
  toujours demandé). Règle `autonomie` dans regles.toml. Ne jamais élargir l'accord au dossier
  personnel ni laisser une commande libre hors du bac.
- GNOME : extension pont dans assets/gnome/<uuid>/ (embarquée par include_str!, src/gnome.rs),
  ES modules GNOME 45+. Elle envoie la fenêtre active à org.lkatta.Maki (comme le script KWin)
  et offre /org/lkatta/Gnome chez org.gnome.Shell. Toute chaîne nouvelle passe par t!() ET
  src/lang/en_*.rs (test every_text_has_an_english_translation). Sous GNOME, « Ne pas
  déranger », raccourci global (raccourci personnalisé → `lkatta raccourci`) et échelle
  (champs sw/sh de l'extension, `facteur_scene`) passent par src/gnome.rs et on_fenetre.
