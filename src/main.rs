//! L Katta (Lemur Kata) — un petit maki assistant de bureau pour Linux/KDE.

#[macro_use]
mod lang;

mod apropos;
mod brain;
mod bureau;
mod chat;
mod cles;
mod clipboard;
mod config;
mod conseils;
mod daemon;
mod flux;
mod fr;
mod install;
mod ipc;
mod kde;
mod mail;
mod maj;
mod markdown;
mod mascot;
mod agenda;
mod apps;
mod memoire;
mod moteur;
mod saisie;
mod sante;
mod paths;
mod profile;
mod regles;
mod recherche;
mod semantique;
mod souvenirs;
mod reminders;
mod local;
mod ecran;
mod notifs;
mod terminal;
mod tools;
mod web;

/// Aide (texte source français ; traduite par `t!` à l'affichage).
fn aide() -> &'static str {
    t!("L Katta — le petit maki assistant de ton ordinateur

Utilisation : lkatta [commande]

  (aucune) | demarrer     lance le maki à l'écran (démon)
  discuter                ouvre ou ferme la bulle de discussion (aussi : panneau)
  demander <question>     pose une question dans le terminal
  chercher <question>     recherche approfondie : plusieurs sources lues et citées
  agent <tâche>           mode agent : plan, actions jusqu'au bout, compte rendu
  regarder [question]     lit l'écran (capture et modèle de vision)
  bilan                   bilan de la semaine (place, cache, mises à jour, problèmes)
  dire <texte>            fait parler le maki
  discret [on|off]        moins de mouvements, pas de remarques spontanées
  cacher [on|off]         masque ou montre le maki
  acrobatie | salut       salto ou salut
  arbre                   fait pousser un arbre et y grimpe
  teleporter              disparaît dans un nuage et réapparaît ailleurs
  grimper                 bondit jusqu'au bord de l'écran et l'escalade
  ecran                   part sur un autre écran
  quitter                 arrête le maki
  rappel <quand> <texte>  ex. lkatta rappel 18h30 appeler Hery
  rappels                 liste les rappels
  reglages                montre les réglages (moteurs, modèles, zone…)
  demarrage on|off        lancer le maki à l'ouverture de session (ou non)
  maj                     cherche et installe la dernière version
  compte-mail <adresse>   ajoute un compte mail (mot de passe rangé dans le trousseau)
  catalogue               rafraîchit les modèles gratuits du moteur intégré
  especes                 les lémuriens au choix (lkatta regle espece indri)
  modeles [n] [filtre]    modèles proposés par le moteur n (défaut 1)
  regle <réglage> <val>   ex. regle zone libre · regle moteur 2 modele <nom>
  etat                    état de la machine
  actualites              dernières nouvelles des flux suivis
  profil                  remesure la machine et affiche le prompt système
  installer               installe dans ~/.local/bin et au démarrage de session
  desinstaller            retire le démarrage automatique et le raccourci
  apercu <fichier.png>    dessine les poses du maki dans une image
  apropos                 version, licence et crédits
  version

Souris : clic = bulle de discussion, glisser = l'attraper et le lancer (près d'un
bord, il s'y agrippe ; en haut, il s'y suspend), clic droit = discret,
clic molette = un numéro au hasard, clic sur une bulle = la fermer.
")
}

/// Noms anglais des commandes → nom français (celui que comprend aussi un démon plus ancien).
fn canon(cmd: &str) -> &str {
    match cmd {
        "start" => "demarrer",
        "chat" | "panel" => "discuter",
        "ask" => "demander",
        "research" | "search" => "chercher",
        "say" => "dire",
        "quiet" => "discret",
        "hide" => "cacher",
        "flip" => "acrobatie",
        "wave" => "salut",
        "tree" => "arbre",
        "teleport" => "teleporter",
        "climb" => "grimper",
        "screen" => "ecran",
        "quit" => "quitter",
        "reminder" | "remind" => "rappel",
        "reminders" => "rappels",
        "settings" => "reglages",
        "set" => "regle",
        "autostart" => "demarrage",
        "update" => "maj",
        "mail-account" => "compte-mail",
        "catalog" => "catalogue",
        "species" => "especes",
        "models" => "modeles",
        "status" => "etat",
        "news" => "actualites",
        "profile" => "profil",
        "install" => "installer",
        "uninstall" => "desinstaller",
        "preview" => "apercu",
        "about" | "a-propos" => "apropos",
        "help" | "--help" | "-h" => "aide",
        "--version" | "-V" => "version",
        other => other,
    }
}

/// Traduit les lignes d'un texte qui figurent telles quelles dans les tables (titres de
/// `etat`, etc.) ; les autres restent intactes. Pour l'affichage seulement.
fn traduire_lignes(s: &str) -> String {
    if !lang::en() {
        return s.to_string();
    }
    // « n°3 — … » (rappels) : numéro à l'anglaise.
    s.lines().map(|l| l.strip_prefix("n°").map(|r| format!("#{r}")).unwrap_or_else(|| lang::trd(l).to_string())).collect::<Vec<_>>().join("\n")
}

