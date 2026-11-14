//! Film de présentation : des scènes scriptées jouées dans la vraie bulle (le même code de
//! dessin que l'application), image par image, envoyées à `ffmpeg`. Rien n'est capturé à
//! l'écran et aucune donnée de l'utilisateur n'est lue : tout se passe dans un dossier d'essai.
//!
//! `lkatta apercu-film sortie.mp4` (ou `image.png 42.5` pour une seule image à 42,5 s).

use std::io::Write;
use std::time::{Duration, Instant};

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::chat::{Chat, Entry, HistoAct};
use crate::conseils::{Kind, Message};
use crate::fils::{self, Cote};
use crate::mascot::behavior::{Attitude, Bubble, Ctx, Exits, Maki, Mode};
use crate::mascot::draw::{self, ChatLayout, R, Text};

const W: u32 = 1280;
const H: u32 = 720;
const IPS: f32 = 20.0;
/// Échelle du texte des bulles.
const U: f32 = 1.12;
/// Sol (haut de la barre du bureau) et place du maki.
const SOL: f32 = 664.0;
const MAKI_X: f32 = 1150.0;
/// Le fil qu'on rouvre depuis l'historique.
const FIL_ROUVERT: &str = "Lettre au propriétaire";

/// Où le pointeur va cliquer (la zone vient de la bulle dessinée à l'image précédente).
#[derive(Clone, Copy, PartialEq)]
enum Cible {
    Maki,
    Saisie,
    Cote(Cote),
    Histo,
    Roue,
    Arreter,
    Oui,
    Fermer,
    /// n-ième zone cliquable de l'historique.
    Fil(usize),
}

struct Film {
    chat: Chat,
    maki: Maki,
    ouverte: bool,
    bulle: Option<Bubble>,
    lay: Option<ChatLayout>,
    souris: (f32, f32),
    vers: (f32, f32),
    clic: f32,
    chapitre: (String, String),
    /// Depuis quand il « réfléchit » (temps du film).
    reflechit: Option<f32>,
    titre_plein: f32,
    /// Ce que montre la fenêtre du bureau : 0 le cours, 1 le budget retrouvé, 2 le projet
    /// (test en échec), 3 le projet corrigé.
    fenetre: u8,
    /// Sons à poser sur la bande (instant, genre) : clic, envoi, réponse, succès.
    sons: Vec<(f32, &'static str)>,
    t: f32,
}

type Action = Box<dyn FnMut(&mut Film)>;

/// Le scénario : une liste d'actions datées, écrite dans l'ordre du temps.
struct Scenario {
    t: f32,
    actions: Vec<(f32, Action)>,
}

impl Scenario {
    fn a(&mut self, f: impl FnMut(&mut Film) + 'static) -> &mut Self {
        self.actions.push((self.t, Box::new(f)));
        self
    }
    fn attendre(&mut self, s: f32) -> &mut Self {
        self.t += s;
        self
    }
    fn chapitre(&mut self, titre: &str, ligne: &str) -> &mut Self {
        let (a, b) = (titre.to_string(), ligne.to_string());
        self.a(move |f| f.chapitre = (a.clone(), b.clone()))
    }
    fn ligne(&mut self, ligne: &str) -> &mut Self {
        let b = ligne.to_string();
        self.a(move |f| f.chapitre.1 = b.clone())
    }
    /// Le pointeur va sur la cible, puis clique (0,7 s en tout).
    fn cliquer(&mut self, c: Cible, mut effet: impl FnMut(&mut Film) + 'static) -> &mut Self {
        self.a(move |f| f.viser(c));
        self.attendre(0.6);
        self.a(move |f| {
            f.clic = 1.0;
            f.sons.push((f.t, "clic"));
            effet(f);
        });
        self.attendre(0.35)
    }
    /// Tape un texte dans la saisie, lettre par lettre.
    fn taper(&mut self, texte: &str) -> &mut Self {
        let lettres: Vec<char> = texte.chars().collect();
        let pas = (1.6 / lettres.len().max(1) as f32).min(0.05);
        for i in 1..=lettres.len() {
            let s: String = lettres[..i].iter().collect();
            self.a(move |f| {
                f.chat.input.set(s.clone());
                if i % 2 == 0 {
                    f.sons.push((f.t, "touche"));
                }
            });
            self.attendre(pas);
        }
        self.attendre(0.35)
    }
    /// Entrée : la question part, il réfléchit.
    fn envoyer(&mut self) -> &mut Self {
        self.a(|f| {
            let q = f.chat.input.prendre();
            f.chat.entries.push(Entry::User(q));
            f.sons.push((f.t, "envoi"));
            f.chat.busy = true;
            f.reflechit = Some(f.t);
        });
        self.attendre(0.7)
    }
    /// Une ligne d'étape (« › je cherche… »).
    fn etape(&mut self, l: &str) -> &mut Self {
        let l = l.to_string();
        self.a(move |f| f.chat.entries.push(Entry::Info(l.clone())));
        self.attendre(0.75)
    }
    /// La réponse s'écrit mot à mot, puis se pose dans la discussion.
    fn repondre(&mut self, texte: &str, duree: f32) -> &mut Self {
        let mots: Vec<&str> = texte.split_inclusive(' ').collect();
        let pas = duree / mots.len().max(1) as f32;
        let mut acc = String::new();
        for m in mots {
            acc.push_str(m);
            let s = acc.clone();
            self.a(move |f| {
                f.chat.flux = s.clone();
                f.maki.set_attitude(Attitude::Parle, 0.5);
            });
            self.attendre(pas);
        }
        let t = texte.to_string();
        self.a(move |f| {
            f.chat.flux.clear();
            f.chat.entries.push(Entry::Maki(t.clone()));
            f.sons.push((f.t, "reponse"));
            f.chat.busy = false;
            f.reflechit = None;
        });
        self.attendre(0.4)
    }
}

impl Film {
    fn centre(r: R) -> (f32, f32) {
        (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0)
    }

