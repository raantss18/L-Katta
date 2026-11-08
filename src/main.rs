//! L Katta (Lemur Kata) — un petit maki assistant de bureau pour Linux/KDE.

mod brain;
mod chat;
mod clipboard;
mod config;
mod daemon;
mod fr;
mod install;
mod ipc;
mod kde;
mod mascot;
mod apps;
mod memoire;
mod sante;
mod paths;
mod profile;
mod reminders;
mod tools;
mod web;

const AIDE: &str = "\
L Katta — le petit maki assistant de ton ordinateur

Utilisation : lkatta [commande]

  (aucune) | demarrer     lance le maki à l'écran (démon)
  discuter                ouvre ou ferme la bulle de discussion (aussi : panneau)
  demander <question>     pose une question dans le terminal
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
  modeles [n] [filtre]    modèles proposés par le moteur n (défaut 1)
  regle <réglage> <val>   ex. regle zone libre · regle moteur 2 modele <nom>
  etat                    état de la machine
  actualites              dernières nouvelles des flux suivis
  profil                  remesure la machine et affiche le prompt système
  installer               installe dans ~/.local/bin et au démarrage de session
  desinstaller            retire le démarrage automatique et le raccourci
  apercu <fichier.png>    dessine les poses du maki dans une image
  version

Souris : clic = bulle de discussion, glisser = l'attraper et le lancer (près d'un
bord, il s'y agrippe ; en haut, il s'y suspend), clic droit = discret,
clic molette = un numéro au hasard, clic sur une bulle = la fermer.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("demarrer");
    let rest = args.get(1..).map(|r| r.join(" ")).unwrap_or_default();
    let r: Result<(), String> = match cmd {
        "demarrer" | "start" => daemon::run(),
        
        "demander" | "ask" if !rest.is_empty() => chat::ask_cli(&rest),
        "dire" | "discret" | "cacher" | "acrobatie" | "salut" | "quitter" | "remarque" | "arbre" | "teleporter" | "grimper" | "discuter" | "panneau" | "ecran" => {
            ipc::send(&format!("{cmd} {rest}")).map(|r| println!("{r}")).map_err(|_| "le maki ne tourne pas (lancer : lkatta)".into())
        }
        "modeles" => {
            let cfg = config::Config::load();
            let n: usize = args.get(1).and_then(|x| x.parse().ok()).unwrap_or(1);
            let filtre = args.iter().skip(if args.get(1).is_some_and(|x| x.parse::<usize>().is_ok()) { 2 } else { 1 }).cloned().collect::<Vec<_>>().join(" ").to_lowercase();
            match cfg.moteur.get(n.saturating_sub(1)) {
                None => Err(format!("pas de moteur {n}")),
                Some(m) => brain::list_models(m).map(|l| {
                    let hits: Vec<_> = l.iter().filter(|x| filtre.is_empty() || x.to_lowercase().contains(&filtre)).collect();
                    println!("{} : {} modèle(s)", m.nom, hits.len());
                    for h in hits {
                        println!("  {h}");
                    }
                    println!("Choisir : lkatta regle moteur {n} modele <nom>");
                }),
            }
        }
        "reglages" => {
            println!("{}", config::Config::load().summary());
            Ok(())
        }
        "regle" => {
            let toks = &args[1..];
            let k = if toks.first().map(String::as_str) == Some("moteur") { 3 } else { 1 };
            if toks.len() <= k {
                Err("usage : lkatta regle <réglage> <valeur>   (ex. regle zone libre, regle moteur 2 modele <nom>)".into())
            } else {
                let mut c = config::Config::load();
                c.set_path(&toks[..k].join("."), &toks[k..].join(" ")).map(|m| {
                    println!("Réglé : {m}");
                    let _ = ipc::send("recharger");
                })
            }
        }
        "rappel" => add_reminder(&args[1..]),
        "rappels" => {
            println!("{}", tools::list_reminders());
            Ok(())
        }
        "etat" => {
            println!("{}", tools::system_status());
            Ok(())
        }
        "actualites" => {
            println!("{}", tools::news("", &config::Config::load()));
            Ok(())
        }
        "profil" => profile::ensure(true).map_err(|e| e.to_string()).map(|_| {
            println!("{}", profile::system_prompt(&config::Config::load()));
            eprintln!("\n(fichiers : {} et {})", paths::prompt_file().display(), paths::machine_file().display());
        }),
        "installer" => install::install(),
        "desinstaller" => install::uninstall(),
        "apercu" => install::preview(args.get(1).map(String::as_str).unwrap_or("lkatta-apercu.png")),
        "version" | "--version" | "-V" => {
            println!("lkatta {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "aide" | "help" | "--help" | "-h" => {
            print!("{AIDE}");
            Ok(())
        }
        other => Err(format!("commande inconnue : {other}\n\n{AIDE}")),
    };
    if let Err(e) = r {
        eprintln!("lkatta : {e}");
        std::process::exit(1);
    }
}

fn add_reminder(args: &[String]) -> Result<(), String> {
    // Le moment peut tenir en deux mots (« demain 9h », « dans 10m »).
    let now = chrono::Local::now();
    for n in [2usize, 1] {
        if args.len() > n {
            let when = args[..n].join(" ");
            if let Some(d) = reminders::parse_when(&when, now) {
                let r = reminders::add(d, &args[n..].join(" ")).map_err(|e| e.to_string())?;
                println!("Rappel n°{} : {} — {}", r.id, fr::date_longue(&d), r.texte);
                return Ok(());
            }
        }
    }
    Err("usage : lkatta rappel <quand> <texte>  (quand : 10m, 2h, 18h30, demain 9h, 2026-11-09 14:00)".into())
}
