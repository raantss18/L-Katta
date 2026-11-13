//! Garde-fou mémoire : L Katta ne doit jamais faire tomber la machine.
//!
//! Toutes les minutes, il mesure sa propre mémoire (privée + partagée avec l'écran). Au-delà
//! du plafond, il note la cause et se relance à neuf (même processus, `exec`), au lieu de
//! grossir jusqu'à ce que le système tue des programmes.

use std::time::Duration;

/// Plafond, en Mo. Le maki vit d'ordinaire avec 60 à 120 Mo.
pub const PLAFOND_MO: u64 = 500;

/// Mémoire résidente du processus (Mo), d'après /proc/self/status.
pub fn memoire_mo() -> Option<u64> {
    lire(&std::fs::read_to_string("/proc/self/status").ok()?)
}

fn lire(status: &str) -> Option<u64> {
    let kb = status.lines().find_map(|l| l.strip_prefix("VmRSS:"))?.trim().trim_end_matches("kB").trim().parse::<u64>().ok()?;
    Some(kb / 1024)
}

/// Surveille la mémoire dans un fil à part.
pub fn surveiller() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(60));
        let Some(mo) = memoire_mo() else { continue };
        if mo > PLAFOND_MO {
            relancer(mo);
        }
    });
}

fn relancer(mo: u64) {
    use std::io::Write;
    let note = format!("{} : {mo} Mo utilisés (plafond {PLAFOND_MO} Mo), relance.\n", chrono::Local::now().format("%Y-%m-%d %H:%M"));
    eprint!("lkatta : {note}");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(crate::paths::data_dir().join("garde-memoire.log")) {
        let _ = f.write_all(note.as_bytes());
    }
    let Ok(exe) = std::env::current_exe() else { std::process::exit(1) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(exe).args(args).exec();
    eprintln!("lkatta : relance impossible ({err}), arrêt.");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_resident_memory() {
        assert_eq!(lire("Name:\tlkatta\nVmRSS:\t   73704 kB\nThreads: 17\n"), Some(71));
        assert_eq!(lire("Name: x\n"), None);
        assert!(memoire_mo().is_some_and(|m| m < PLAFOND_MO));
    }
}