    fn viser(&mut self, c: Cible) {
        let l = self.lay.clone().unwrap_or_default();
        let p = match c {
            Cible::Maki => Some((MAKI_X, SOL - 60.0)),
            Cible::Saisie => Some(Self::centre(l.input)),
            Cible::Cote(k) => l.cotes.iter().find(|(_, x)| *x == k).map(|(r, _)| Self::centre(*r)),
            Cible::Histo => Some(Self::centre(l.histo_btn)),
            Cible::Roue => Some(Self::centre(l.gear)),
            Cible::Arreter => l.stop.map(Self::centre),
            Cible::Oui => l.yes.map(Self::centre),
            Cible::Fermer => Some(Self::centre(l.close)),
            Cible::Fil(_) => {
                let id = fils::liste(Cote::Discussion).into_iter().find(|r| r.titre == FIL_ROUVERT).map(|r| r.id);
                l.histo.iter().find(|(_, a)| matches!(a, HistoAct::Ouvrir(x) if Some(x) == id.as_ref())).map(|(r, _)| Self::centre(*r))
            }
        };
        if let Some(p) = p {
            self.vers = p;
        }
    }
}

fn scenario() -> Scenario {
    let mut s = Scenario { t: 0.0, actions: vec![] };

    // --- 0. Ouverture -------------------------------------------------------------------
    s.a(|f| f.titre_plein = 1.0).attendre(3.2);
    // L'idée : la nostalgie des assistants de bureau d'autrefois.
    s.a(|f| f.titre_plein = 2.0).attendre(6.0);
    s.a(|f| f.titre_plein = 0.0);
    s.chapitre("Au quotidien", "Il vit au bord de l'écran, sans jamais cacher ton travail.");
    s.a(|f| {
        f.maki.wave();
        let m = Message::new(Kind::Hello, "Bonjour Antsa !", "Samedi 14 novembre — 08:30\n· 10h Réunion du labo\n· ☐ Rendre le TP d'analyse");
        f.bulle = Some(Bubble { msg: m, ttl: 8.0, age: 0.0 });
    });
    s.attendre(4.6);
    s.ligne("Le matin : tes rendez-vous et ce qu'il reste à faire.").attendre(2.4);

    // --- 1. Discussion ------------------------------------------------------------------
    s.ligne("Un clic sur lui ouvre la bulle : tout se passe ici, pas dans un terminal.");
    s.cliquer(Cible::Maki, |f| {
        f.bulle = None;
        f.ouverte = true;
    });
    s.attendre(1.0);
    s.cliquer(Cible::Saisie, |_| {});
    s.taper("Où est le document où je parlais du budget 2026 ?").envoyer();
    s.ligne("Il retrouve tes fichiers par leur contenu : l'index ne quitte jamais ta machine.");
    s.etape("› je cherche « budget 2026 » dans tes fichiers");
    s.repondre("Je l'ai : **~/Documents/Association/budget-2026.md**, modifié mardi. Tu y prévois 1,2 M Ar pour le matériel. Je te l'ouvre ?", 3.0);
    s.attendre(0.8);
    s.taper("Oui, ouvre-le").envoyer();
    s.etape("› j'ouvre ~/Documents/Association/budget-2026.md");
    s.a(|f| {
        f.fenetre = 1;
        f.sons.push((f.t, "ouvre"));
    });
    s.repondre("Le voilà, dans ton éditeur.", 0.8);
    s.attendre(1.4);

    s.ligne("Rappels, liste à faire, agenda : il s'en occupe en une phrase.");
    s.taper("Rappelle-moi chaque lundi à 8h de préparer le cours").envoyer();
    s.etape("› rappel ajouter");
    s.repondre("C'est noté : **chaque semaine**, à partir de lundi 8 h. Je te préviendrai par une bulle.", 2.0);
    s.attendre(1.2);

    s.ligne("Mails : il prépare la réponse et la range en brouillon. Rien ne part sans toi.");
    s.taper("Réponds à Hery que je serai à la réunion, en brouillon").envoyer();
    s.etape("› je lis tes mails : « Réunion du labo » de Hery").etape("› je prépare la réponse");
    s.repondre("Brouillon « Re: Réunion du labo » rangé dans **Brouillons** : à relire et envoyer depuis ta messagerie.", 2.2);
    s.attendre(1.6);

    // --- 2. Arrêter ---------------------------------------------------------------------
    s.chapitre("Tu gardes la main", "Trop long, ou mal demandé ? Le bouton Arrêter coupe vraiment.");
    s.taper("Résume en détail le budget que tu viens d'ouvrir").envoyer();
    s.etape("› je lis ~/Documents/Association/budget-2026.md");
    s.a(|f| f.chat.flux = "Le budget 2026 de l'association se répartit en quatre postes. Le premier,".into());
    s.attendre(2.4);
    s.cliquer(Cible::Arreter, |f| {
        f.chat.busy = false;
        f.reflechit = None;
        f.chat.entries.push(Entry::Maki("Le budget 2026 de l'association se répartit en quatre postes. Le premier, …".into()));
        f.chat.flux.clear();
        f.chat.entries.push(Entry::Info("⏹ Arrêté. Ta demande est revenue dans la saisie : corrige-la et renvoie-la.".into()));
        f.chat.input.set("Résume en détail le budget que tu viens d'ouvrir");
    });
    s.ligne("Ta question revient dans la saisie : on la corrige et on la renvoie.").attendre(1.6);
    s.a(|f| f.chat.input.set("Résume en trois lignes le budget que tu viens d'ouvrir"));
    s.attendre(1.2);
    s.envoyer();
    s.repondre("**1,2 M Ar** de matériel, **800 000 Ar** de déplacements, le reste en fournitures. Total : 2,6 M Ar, couvert par les cotisations.", 2.6);
    s.attendre(1.6);

    // --- 3. Historique ------------------------------------------------------------------
    s.chapitre("Fils et historique", "Chaque sujet a son fil et sa mémoire. Rien ne se mélange.");
    s.cliquer(Cible::Histo, |f| f.chat.basculer_histo());
    s.attendre(2.6);
    s.ligne("Rangés par date, retrouvés par le sens. Un clic rouvre la lettre d'hier.");
    s.attendre(2.6);
    s.cliquer(Cible::Fil(1), |f| {
        if let Some(r) = fils::liste(Cote::Discussion).into_iter().find(|r| r.titre == FIL_ROUVERT) {
            f.chat.histo_act(HistoAct::Ouvrir(r.id.clone()));
        }
        f.chat.histo = None;
    });
    s.attendre(1.8);

    // --- 4. Côté Code -------------------------------------------------------------------
    s.chapitre("Côté Code", "Un interrupteur : Discussion pour le divers, Code pour agir sur les fichiers.");
    s.cliquer(Cible::Cote(Cote::Code), |f| {
        f.chat.basculer_cote(Cote::Code);
        if let Some(r) = fils::liste(Cote::Code).first() {
            f.chat.histo_act(HistoAct::Ouvrir(r.id.clone()));
        }
        f.chat.histo = None;
        f.chat.entries.clear();
    });
    s.a(|f| {
        f.fenetre = 2;
        f.sons.push((f.t, "ouvre"));
    });
    s.attendre(1.8);
    s.ligne("Le projet « carnet-de-notes » est ouvert dans l'éditeur : un test échoue.");
    s.attendre(2.4);
    s.cliquer(Cible::Saisie, |_| {});
    s.taper("Le test moyenne_vide échoue, corrige src/notes.rs").envoyer();
    s.ligne("Il travaille dans le dossier du projet, affiché sous l'en-tête de la bulle.");
    s.etape("› $ cargo test").etape("  ↳ 1 test en échec : moyenne_vide (division par zéro)").etape("› je lis src/notes.rs");
    s.ligne("Avant de modifier quoi que ce soit, il montre et demande ton accord.");
    s.a(|f| {
        f.chat.confirm = Some("Remplacer le fichier ~/GitHub/carnet-de-notes/src/notes.rs (12 lignes) ?\n\npub fn moyenne(notes: &[f32]) -> Option<f32> {\n    if notes.is_empty() {\n        return None;\n    }\n    Some(notes.iter().sum::<f32>() / notes.len() as f32)\n}".into());
        f.reflechit = None;
    });
    s.attendre(3.6);
    s.cliquer(Cible::Oui, |f| {
        f.chat.confirm = None;
        f.reflechit = Some(f.t);
        f.fenetre = 3;
    });
    s.ligne("Puis il vérifie lui-même : les tests repassent.");
    s.etape("› $ cargo test");
    s.a(|f| f.fenetre = 4);
    s.etape("  ↳ 14 tests réussis");
    s.repondre("Corrigé : une liste vide donnait une division par zéro. `moyenne` renvoie maintenant **rien** dans ce cas. Les 14 tests passent.", 2.8);
    s.a(|f| {
        f.maki.set_attitude(Attitude::Danse, 3.0);
        f.sons.push((f.t, "succes"));
    });
    s.attendre(2.8);

    // --- 5. Réglages --------------------------------------------------------------------
    s.chapitre("Réglages", "Tout se règle dans la bulle : la roue dentée.");
    s.cliquer(Cible::Roue, |f| f.chat.open_settings(0));
    s.attendre(2.4);
    for (onglet, ligne, pause) in [
        (5, "Le cerveau : des modèles gratuits, les tiens, ou un modèle local choisi selon ta machine.", 3.8),
        (2, "Son allure : six lémuriens, sa taille, le thème, où il se promène.", 3.4),
        (3, "Démarrage, mises à jour vérifiées, index des fichiers : tout se coupe d'un clic.", 3.4),
        (4, "Mails, agenda, contacts : les mots de passe vont dans le trousseau du système.", 3.6),
    ] {
        s.ligne(ligne);
        s.a(move |f| {
            if let Some(p) = &mut f.chat.panneau {
                p.onglet = onglet;
                p.scroll = 0.0;
            }
            f.clic = 1.0;
        });
        s.attendre(pause);
    }
    s.cliquer(Cible::Fermer, |f| {
        f.chat.panneau = None;
        f.ouverte = false;
    });

    // Six lémuriens : il change d'allure.
    s.chapitre("Six lémuriens", "Maki catta, sifaka, indri, aye-aye, vari, microcèbe : chacun son caractère.");
    for e in [crate::mascot::especes::Espece::Sifaka, crate::mascot::especes::Espece::Indri, crate::mascot::especes::Espece::Catta] {
        s.a(move |f| {
            draw::set_species(e);
            f.maki.set_attitude(Attitude::Danse, 1.8);
        });
        s.attendre(1.9);
    }

    // --- 6. Fin -------------------------------------------------------------------------
    s.chapitre("Léger et sûr", "16 Mo de mémoire · rien ne quitte ta machine sans ton accord · logiciel libre.");
    s.a(|f| {
        f.maki.set_attitude(Attitude::Danse, 5.0);
        let m = Message::new(Kind::Tip, "À toi de jouer", "raantss18.github.io/L-Katta\nUne commande pour l'installer, il se met à jour tout seul.");
        f.bulle = Some(Bubble { msg: m, ttl: 9.0, age: 0.0 });
    });
    s.attendre(5.5);
    s.a(|f| f.titre_plein = 1.0).attendre(3.0);
    s
}

fn rect(pm: &mut Pixmap, r: R, c: (u8, u8, u8), a: f32) {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba(c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0, a).unwrap_or(Color::BLACK));
    if let Some(rc) = tiny_skia::Rect::from_xywh(r.0, r.1, r.2, r.3) {
        pm.fill_rect(rc, &p, Transform::identity(), None);
    }
}