fn main() {
    // Langue d'abord (sans créer de config.toml pour une simple commande d'aide).
    lang::regler("auto");
    if paths::config_file().exists() {
        config::Config::load();
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = canon(args.first().map(String::as_str).unwrap_or("demarrer"));
    let rest = args.get(1..).map(|r| r.join(" ")).unwrap_or_default();
    let r: Result<(), String> = match cmd {
        "demarrer" => daemon::run(),
        "demander" if !rest.is_empty() => chat::ask_cli(&rest, brain::Mode::Discussion),
        "chercher" if !rest.is_empty() => chat::ask_cli(&rest, brain::Mode::Recherche),
        "agent" if !rest.is_empty() => chat::ask_cli(&rest, brain::Mode::Agent),
        "regarder" | "look" => crate::ecran::regarder(&rest).map(|t| println!("{t}")),
        "bilan" | "weekly" => {
            println!("{}", crate::sante::bilan_semaine());
            Ok(())
        }
        "dire" | "discret" | "cacher" | "acrobatie" | "salut" | "quitter" | "remarque" | "arbre" | "teleporter" | "grimper" | "discuter" | "panneau" | "ecran" => {
            ipc::send(&format!("{cmd} {rest}")).map(|r| println!("{}", if r == "ok" { r } else { lang::trd(&r).to_string() })).map_err(|_| t!("le maki ne tourne pas (lancer : lkatta)").into())
        }
        "modeles" => {
            let cfg = config::Config::load();
            let n: usize = args.get(1).and_then(|x| x.parse().ok()).unwrap_or(1);
            let filtre = args.iter().skip(if args.get(1).is_some_and(|x| x.parse::<usize>().is_ok()) { 2 } else { 1 }).cloned().collect::<Vec<_>>().join(" ").to_lowercase();
            match cfg.moteur.get(n.saturating_sub(1)) {
                None => Err(t!("pas de moteur {}", n)),
                Some(m) => brain::list_models(m).map(|l| {
                    let hits: Vec<_> = l.iter().filter(|x| filtre.is_empty() || x.to_lowercase().contains(&filtre)).collect();
                    println!("{}", t!("{} : {} modèle(s)", m.nom, hits.len()));
                    for h in hits {
                        println!("  {h}");
                    }
                    println!("{}", t!("Choisir : lkatta regle moteur {} modele <nom>", n));
                }),
            }
        }
        "demarrage" => {
            let on = !matches!(args.get(1).map(String::as_str), Some("off" | "non" | "no" | "0"));
            let mut c = config::Config::load();
            c.demarrage_auto = on;
            c.save().map_err(|e| e.to_string()).and_then(|_| install::sync_autostart(on)).map(|m| println!("{m}"))
        }
        "maj" => update_now(),
        "compte-mail" => add_mail_account(args.get(1).map(String::as_str).unwrap_or("")),
        "catalogue" => {
            match moteur::refresh() {
                Ok(n) => println!("{}", t!("Catalogue rafraîchi : {} modèles.", n)),
                Err(e) => println!("{}", t!("Rafraîchissement impossible ({}) : catalogue de secours.", e)),
            }
            let c = moteur::catalogue();
            for f in &c.fournisseurs {
                let n = c.modeles.iter().filter(|m| m.provider == f.id).count();
                let t = c.modeles.iter().filter(|m| m.provider == f.id && m.tools).count();
                println!("{}", t!("  {} : {} modèles gratuits ({} avec outils)", f.nom, n, t));
            }
            Ok(())
        }
        "especes" => {
            println!("{}", mascot::especes::liste());
            println!("\n{}", t!("Choisir : lkatta regle espece sifaka"));
            Ok(())
        }
        "reglages" => {
            println!("{}", config::Config::load().summary());
            Ok(())
        }
        "regle" => {
            let toks = &args[1..];
            let k = if toks.first().map(String::as_str) == Some("moteur") { 3 } else { 1 };
            if toks.len() <= k {
                Err(t!("usage : lkatta regle <réglage> <valeur>   (ex. regle zone libre, regle moteur 2 modele <nom>)").into())
            } else {
                let mut c = config::Config::load();
                c.set_path(&toks[..k].join("."), &toks[k..].join(" ")).map(|m| {
                    println!("{}", t!("Réglé : {}", m));
                    let _ = ipc::send("recharger");
                })
            }
        }
        "rappel" => add_reminder(&args[1..]),
        "rappels" => {
            println!("{}", traduire_lignes(&tools::list_reminders()));
            Ok(())
        }
        "etat" => {
            println!("{}", traduire_lignes(&tools::system_status()));
            Ok(())
        }
        "actualites" => {
            println!("{}", tools::news("", &config::Config::load()));
            Ok(())
        }
        "profil" => profile::ensure(true).map_err(|e| e.to_string()).map(|_| {
            println!("{}", profile::system_prompt(&config::Config::load()));
            eprintln!("\n{}", t!("(fichiers : {} et {})", paths::prompt_file().display(), paths::machine_file().display()));
        }),
        "installer" => install::install(),
        "desinstaller" => install::uninstall(),
        "site-images" => install::site_images(args.get(1).map(String::as_str).unwrap_or("site/img/lemuriens")),
        "icone" => {
            let out = args.get(1).map(String::as_str).unwrap_or("lkatta.png");
            install::icon(256).save_png(out).map_err(|e| e.to_string()).map(|_| println!("{out}"))
        }
        "apropos" => {
            println!("{}", apropos::texte());
            Ok(())
        }
        "apercu-discussion" => install::preview_chat(args.get(1).map(String::as_str).unwrap_or("lkatta-discussion.png")),
        "apercu-reglages" => install::preview_settings(args.get(1).map(String::as_str).unwrap_or("lkatta-reglages.png")),
        "mesure-bulle" => install::mesure_bulle(),
        "apercu-accueil" => install::preview_welcome(args.get(1).map(String::as_str).unwrap_or("lkatta-accueil.png")),
        "apercu-modeles" => install::preview_models(args.get(1).map(String::as_str).unwrap_or("lkatta-modeles.png")),
        "apercu-bulles" => install::preview_bubbles(args.get(1).map(String::as_str).unwrap_or("lkatta-bulles.png")),
        "apercu" => install::preview(args.get(1).map(String::as_str).unwrap_or("lkatta-apercu.png")),
        "version" => {
            println!("lkatta {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "aide" => {
            print!("{}", aide());
            Ok(())
        }
        other => Err(t!("commande inconnue : {}\n\n{}", other, aide())),
    };
    if let Err(e) = r {
        eprintln!("lkatta : {e}");
        std::process::exit(1);
    }
}

fn add_reminder(args: &[String]) -> Result<(), String> {
    // Le moment peut tenir en deux mots (« demain 9h », « dans 10m »).
    let now = chrono::Local::now();
    // `parse_when` comprend aussi l'anglais (« tomorrow at 9am », « in 10 min »).
    for n in [4usize, 3, 2, 1] {
        if args.len() > n {
            let when = args[..n].join(" ");
            if let Some(d) = reminders::parse_when(&when, now) {
                let r = reminders::add(d, &args[n..].join(" ")).map_err(|e| e.to_string())?;
                println!("{}", t!("Rappel n°{} : {} — {}", r.id, fr::date_longue(&d), r.texte));
                return Ok(());
            }
        }
    }
    Err(t!("usage : lkatta rappel <quand> <texte>  (quand : 10m, 2h, 18h30, demain 9h, 2026-11-09 14:00)").into())
}

fn add_mail_account(adresse: &str) -> Result<(), String> {
    if !adresse.contains('@') {
        return Err(t!("usage : lkatta compte-mail adresse@exemple.com").into());
    }
    let mut cfg = config::Config::load();
    if !cfg.mail.iter().any(|c| c.adresse == adresse) {
        cfg.mail.push(mail::Compte::guess(adresse));
        cfg.save().map_err(|e| e.to_string())?;
    }
    let c = cfg.mail.iter().find(|c| c.adresse == adresse).unwrap();
    println!("{}", t!("Compte {} : {} / {}", c.nom, c.imap, c.smtp));
    if adresse.ends_with("@gmail.com") {
        println!("{}", t!("Gmail : utilise un « mot de passe d'application » (myaccount.google.com › Sécurité › Mots de passe des applications)."));
    }
    println!("{}", t!("Tape le mot de passe (il va dans le trousseau, jamais dans un fichier) :"));
    mail::store_secret(adresse)?;
    match mail::list(c, true, 3) {
        Ok(l) => println!("{}", t!("Connexion réussie : {} mail(s) non lu(s) récent(s).", l.len())),
        Err(e) => println!("{}", t!("Enregistré, mais la connexion a échoué : {}", e)),
    }
    Ok(())
}

fn update_now() -> Result<(), String> {
    println!("{}", t!("Version installée : {}", maj::current()));
    let Some(rel) = maj::check()? else {
        println!("{}", t!("C'est déjà la dernière version."));
        return Ok(());
    };
    println!("{}", t!("Nouvelle version {} : téléchargement et vérification…", rel.version));
    let v = maj::install(&rel)?;
    println!("{}", t!("Version {} installée.", v));
    if ipc::daemon_running() {
        let _ = ipc::send(&format!("dire {}", t!("Mise à jour {} installée, je reviens tout de suite !", v)));
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = ipc::send("quitter");
        maj::restart();
        println!("{}", t!("Le maki redémarre avec la nouvelle version."));
    }
    Ok(())
}

