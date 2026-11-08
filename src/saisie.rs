//! Ligne de saisie de la bulle : curseur déplaçable, sélection (Maj + flèches, Ctrl+A),
//! mots (Ctrl + flèches, Ctrl + Retour arrière), copier, couper, coller.
//! Les positions sont en caractères (pas en octets) : les accents ne cassent rien.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Saisie {
    texte: String,
    /// Position du curseur, en caractères.
    pub curseur: usize,
    /// Autre bout de la sélection, s'il y en a une.
    pub ancre: Option<usize>,
}

pub const MAX: usize = 2000;

impl Saisie {
    pub fn texte(&self) -> &str {
        &self.texte
    }
    pub fn is_empty(&self) -> bool {
        self.texte.is_empty()
    }
    fn len(&self) -> usize {
        self.texte.chars().count()
    }
    fn octet(&self, i: usize) -> usize {
        self.texte.char_indices().nth(i).map(|(b, _)| b).unwrap_or(self.texte.len())
    }

    /// Remplace tout le texte, curseur à la fin.
    pub fn set(&mut self, t: impl Into<String>) {
        self.texte = t.into();
        self.curseur = self.len();
        self.ancre = None;
    }
    /// Vide et rend le texte.
    pub fn prendre(&mut self) -> String {
        self.curseur = 0;
        self.ancre = None;
        std::mem::take(&mut self.texte)
    }

    /// Sélection ordonnée (début, fin), vide exclue.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.ancre?;
        (a != self.curseur).then(|| (a.min(self.curseur), a.max(self.curseur)))
    }
    pub fn texte_selectionne(&self) -> Option<String> {
        let (a, b) = self.selection()?;
        Some(self.texte.chars().skip(a).take(b - a).collect())
    }
    fn effacer_selection(&mut self) -> bool {
        let Some((a, b)) = self.selection() else {
            self.ancre = None;
            return false;
        };
        let (ba, bb) = (self.octet(a), self.octet(b));
        self.texte.replace_range(ba..bb, "");
        self.curseur = a;
        self.ancre = None;
        true
    }

    /// Insère du texte au curseur (remplace la sélection). Retours à la ligne → espaces.
    pub fn inserer(&mut self, t: &str) {
        self.effacer_selection();
        let t: String = t.chars().map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c }).filter(|c| !c.is_control()).collect();
        let place = MAX.saturating_sub(self.len());
        let t: String = t.chars().take(place).collect();
        let b = self.octet(self.curseur);
        self.texte.insert_str(b, &t);
        self.curseur += t.chars().count();
    }

    pub fn retour_arriere(&mut self, mot: bool) {
        if self.effacer_selection() || self.curseur == 0 {
            return;
        }
        let debut = if mot { self.mot_precedent() } else { self.curseur - 1 };
        let (a, b) = (self.octet(debut), self.octet(self.curseur));
        self.texte.replace_range(a..b, "");
        self.curseur = debut;
    }

    pub fn supprimer(&mut self, mot: bool) {
        if self.effacer_selection() || self.curseur >= self.len() {
            return;
        }
        let fin = if mot { self.mot_suivant() } else { self.curseur + 1 };
        let (a, b) = (self.octet(self.curseur), self.octet(fin));
        self.texte.replace_range(a..b, "");
    }

    fn chars(&self) -> Vec<char> {
        self.texte.chars().collect()
    }
    fn mot_precedent(&self) -> usize {
        let c = self.chars();
        let mut i = self.curseur.min(c.len());
        while i > 0 && c[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !c[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }
    fn mot_suivant(&self) -> usize {
        let c = self.chars();
        let mut i = self.curseur;
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        while i < c.len() && !c[i].is_whitespace() {
            i += 1;
        }
        i
    }

    /// Déplace le curseur ; `etendre` (Maj) garde ou commence une sélection.
    fn aller(&mut self, to: usize, etendre: bool) {
        if etendre {
            if self.ancre.is_none() {
                self.ancre = Some(self.curseur);
            }
        } else {
            self.ancre = None;
        }
        self.curseur = to.min(self.len());
    }
    pub fn gauche(&mut self, mot: bool, etendre: bool) {
        // Sans Maj, une sélection se replie à son début.
        if !etendre {
            if let Some((a, _)) = self.selection() {
                return self.aller(a, false);
            }
        }
        let to = if mot { self.mot_precedent() } else { self.curseur.saturating_sub(1) };
        self.aller(to, etendre);
    }
    pub fn droite(&mut self, mot: bool, etendre: bool) {
        if !etendre {
            if let Some((_, b)) = self.selection() {
                return self.aller(b, false);
            }
        }
        let to = if mot { self.mot_suivant() } else { self.curseur + 1 };
        self.aller(to, etendre);
    }
    pub fn debut(&mut self, etendre: bool) {
        self.aller(0, etendre);
    }
    pub fn fin(&mut self, etendre: bool) {
        let n = self.len();
        self.aller(n, etendre);
    }
    pub fn tout_selectionner(&mut self) {
        self.ancre = Some(0);
        self.curseur = self.len();
    }
    /// Place le curseur (clic) ; avec Maj, étend la sélection.
    pub fn placer(&mut self, i: usize, etendre: bool) {
        self.aller(i, etendre);
    }
    /// Couper : rend le texte sélectionné et l'efface.
    pub fn couper(&mut self) -> Option<String> {
        let t = self.texte_selectionne()?;
        self.effacer_selection();
        Some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_in_the_middle_with_accents() {
        let mut s = Saisie::default();
        s.inserer("été chaud");
        s.gauche(true, false); // avant « chaud »
        s.inserer("très ");
        assert_eq!(s.texte(), "été très chaud");
        s.debut(false);
        s.supprimer(false);
        assert_eq!(s.texte(), "té très chaud");
        s.fin(false);
        s.retour_arriere(true);
        assert_eq!(s.texte(), "té très ");
        s.retour_arriere(false);
        assert_eq!(s.texte(), "té très");
    }

    #[test]
    fn selects_copies_and_replaces() {
        let mut s = Saisie::default();
        s.set("bonjour le maki");
        s.gauche(true, true);
        assert_eq!(s.texte_selectionne().as_deref(), Some("maki"));
        s.inserer("lémurien");
        assert_eq!(s.texte(), "bonjour le lémurien");
        s.tout_selectionner();
        assert_eq!(s.couper().as_deref(), Some("bonjour le lémurien"));
        assert!(s.is_empty());
        s.inserer("a\nb");
        assert_eq!(s.texte(), "a b");
        // Flèche sans Maj : la sélection se replie.
        s.tout_selectionner();
        s.gauche(false, false);
        assert_eq!((s.curseur, s.selection()), (0, None));
    }
}