/// Un bureau d'exemple : fond en dégradé, la fenêtre de travail (le document ou le projet dont
/// parle la scène), la barre du bas.
fn bureau(pm: &mut Pixmap, text: &Text, bold: &Text, fenetre: u8) {
    for y in 0..H {
        let k = y as f32 / H as f32;
        let c = (30.0 + 26.0 * k, 52.0 + 30.0 * k, 74.0 + 26.0 * k);
        rect(pm, (0.0, y as f32, W as f32, 1.0), (c.0 as u8, c.1 as u8, c.2 as u8), 1.0);
    }
    let f: R = (56.0, 44.0, 660.0, 534.0);
    let code = fenetre >= 2;
    let (fond, barre, encre) = if code { ((30, 34, 44), (44, 50, 62), (214, 220, 230)) } else { ((246, 244, 238), (224, 221, 212), (40, 44, 52)) };
    draw::rrect(pm, (f.0 + 4.0, f.1 + 8.0, f.2, f.3), 12.0, &draw::paint((16, 24, 34)), None);
    draw::rrect(pm, f, 12.0, &draw::paint(fond), None);
    draw::rrect(pm, (f.0, f.1, f.2, 34.0), 12.0, &draw::paint(barre), None);
    rect(pm, (f.0, f.1 + 20.0, f.2, 14.0), barre, 1.0);
    let titre = match fenetre {
        0 => "cours-analyse.md — Éditeur",
        1 => "budget-2026.md — ~/Documents/Association — Éditeur",
        _ => "notes.rs — carnet-de-notes — Éditeur de code",
    };
    text.draw(pm, 13.0, f.0 + 16.0, f.1 + 8.0, titre, if code { (170, 178, 192) } else { (70, 70, 76) });
    let (x, mut y) = (f.0 + 34.0, f.1 + 58.0);
    match fenetre {
        0 | 1 => {
            let (h1, lignes): (&str, &[&str]) = if fenetre == 0 {
                (
                    "Chapitre 2 — Dérivabilité",
                    &[
                        "Une fonction f est dérivable en a si le taux d'accroissement",
                        "(f(x) − f(a)) / (x − a) admet une limite finie quand x tend vers a.",
                        "",
                        "Théorème de Rolle. Si f est continue sur [a, b], dérivable sur",
                        "]a, b[ et si f(a) = f(b), alors il existe c dans ]a, b[ tel que",
                        "f′(c) = 0.",
                        "",
                        "Théorème des accroissements finis. Sous les mêmes hypothèses,",
                        "il existe c dans ]a, b[ tel que f(b) − f(a) = f′(c) (b − a).",
                        "",
                        "À faire pour lundi : exercices 4 à 9, page 37.",
                    ],
                )
            } else {
                (
                    "Budget 2026 de l'association",
                    &[
                        "Poste                          Montant prévu",
                        "Matériel (ordinateurs, câbles)     1 200 000 Ar",
                        "Déplacements                         800 000 Ar",
                        "Fournitures et impressions           450 000 Ar",
                        "Imprévus                             150 000 Ar",
                        "",
                        "Total                              2 600 000 Ar",
                        "",
                        "Recettes : cotisations des 52 membres, soit 2 600 000 Ar.",
                        "Le matériel est acheté en mars, après l'assemblée générale.",
                        "À voter en réunion du labo, samedi à 10 h.",
                    ],
                )
            };
            bold.draw(pm, 23.0, x, y, h1, encre);
            y += 50.0;
            for l in lignes {
                let gras = l.starts_with("Total") || l.starts_with("Poste") || l.starts_with("Théorème");
                // Deux colonnes alignées quand la ligne en a deux (séparées par plusieurs espaces).
                let police = if gras { bold } else { text };
                let couleur = if gras { encre } else { (72, 76, 86) };
                match l.split_once("   ") {
                    Some((a, b)) => {
                        police.draw(pm, 15.5, x, y, a.trim(), couleur);
                        let b = b.trim();
                        police.draw(pm, 15.5, x + 470.0 - police.width(15.5, b), y, b, couleur);
                    }
                    None => police.draw(pm, 15.5, x, y, l, couleur),
                }
                y += 31.0;
            }
        }
        _ => {
            let corrige = fenetre >= 3;
            let source: Vec<(&str, (u8, u8, u8))> = if corrige {
                vec![
                    ("/// Moyenne d'une liste de notes (rien si la liste est vide).", (120, 132, 150)),
                    ("pub fn moyenne(notes: &[f32]) -> Option<f32> {", (214, 220, 230)),
                    ("    if notes.is_empty() {", (150, 220, 160)),
                    ("        return None;", (150, 220, 160)),
                    ("    }", (150, 220, 160)),
                    ("    Some(notes.iter().sum::<f32>() / notes.len() as f32)", (150, 220, 160)),
                    ("}", (214, 220, 230)),
                ]
            } else {
                vec![
                    ("/// Moyenne d'une liste de notes.", (120, 132, 150)),
                    ("pub fn moyenne(notes: &[f32]) -> f32 {", (214, 220, 230)),
                    ("    notes.iter().sum::<f32>() / notes.len() as f32", (240, 150, 140)),
                    ("}", (214, 220, 230)),
                ]
            };
            text.draw(pm, 12.5, x, y - 6.0, "carnet-de-notes  ›  src  ›  notes.rs", (130, 140, 158));
            y += 24.0;
            for (i, (l, c)) in source.iter().enumerate() {
                text.draw(pm, 13.0, x, y, &format!("{:>2}", i + 1), (90, 98, 114));
                text.draw(pm, 14.5, x + 34.0, y, l, *c);
                y += 27.0;
            }
            // Le terminal de l'éditeur, en bas.
            let ty = f.1 + f.3 - 190.0;
            rect(pm, (f.0, ty, f.2, 1.0), (60, 68, 84), 1.0);
            text.draw(pm, 12.5, x, ty + 10.0, "TERMINAL", (130, 140, 158));
            let sortie: Vec<(&str, (u8, u8, u8))> = if fenetre >= 4 {
                vec![("$ cargo test", (214, 220, 230)), ("test moyenne_simple ... ok", (150, 220, 160)), ("test moyenne_vide ... ok", (150, 220, 160)), ("test result: ok. 14 passed; 0 failed", (150, 220, 160))]
            } else if fenetre == 3 {
                vec![("$ cargo test", (214, 220, 230)), ("   Compiling carnet-de-notes v0.3.0", (170, 178, 192))]
            } else {
                vec![
                    ("$ cargo test", (214, 220, 230)),
                    ("test moyenne_simple ... ok", (150, 220, 160)),
                    ("test moyenne_vide ... FAILED", (240, 150, 140)),
                    ("  attendu : aucune moyenne ; obtenu : NaN (division par zéro)", (240, 150, 140)),
                    ("test result: FAILED. 13 passed; 1 failed", (240, 150, 140)),
                ]
            };
            let mut yy = ty + 38.0;
            for (l, c) in sortie {
                text.draw(pm, 14.0, x, yy, l, c);
                yy += 26.0;
            }
        }
    }
    // Barre du bureau.
    rect(pm, (0.0, SOL, W as f32, H as f32 - SOL), (22, 28, 38), 0.96);
    for i in 0..5 {
        draw::rrect(pm, (20.0 + i as f32 * 46.0, SOL + 12.0, 32.0, 32.0), 8.0, &draw::paint([(86, 140, 200), (214, 150, 70), (120, 180, 120), (190, 110, 150), (150, 150, 160)][i]), None);
    }
    text.draw(pm, 14.0, W as f32 - 84.0, SOL + 18.0, "08:30", (220, 224, 230));
}

