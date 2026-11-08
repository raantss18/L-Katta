//! « À propos » : version, licence et coordonnées de l'autrice.

pub const AUTEUR: &str = "RANIRIAMANJAKA Antsa";
pub const MAIL: &str = "antsa.raniriamanjaka@univ-antananarivo.mg";
pub const TELEPHONE: &str = "+261 33 43 838 82";
pub const WHATSAPP: &str = "https://wa.me/261334383882";
pub const SITE: &str = "https://raantss18.github.io/L-Katta/";
pub const DEPOT: &str = "https://github.com/raantss18/L-Katta";

/// Texte complet (commande /apropos, `lkatta apropos`).
pub fn texte() -> String {
    format!(
        "L Katta (Lemur Kata) {}\n{}\n\n{} {AUTEUR}\n{} {MAIL}\n{} {TELEPHONE}\n{} {SITE}\n{} {DEPOT}\n\n{}",
        env!("CARGO_PKG_VERSION"),
        t!("Un petit lémurien assistant pour ton bureau Linux."),
        t!("Conçu par"),
        t!("E-mail :"),
        t!("Téléphone et WhatsApp :"),
        t!("Site :"),
        t!("Code source :"),
        t!("Licence Apache-2.0. Moteur d'IA dérivé de Tontolo, outils repris de mi-saina.")
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn about_lists_contacts() {
        let t = super::texte();
        assert!(t.contains(super::MAIL) && t.contains("+261 33 43 838 82") && t.contains(env!("CARGO_PKG_VERSION")));
    }
}
