//! Les lémuriens de Madagascar proposés comme personnage : allure (couleurs, oreilles,
//! museau, queue) et façon de bouger (démarche, numéro favori, rythme de vie).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Espece {
    /// Maki catta (*Lemur catta*) — « maky ».
    #[default]
    Catta,
    /// Propithèque de Verreaux (*Propithecus verreauxi*) — « sifaka ».
    Sifaka,
    /// Indri (*Indri indri*) — « babakoto ».
    Indri,
    /// Aye-aye (*Daubentonia madagascariensis*).
    AyeAye,
    /// Microcèbe (*Microcebus*), le plus petit primate du monde.
    Microcebe,
    /// Vari noir et blanc (*Varecia variegata*) — « varikandana ».
    Vari,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Oreilles {
    Pointues,
    /// Rondes et duveteuses.
    Rondes,
    /// Très grandes (aye-aye, microcèbe).
    Grandes,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Queue {
    /// Anneaux alternés (couleur A, couleur B).
    Annelee((u8, u8, u8), (u8, u8, u8)),
    /// Unie (couleur, bout).
    Unie((u8, u8, u8), (u8, u8, u8)),
}

/// Apparence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pal {
    pub fur: (u8, u8, u8),
    pub fur_dark: (u8, u8, u8),
    pub cream: (u8, u8, u8),
    pub black: (u8, u8, u8),
    pub eye: (u8, u8, u8),
    /// Visage sombre (sinon clair avec masque autour des yeux).
    pub face_dark: bool,
    pub cap: Option<(u8, u8, u8)>,
    pub ruff: bool,
    pub ears: Oreilles,
    pub ear_color: (u8, u8, u8),
    pub queue: Queue,
    /// Longueur relative de la queue (1 = maki catta).
    pub tail_len: f32,
    pub tail_w: f32,
    /// Queue dressée en marchant (sinon portée à l'horizontale).
    pub tail_up: bool,
    /// Longueur relative du museau.
    pub muzzle: f32,
    /// Taille relative des yeux.
    pub eye_r: f32,
    /// Taille relative du corps.
    pub size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Demarche {
    /// À quatre pattes.
    Quatre,
    /// Debout, par bonds (propithèque, indri).
    Bonds,
    /// Petites courses rapides entrecoupées d'arrêts.
    Trottine,
    /// Lent et prudent.
    Lent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numero {
    /// S'étire au soleil.
    Soleil,
    /// Danse de côté par petits bonds.
    Danse,
    /// Chante (notes de musique).
    Chant,
    /// Tapote avec son long doigt.
    Tapote,
    /// Ouvre de grands yeux et regarde partout.
    Guette,
    /// Pousse un grand cri.
    Cri,
}

/// Comportement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub demarche: Demarche,
    pub vitesse: f32,
    pub bond_vy: f32,
    pub bond_vx: f32,
    pub numero: Numero,
    /// Actif la nuit : paupières lourdes le jour.
    pub nocturne: bool,
}

impl Espece {
    pub const TOUTES: [Espece; 6] = [Espece::Catta, Espece::Sifaka, Espece::Indri, Espece::AyeAye, Espece::Microcebe, Espece::Vari];

    pub fn parse(s: &str) -> Option<Espece> {
        let s = s.trim().to_lowercase().replace(['-', ' ', 'è', 'é'], "");
        Some(match s.as_str() {
            "catta" | "maki" | "maky" | "makicatta" | "ringtailed" | "ringtailedlemur" => Espece::Catta,
            "sifaka" | "propithque" | "propitheque" | "simpona" => Espece::Sifaka,
            "indri" | "babakoto" => Espece::Indri,
            "ayeaye" | "aiai" => Espece::AyeAye,
            "microcbe" | "microcebe" | "tsidy" | "lemurmouse" | "mouselemur" => Espece::Microcebe,
            "vari" | "varikandana" | "varinoiretblanc" | "ruffed" | "ruffedlemur" | "blackandwhiteruffedlemur" => Espece::Vari,
            _ => return None,
        })
    }

    pub fn id(self) -> &'static str {
        match self {
            Espece::Catta => "catta",
            Espece::Sifaka => "sifaka",
            Espece::Indri => "indri",
            Espece::AyeAye => "ayeaye",
            Espece::Microcebe => "microcebe",
            Espece::Vari => "vari",
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Espece::Catta => t!("Maki catta (maky)"),
            Espece::Sifaka => t!("Propithèque de Verreaux (sifaka)"),
            Espece::Indri => t!("Indri (babakoto)"),
            Espece::AyeAye => t!("Aye-aye"),
            Espece::Microcebe => t!("Microcèbe"),
            Espece::Vari => t!("Vari noir et blanc (varikandana)"),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Espece::Catta => t!("queue annelée dressée, marche à quatre pattes, aime s'étirer au soleil"),
            Espece::Sifaka => t!("tout blanc à calotte brune, se déplace debout par bonds de côté : il danse"),
            Espece::Indri => t!("le plus grand, noir et blanc, presque sans queue, bondit debout et chante"),
            Espece::AyeAye => t!("grandes oreilles, gros yeux, long doigt qui tapote ; lent, plutôt nocturne"),
            Espece::Microcebe => t!("minuscule, roux, yeux énormes ; trottine par à-coups, plutôt nocturne"),
            Espece::Vari => t!("collerette noir et blanc, longue queue noire, pousse de grands cris"),
        }
    }

    pub fn pal(self) -> Pal {
        match self {
            Espece::Catta => Pal {
                fur: (150, 148, 146),
                fur_dark: (104, 102, 102),
                cream: (244, 241, 233),
                black: (34, 32, 32),
                eye: (246, 166, 35),
                face_dark: false,
                cap: None,
                ruff: false,
                ears: Oreilles::Pointues,
                ear_color: (104, 102, 102),
                queue: Queue::Annelee((244, 241, 233), (34, 32, 32)),
                tail_len: 1.0,
                tail_w: 10.0,
                tail_up: true,
                muzzle: 1.0,
                eye_r: 1.0,
                size: 1.0,
            },
            Espece::Sifaka => Pal {
                fur: (238, 235, 228),
                fur_dark: (214, 208, 198),
                cream: (250, 248, 243),
                black: (38, 34, 32),
                eye: (236, 186, 52),
                face_dark: true,
                cap: Some((122, 82, 52)),
                ruff: false,
                ears: Oreilles::Rondes,
                ear_color: (226, 222, 214),
                queue: Queue::Unie((236, 233, 226), (224, 220, 212)),
                tail_len: 1.05,
                tail_w: 8.0,
                tail_up: false,
                muzzle: 0.75,
                eye_r: 1.05,
                size: 1.05,
            },
            Espece::Indri => Pal {
                fur: (44, 42, 46),
                fur_dark: (26, 25, 28),
                cream: (232, 230, 222),
                black: (22, 20, 22),
                eye: (196, 206, 96),
                face_dark: true,
                cap: None,
                ruff: true,
                ears: Oreilles::Rondes,
                ear_color: (30, 28, 32),
                queue: Queue::Unie((44, 42, 46), (30, 28, 32)),
                tail_len: 0.12,
                tail_w: 9.0,
                tail_up: false,
                muzzle: 1.25,
                eye_r: 1.0,
                size: 1.25,
            },
            Espece::AyeAye => Pal {
                fur: (72, 64, 60),
                fur_dark: (46, 41, 39),
                cream: (150, 138, 118),
                black: (30, 26, 24),
                eye: (244, 172, 36),
                face_dark: false,
                cap: None,
                ruff: false,
                ears: Oreilles::Grandes,
                ear_color: (40, 34, 32),
                queue: Queue::Unie((58, 52, 50), (96, 90, 84)),
                tail_len: 1.0,
                tail_w: 15.0,
                tail_up: false,
                muzzle: 0.7,
                eye_r: 1.35,
                size: 0.95,
            },
            Espece::Microcebe => Pal {
                fur: (176, 118, 78),
                fur_dark: (132, 86, 56),
                cream: (236, 218, 194),
                black: (60, 36, 26),
                eye: (196, 112, 40),
                face_dark: false,
                cap: None,
                ruff: false,
                ears: Oreilles::Grandes,
                ear_color: (196, 140, 112),
                queue: Queue::Unie((156, 104, 70), (120, 80, 54)),
                tail_len: 1.0,
                tail_w: 5.0,
                tail_up: false,
                muzzle: 0.6,
                eye_r: 1.6,
                size: 0.6,
            },
            Espece::Vari => Pal {
                fur: (236, 234, 228),
                fur_dark: (34, 32, 34),
                cream: (246, 244, 238),
                black: (24, 22, 24),
                eye: (226, 204, 64),
                face_dark: true,
                cap: None,
                ruff: true,
                ears: Oreilles::Rondes,
                ear_color: (240, 238, 232),
                queue: Queue::Unie((32, 30, 32), (24, 22, 24)),
                tail_len: 1.1,
                tail_w: 12.0,
                tail_up: false,
                muzzle: 1.15,
                eye_r: 1.0,
                size: 1.15,
            },
        }
    }

    pub fn style(self) -> Style {
        match self {
            Espece::Catta => Style { demarche: Demarche::Quatre, vitesse: 72.0, bond_vy: 430.0, bond_vx: 330.0, numero: Numero::Soleil, nocturne: false },
            Espece::Sifaka => Style { demarche: Demarche::Bonds, vitesse: 90.0, bond_vy: 520.0, bond_vx: 250.0, numero: Numero::Danse, nocturne: false },
            Espece::Indri => Style { demarche: Demarche::Bonds, vitesse: 90.0, bond_vy: 600.0, bond_vx: 300.0, numero: Numero::Chant, nocturne: false },
            Espece::AyeAye => Style { demarche: Demarche::Lent, vitesse: 34.0, bond_vy: 360.0, bond_vx: 200.0, numero: Numero::Tapote, nocturne: true },
            Espece::Microcebe => Style { demarche: Demarche::Trottine, vitesse: 160.0, bond_vy: 380.0, bond_vx: 260.0, numero: Numero::Guette, nocturne: true },
            Espece::Vari => Style { demarche: Demarche::Quatre, vitesse: 80.0, bond_vy: 460.0, bond_vx: 340.0, numero: Numero::Cri, nocturne: false },
        }
    }

    /// Ce que l'espèce dit d'elle-même quand on la choisit.
    pub fn presentation(self) -> &'static str {
        match self {
            Espece::Catta => t!("Me voilà en maki catta : queue en l'air, je suis prêt !"),
            Espece::Sifaka => t!("Me voilà en sifaka : regarde-moi danser de côté !"),
            Espece::Indri => t!("Me voilà en babakoto : le plus grand des lémuriens, et je chante !"),
            Espece::AyeAye => t!("Me voilà en aye-aye : grandes oreilles, et un doigt qui tapote."),
            Espece::Microcebe => t!("Me voilà en microcèbe : tout petit, mais j'ai l'œil !"),
            Espece::Vari => t!("Me voilà en vari : attention, je peux crier fort !"),
        }
    }
}

pub fn liste() -> String {
    Espece::TOUTES.iter().map(|e| t!("- {} : {} — {}", e.id(), e.nom(), e.description())).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_roundtrip() {
        for e in Espece::TOUTES {
            assert_eq!(Espece::parse(e.id()), Some(e));
        }
        assert_eq!(Espece::parse("Babakoto"), Some(Espece::Indri));
        assert_eq!(Espece::parse("aye-aye"), Some(Espece::AyeAye));
        assert_eq!(Espece::parse("microcèbe"), Some(Espece::Microcebe));
        assert_eq!(Espece::parse("girafe"), None);
    }
}