fn pointeur(pm: &mut Pixmap, p: (f32, f32), clic: f32) {
    if clic > 0.0 {
        let mut pa = Paint::default();
        pa.set_color(Color::from_rgba(1.0, 0.82, 0.3, 0.55 * clic).unwrap_or(Color::WHITE));
        pa.anti_alias = true;
        if let Some(c) = PathBuilder::from_circle(p.0, p.1, 10.0 + 22.0 * (1.0 - clic)) {
            pm.fill_path(&c, &pa, FillRule::Winding, Transform::identity(), None);
        }
    }
    let mut pb = PathBuilder::new();
    for (i, (x, y)) in [(0.0, 0.0), (0.0, 19.0), (5.0, 14.5), (8.5, 22.0), (11.5, 20.5), (8.0, 13.5), (14.0, 13.5)].iter().enumerate() {
        if i == 0 { pb.move_to(p.0 + x, p.1 + y) } else { pb.line_to(p.0 + x, p.1 + y) }
    }
    pb.close();
    if let Some(path) = pb.finish() {
        let mut noir = Paint::default();
        noir.set_color(Color::from_rgba8(20, 20, 24, 255));
        noir.anti_alias = true;
        let mut blanc = Paint::default();
        blanc.set_color(Color::WHITE);
        blanc.anti_alias = true;
        pm.fill_path(&path, &blanc, FillRule::Winding, Transform::identity(), None);
        pm.stroke_path(&path, &noir, &tiny_skia::Stroke { width: 1.4, ..Default::default() }, Transform::identity(), None);
    }
}

