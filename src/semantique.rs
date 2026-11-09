//! Proximité de sens sans modèle ni réseau (repris de Tontolo `semantic.py`) : chaque texte
//! devient un vecteur creux de racines de mots (6 lettres) et de trigrammes, sans accents,
//! pondérés en log et normalisés. Assez bon pour retrouver un ancien échange ou choisir les
//! souvenirs utiles, en quelques microsecondes.

use std::collections::HashMap;

pub type Vecteur = HashMap<u32, f32>;

/// FNV-1a 32 bits (stable d'une version à l'autre).
fn hache(prefixe: u8, s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5 ^ prefixe as u32;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

/// Minuscules sans accents (français, malgache).
pub fn simplifier(t: &str) -> String {
    t.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'œ' => 'o',
            c => c,
        })
        .collect()
}

/// Mots de 2 lettres ou plus (lettres et chiffres).
pub fn mots(t: &str) -> Vec<String> {
    simplifier(t).split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 2).map(String::from).collect()
}

pub fn vecteur(t: &str) -> Vecteur {
    let mut v: Vecteur = HashMap::new();
    for w in mots(t) {
        let racine: String = w.chars().take(6).collect();
        *v.entry(hache(b'w', &racine)).or_default() += 1.0;
        let c: Vec<char> = format!(" {w} ").chars().collect();
        for tri in c.windows(3) {
            *v.entry(hache(b't', &tri.iter().collect::<String>())).or_default() += 0.5;
        }
    }
    let mut n = 0.0;
    for x in v.values_mut() {
        *x = (1.0 + *x).ln();
        n += *x * *x;
    }
    let n = n.sqrt();
    if n > 0.0 {
        for x in v.values_mut() {
            *x /= n;
        }
    }
    v
}

pub fn cosinus(a: &Vecteur, b: &Vecteur) -> f32 {
    let (petit, grand) = if a.len() < b.len() { (a, b) } else { (b, a) };
    petit.iter().filter_map(|(k, x)| grand.get(k).map(|y| x * y)).sum()
}

/// Part des mots (de 4 lettres ou plus) de `a` présents dans `b`.
pub fn recouvrement(a: &str, b: &str) -> f32 {
    let ma: std::collections::HashSet<String> = mots(a).into_iter().filter(|w| w.chars().count() >= 4).collect();
    if ma.is_empty() {
        return 0.0;
    }
    let mb: std::collections::HashSet<String> = mots(b).into_iter().collect();
    ma.iter().filter(|w| mb.contains(*w)).count() as f32 / ma.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_texts_are_close() {
        let a = vecteur("Comment nettoyer le cache de yay sur Arch ?");
        let b = vecteur("nettoyage du cache yay, ça prend 18 Go");
        let c = vecteur("Recette du romazava avec des brèdes");
        assert!(cosinus(&a, &b) > 0.35, "{}", cosinus(&a, &b));
        assert!(cosinus(&a, &c) < 0.2, "{}", cosinus(&a, &c));
        assert!((cosinus(&a, &a) - 1.0).abs() < 1e-4);
        assert_eq!(simplifier("Éléphant à Côté"), "elephant a cote");
        assert!(recouvrement("J'aime le café noir le matin", "le matin, café noir") > 0.6);
    }
}
