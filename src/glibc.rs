//! Compatibilité avec les glibc plus anciennes que celle de la machine de compilation.
//!
//! glibc 2.43 a donné une nouvelle version à `acosf`, `atan2f` et `log10f` : un binaire compilé sur une
//! distribution récente (Arch) exigeait alors glibc 2.43 et ne démarrait pas sur Ubuntu,
//! Debian ou Fedora. Ces fonctions sont fournies ici (calcul en f64, dont les symboles
//! datent de glibc 2.2.5) : l'édition de liens les prend avant celles de la glibc.
//! `tool/glibc-max.sh` vérifie à chaque publication qu'aucun symbole plus récent ne revient.

#[unsafe(no_mangle)]
pub extern "C" fn acosf(x: f32) -> f32 {
    (x as f64).acos() as f32
}

#[unsafe(no_mangle)]
pub extern "C" fn atan2f(y: f32, x: f32) -> f32 {
    (y as f64).atan2(x as f64) as f32
}

#[unsafe(no_mangle)]
pub extern "C" fn log10f(x: f32) -> f32 {
    (x as f64).log10() as f32
}

#[cfg(test)]
mod tests {
    #[test]
    fn same_results_as_f32_math() {
        assert!((super::acosf(0.5) - 0.5f32.acos()).abs() < 1e-6);
        assert!((super::log10f(1000.0) - 3.0).abs() < 1e-6);
        assert!((super::atan2f(1.0, -1.0) - 1.0f32.atan2(-1.0)).abs() < 1e-6);
    }
}