fn image(f: &mut Film, pm: &mut Pixmap, text: &Text, bold: &Text) {
    draw::set_theme("clair");
    bureau(pm, text, bold, f.fenetre);
    // Le maki, assis sur la barre.
    let pose = f.maki.pose();
    draw::maki(pm, Transform::from_translate(f.maki.pos.0, f.maki.pos.1).pre_scale(1.15, 1.15), &pose);
    let (hx, ty) = (f.maki.pos.0 - 6.0, f.maki.pos.1 - 132.0);
    if f.ouverte {
        // Hauteur réelle de la bulle à l'image précédente (elle change peu d'une image à l'autre).
        let h = f.lay.as_ref().map(|l| l.rect.3).filter(|h| *h > 10.0).unwrap_or_else(|| draw::chat_height(text, &f.chat, U));
        let w = 400.0 * U;
        let bx = (hx - w * 0.82).clamp(16.0, W as f32 - w - 16.0);
        // Bulle au-dessus de lui : `by` est son bas ; une bulle très haute descend un peu.
        let by = (ty - 14.0).max(h + 10.0);
        let caret = (f.t * 2.0) as i64 % 2 == 0;
        f.lay = Some(draw::chat_bubble(pm, text, &f.chat, hx, ty, true, bx, by, U, caret, true, f.t));
    } else if let Some(b) = &f.bulle {
        let acts: Vec<&str> = match b.msg.kind {
            Kind::Hello => vec!["Discuter", "Une astuce"],
            _ => vec![],
        };
        draw::fancy_bubble(pm, text, bold, b, &acts, hx, ty, U);
    }
    // Bandeau du chapitre.
    if !f.chapitre.0.is_empty() && f.titre_plein < 0.5 {
        let (x, y, w) = (40.0, 596.0, 700.0);
        draw::rrect(pm, (x, y, w, 58.0), 12.0, &draw::paint((18, 22, 30)), Some(((70, 84, 104), 1.0)));
        draw::rrect(pm, (x + 12.0, y + 14.0, 4.0, 30.0), 2.0, &draw::paint((240, 178, 70)), None);
        bold.draw(pm, 15.0, x + 28.0, y + 8.0, &f.chapitre.0.to_uppercase(), (240, 178, 70));
        let l = text.wrap(15.0, &f.chapitre.1, w - 48.0);
        text.draw(pm, 15.0, x + 28.0, y + 30.0, l.first().map(String::as_str).unwrap_or(""), (232, 234, 240));
    }
    pointeur(pm, f.souris, f.clic);
    // Carton « l'idée ».
    if f.titre_plein > 1.5 {
        rect(pm, (0.0, 0.0, W as f32, H as f32), (20, 26, 36), 1.0);
        draw::maki(pm, Transform::from_translate(1040.0, 520.0).pre_scale(1.9, 1.9), &pose);
        bold.draw(pm, 17.0, 110.0, 150.0, "L'IDÉE", (240, 178, 70));
        let lignes = [
            ("Tu te souviens du petit trombone d'Office,", (246, 240, 226)),
            ("et de Merlin l'enchanteur ?", (246, 240, 226)),
            ("", (0, 0, 0)),
            ("L Katta est né de cette nostalgie :", (200, 208, 222)),
            ("un compagnon sur le bureau, mais qui comprend,", (200, 208, 222)),
            ("agit sur l'ordinateur et connaît ta machine.", (200, 208, 222)),
        ];
        for (i, (l, c)) in lignes.iter().enumerate() {
            let t = if i < 2 { bold } else { text };
            t.draw(pm, if i < 2 { 34.0 } else { 24.0 }, 110.0, 190.0 + i as f32 * 48.0, l, *c);
        }
        let fin = "Le même plaisir, en bien plus puissant.";
        bold.draw(pm, 26.0, 110.0, 190.0 + 6.4 * 48.0, fin, (240, 178, 70));
        return;
    }
    // Carton de titre.
    if f.titre_plein > 0.5 {
        rect(pm, (0.0, 0.0, W as f32, H as f32), (20, 26, 36), 1.0);
        draw::maki(pm, Transform::from_translate(W as f32 / 2.0, 330.0).pre_scale(1.7, 1.7), &pose);
        let t1 = "L Katta";
        bold.draw(pm, 58.0, (W as f32 - bold.width(58.0, t1)) / 2.0, 360.0, t1, (246, 240, 226));
        let t2 = "Le petit maki assistant de ton ordinateur";
        text.draw(pm, 24.0, (W as f32 - text.width(24.0, t2)) / 2.0, 440.0, t2, (240, 178, 70));
        let t3 = format!("version {} · Linux · libre et gratuit · raantss18.github.io/L-Katta", env!("CARGO_PKG_VERSION"));
        text.draw(pm, 16.0, (W as f32 - text.width(16.0, &t3)) / 2.0, 490.0, &t3, (170, 178, 192));
    }
}

/// Fils d'exemple (dans le dossier d'essai) pour l'historique.
fn fils_exemple() {
    use serde_json::json;
    let demo = |cote: Cote, dossier: Option<&str>, titre: &str, q: &str, r: &str, jours: i64| {
        let mut f = fils::nouveau(cote, dossier.map(String::from));
        f.id = format!("film-{}-{jours}", titre.len());
        f.titre = titre.into();
        f.entries = vec![Entry::User(q.into()), Entry::Maki(r.into())];
        f.history = vec![json!({"role": "user", "content": q})];
        fils::enregistrer(&mut f);
        let idx = crate::paths::data_dir().join("fils/index.json");
        let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&idx).unwrap_or_default()).unwrap_or_default();
        let d = (chrono::Local::now() - chrono::Duration::days(jours)).to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
        if let Some(a) = v["fils"].as_array_mut() {
            for x in a.iter_mut().filter(|x| x["id"] == f.id.as_str()) {
                x["maj"] = json!(d);
            }
        }
        let _ = crate::paths::write_private(&idx, v.to_string().as_bytes());
    };
    demo(Cote::Discussion, None, "Vols Tana – Paris en décembre", "vols pas chers tana paris en décembre", "Les moins chers partent le mardi…", 6);
    demo(Cote::Discussion, None, "Comprendre les intégrales", "explique-moi les intégrales simplement", "Une intégrale mesure une aire…", 3);
    demo(Cote::Discussion, None, "Lettre au propriétaire", "aide-moi à écrire au propriétaire pour la fuite", "Voici une lettre courte et polie : « Monsieur, je vous signale une fuite sous l'évier depuis lundi… »", 1);
    demo(Cote::Discussion, None, "Recette du romazava", "comment faire un bon romazava ?", "Des brèdes mafana, du zébu…", 0);
    demo(Cote::Code, Some("~/Documents/Cours"), "Ranger les PDF par matière", "range les pdf par matière", "Fait : 42 fichiers rangés.", 4);
    demo(Cote::Code, Some("~/GitHub/carnet-de-notes"), "Calcul de la moyenne", "les tests échouent", "…", 0);
}

/// Rend le film (`.mp4`, par ffmpeg) ou une seule image (`.png`, à l'instant `instant`).
pub fn rendre(sortie: &str, instant: Option<f32>) -> Result<(), String> {
    // Dossier d'essai : ni les réglages ni les discussions de l'utilisateur ne sont touchés.
    let essai = std::env::temp_dir().join(format!("lkatta-film-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&essai);
    std::fs::create_dir_all(&essai).map_err(|e| e.to_string())?;
    // Avant tout autre fil : sûr.
    unsafe {
        std::env::set_var("XDG_DATA_HOME", essai.join("data"));
        std::env::set_var("XDG_CONFIG_HOME", essai.join("config"));
    }
    crate::lang::regler("fr");
    let text = Text::load().ok_or(t!("pas de police"))?;
    let bold = draw::load_bold().unwrap_or_else(|| Text::load().expect("police"));
    fils_exemple();
    let mut cfg = crate::config::Config::load();
    cfg.prenom = "Antsa".into();
    cfg.langue = "fr".into();
    cfg.accueil = true;
    let mut chat = Chat::new(cfg);
    crate::lang::regler("fr");
    chat.basculer_cote(Cote::Discussion);
    chat.nouveau_fil(None);
    chat.open = true;
    let mut maki = Maki::new(MAKI_X, 1.0, 7);
    maki.mode = Mode::Sit;
    maki.pos = (MAKI_X, SOL);
    let mut f = Film {
        chat,
        maki,
        ouverte: false,
        bulle: None,
        lay: None,
        souris: (760.0, 420.0),
        vers: (760.0, 420.0),
        clic: 0.0,
        chapitre: (String::new(), String::new()),
        reflechit: None,
        titre_plein: 0.0,
        fenetre: 0,
        sons: vec![],
        t: 0.0,
    };
    let mut sc = scenario();
    let duree = sc.t;
    sc.actions.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut suivante = 0;
    let mut pm = Pixmap::new(W, H).ok_or("image")?;
    let ctx = Ctx {
        w: W as f32,
        h: SOL,
        night: false,
        quiet: true,
        acrobatics: false,
        show_gap_min: 999.0,
        hovered: false,
        pointer: None,
        chatting: true,
        thinking: false,
        bubble_hover: false,
        day: true,
        band: 0.0,
        exits: Exits::default(),
        avoid: None,
        toit: None,
    };
    let mut ffmpeg = match instant {
        Some(_) => None,
        None => Some(
            std::process::Command::new("ffmpeg")
                .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{W}x{H}"), "-r", &format!("{IPS}"), "-i", "-"])
                .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "21", "-preset", "slow", "-movflags", "+faststart", sortie])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("ffmpeg : {e}"))?,
        ),
    };
    let n = (duree * IPS).ceil() as usize;
    let dt = 1.0 / IPS;
    for i in 0..n {
        f.t = i as f32 * dt;
        let avant = f.fenetre;
        while suivante < sc.actions.len() && sc.actions[suivante].0 <= f.t {
            (sc.actions[suivante].1)(&mut f);
            suivante += 1;
        }
        // Le temps « réfléchi » affiché suit le temps du film.
        f.chat.debut = f.reflechit.map(|d| Instant::now() - Duration::from_secs_f32((f.t - d).max(0.0)));
        f.chat.busy = f.reflechit.is_some();
        // Le maki reste assis à sa place (comme bulle ouverte) et vit sa vie d'attitudes.
        f.maki.step(dt, Ctx { thinking: f.chat.busy, ..ctx });
        f.maki.pos = (MAKI_X, SOL);
        // Il pense tant que rien ne s'écrit, puis « dit » la réponse.
        if f.reflechit.is_some() && f.chat.flux.is_empty() && f.chat.confirm.is_none() {
            if f.maki.mode == Mode::Sit {
                f.maki.mode = Mode::Think;
            }
        } else if f.maki.mode == Mode::Think {
            f.maki.mode = Mode::Sit;
        }
        // Il garde les poses sur place (salut, réflexion) ; tout déplacement est annulé.
        if !matches!(f.maki.mode, Mode::Sit | Mode::Wave | Mode::Think | Mode::Point | Mode::Stretch | Mode::Scratch) {
            f.maki.mode = Mode::Sit;
        }
        let _ = avant;
        if let Some(b) = &mut f.bulle {
            b.age += dt;
        }
        f.souris.0 += (f.vers.0 - f.souris.0) * 0.22;
        f.souris.1 += (f.vers.1 - f.souris.1) * 0.22;
        f.clic = (f.clic - dt * 2.4).max(0.0);
        let une = instant.is_some_and(|t| f.t + dt > t);
        if instant.is_none() || une {
            image(&mut f, &mut pm, &text, &bold);
        } else if f.ouverte {
            // Sans dessiner, les zones cliquables ne seraient pas connues : image légère.
            image(&mut f, &mut pm, &text, &bold);
        }
        match &mut ffmpeg {
            Some(p) => p.stdin.as_mut().ok_or("ffmpeg")?.write_all(pm.data()).map_err(|e| format!("ffmpeg : {e}"))?,
            None if une => {
                pm.save_png(sortie).map_err(|e| e.to_string())?;
                break;
            }
            None => {}
        }
    }
    if instant.is_none() {
        let v: Vec<serde_json::Value> = f.sons.iter().map(|(t, g)| serde_json::json!({"t": t, "son": g})).collect();
        let _ = std::fs::write(format!("{sortie}.sons.json"), serde_json::json!({"duree": duree, "sons": v}).to_string());
    }
    if let Some(mut p) = ffmpeg {
        drop(p.stdin.take());
        let st = p.wait().map_err(|e| e.to_string())?;
        if !st.success() {
            return Err("ffmpeg a échoué".into());
        }
    }
    let _ = std::fs::remove_dir_all(&essai);
    println!("{sortie} : {:.0} s", duree);
    Ok(())
}
