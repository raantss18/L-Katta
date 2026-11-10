//! Configuration (`~/.config/lkatta/config.toml`), créée au premier lancement.

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Prénom utilisé pour saluer.
    pub prenom: String,
    /// Langue de l'interface et des réponses : « auto » (celle du système), « fr » ou « en ».
    pub langue: String,
    /// Assistant de premier lancement déjà passé (ou fermé).
    pub accueil: bool,
    /// Modèles payants (clé Anthropic, OpenAI…) essayés en premier ; sinon seulement choisis exprès.
    pub payants_en_premier: bool,
    /// Garder (localement, 24 h) les notifications reçues pour pouvoir les résumer.
    pub notifications: bool,
    /// Noter des souvenirs durables au fil des discussions (et s'en servir).
    pub souvenirs: bool,
    /// Un bilan de la machine une fois par semaine (place, cache, mises à jour, problèmes).
    pub bilan_hebdo: bool,
    /// Lancer le maki à l'ouverture de session.
    pub demarrage_auto: bool,
    /// Mises à jour : « proposer » (une bulle demande), « auto » (installées seules), « non ».
    pub maj: String,
    pub maki: MakiCfg,
    /// Moteurs essayés dans l'ordre : le premier qui répond gagne.
    pub moteur: Vec<Moteur>,
    pub actualites: Actualites,
    pub presse_papiers: PressePapiers,
    /// Comptes mail (mots de passe dans le trousseau : `lkatta compte-mail <adresse>`).
    pub mail: Vec<crate::mail::Compte>,
    /// Agendas : adresses iCal secrètes (Google Agenda › Paramètres) ou fichiers .ics.
    pub agenda: Vec<String>,
    /// Carnets d'adresses : fichiers vCard (.vcf).
    pub contacts: Vec<String>,
    pub alertes: Alertes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Alertes {
    /// Signaler les nouveaux mails.
    pub mails: bool,
    /// Prévenir avant les rendez-vous de l'agenda.
    pub agenda: bool,
    /// Minutes d'avance pour les rendez-vous.
    pub avance_min: u32,
}

impl Default for Alertes {
    fn default() -> Self {
        Self { mails: true, agenda: true, avance_min: 15 }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MakiCfg {
    pub actif: bool,
    /// Bulles : « auto » (suit KDE), « clair » ou « sombre ».
    pub theme: String,
    /// Lémurien : catta, sifaka, indri, ayeaye, microcebe, vari.
    pub espece: String,
    /// Taille du texte et des bulles (1.0 = petit, 1.2 par défaut).
    pub texte: f32,
    /// Échelle du personnage (1.0 = environ 90 px de haut).
    pub taille: f32,
    pub acrobaties: bool,
    /// Minutes minimum entre deux numéros (arbre, téléportation, escalade, salto).
    pub numeros_min: u32,
    /// « bords » : il reste près des bords gauche et droit (zone de travail libre) ;
    /// « libre » : tout l'écran.
    pub zone: String,
    /// Se promène d'un écran à l'autre.
    pub ecrans_multiples: bool,
    /// Rejoint l'écran où l'on travaille (souris, fenêtre active).
    pub suivre: bool,
    /// Minutes minimum entre deux paroles spontanées (les rappels passent toujours).
    pub parole_min: u32,
    /// Le maki dort entre ces deux heures (bornes [début, fin[).
    pub sommeil: [u32; 2],
    /// Une astuce toutes les N heures (0 = jamais).
    pub conseils_h: u32,
    /// Durée d'affichage d'une bulle, en secondes.
    pub bulle_s: u32,
    /// Raccourci global qui ouvre la bulle (ex. « Meta+K », vide = aucun).
    pub raccourci: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Moteur {
    pub nom: String,
    /// Base d'une API compatible OpenAI, ex. `http://127.0.0.1:8765/v1`.
    pub url: String,
    pub modele: String,
    /// Modèles de repli envoyés dans `models` (OpenRouter).
    pub replis: Vec<String>,
    /// Clé : variable d'environnement…
    pub cle_env: String,
    /// …ou fichier `KEY=valeur` (ex. `~/.config/openrouter/env`).
    pub cle_fichier: String,
    /// Force un prix nul (OpenRouter : `provider.max_price = 0`).
    pub gratuit_seulement: bool,
    /// Commande pour démarrer le moteur s'il ne répond pas (ex. `tontolo serve`).
    pub demarrer: String,
    pub actif: bool,
}

impl Default for Moteur {
    fn default() -> Self {
        Self {
            nom: String::new(),
            url: String::new(),
            modele: "auto".into(),
            replis: vec![],
            cle_env: String::new(),
            cle_fichier: String::new(),
            gratuit_seulement: true,
            demarrer: String::new(),
            actif: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Actualites {
    pub flux: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PressePapiers {
    pub actif: bool,
    /// Nombre d'entrées gardées.
    pub taille: usize,
}

impl Default for MakiCfg {
    fn default() -> Self {
        Self { actif: true, texte: 1.2, theme: "auto".into(), espece: "catta".into(), taille: 1.0, acrobaties: true, numeros_min: 15, zone: "bords".into(), ecrans_multiples: true, suivre: true, parole_min: 25, sommeil: [23, 7], conseils_h: 2, bulle_s: 9, raccourci: "Meta+K".into() }
    }
}

impl Default for Actualites {
    fn default() -> Self {
        Self {
            flux: vec![
                "https://archlinux.org/feeds/news/".into(),
                "https://kde.org/index.xml".into(),
                "https://linuxfr.org/news.atom".into(),
            ],
        }
    }
}

impl Default for PressePapiers {
    fn default() -> Self {
        Self { actif: true, taille: 40 }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prenom: default_prenom(),
            langue: "auto".into(),
            accueil: false,
            payants_en_premier: false,
            notifications: true,
            souvenirs: true,
            bilan_hebdo: true,
            demarrage_auto: true,
            maj: "proposer".into(),
            maki: MakiCfg::default(),
            moteur: default_moteurs(),
            actualites: Actualites::default(),
            presse_papiers: PressePapiers::default(),
            mail: vec![],
            agenda: vec![],
            contacts: vec![],
            alertes: Alertes::default(),
        }
    }
}

fn default_prenom() -> String {
    // Champ GECOS de /etc/passwd, sinon $USER.
    let user = std::env::var("USER").unwrap_or_default();
    if let Ok(passwd) = std::fs::read_to_string("/etc/passwd") {
        for line in passwd.lines() {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() > 4 && f[0] == user {
                let gecos = f[4].split(',').next().unwrap_or("").trim();
                if let Some(first) = gecos.split_whitespace().next() {
                    return first.to_string();
                }
            }
        }
    }
    user
}

pub fn which(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
        .unwrap_or(false)
}

/// Le moteur intégré (url spéciale `integre`) : aucun serveur, aucun compte.
pub fn moteur_integre() -> Moteur {
    Moteur { nom: "L Katta (intégré)".into(), url: "integre".into(), modele: "auto".into(), ..Default::default() }
}

fn default_moteurs() -> Vec<Moteur> {
    vec![
        moteur_integre(),
        Moteur {
            nom: "Ollama (local)".into(),
            url: "http://127.0.0.1:11434/v1".into(),
            modele: "qwen3.5:4b".into(),
            actif: false,
            ..Default::default()
        },
    ]
}

/// Clé OpenRouter éventuelle (moteur configuré, variable ou ~/.config/openrouter/env) :
/// le moteur intégré s'en sert pour ajouter les modèles gratuits d'OpenRouter.
pub fn openrouter_key(cfg: &Config) -> Option<String> {
    if let Some(k) = cfg.moteur.iter().find(|m| m.url.contains("openrouter.ai")).and_then(|m| m.cle()) {
        return Some(k);
    }
    Moteur { cle_env: "OPENROUTER_API_KEY".into(), cle_fichier: "~/.config/openrouter/env".into(), ..Default::default() }.cle()
}

impl Config {
    /// Charge la configuration ; la crée avec les valeurs par défaut si absente.
    /// Lit la configuration et applique sa langue.
    pub fn load() -> Config {
        let c = Self::lire();
        crate::lang::regler(&c.langue);
        c
    }

    fn lire() -> Config {
        let path = paths::config_file();
        match std::fs::read_to_string(&path) {
            Ok(s) => match toml::from_str::<Config>(&s) {
                Ok(mut c) => {
                    // Installation d'avant le choix de langue : elle reste en français.
                    if !s.lines().any(|l| l.trim_start().starts_with("langue")) {
                        c.langue = "fr".into();
                        let _ = c.save();
                    }
                    // Installation d'avant l'assistant de premier lancement : déjà réglée.
                    if !s.lines().any(|l| l.trim_start().starts_with("accueil")) {
                        c.accueil = true;
                        let _ = c.save();
                    }
                    // L Katta a son propre moteur : il passe en premier ; Tontolo, s'il était
                    // configuré, reste disponible mais désactivé (réactivable par /regle).
                    if !c.moteur.iter().any(|m| m.url == "integre") {
                        for m in c.moteur.iter_mut().filter(|m| m.url.contains(":8765")) {
                            m.actif = false;
                        }
                        c.moteur.insert(0, moteur_integre());
                        let _ = c.save();
                    }
                    // Réglages ajoutés par une nouvelle version : on les écrit dans le fichier
                    // pour qu'ils soient visibles (valeurs déjà choisies conservées).
                    if toml::to_string_pretty(&c).is_ok_and(|full| full.lines().filter(|l| l.contains(" = ")).count() > s.lines().filter(|l| l.contains(" = ")).count()) {
                        let _ = c.save();
                    }
                    c
                }
                Err(e) => {
                    eprintln!("lkatta : {}", t!("config.toml illisible ({}), valeurs par défaut utilisées", e));
                    Config::default()
                }
            },
            Err(_) => {
                let c = Config::default();
                let _ = c.save();
                c
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let s = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let header = "# Configuration de L Katta. Modifiable à la main ; relancer le maki après :\n\
                      #   lkatta quitter && lkatta &\n\
                      # Moteurs : essayés dans l'ordre, le premier qui répond gagne (actif = false pour en couper un).\n\
                      # maki.zone : \"bords\" (laisse le milieu libre) ou \"libre\" ; maki.taille : 1.0 = ~90 px.\n\n";
        paths::write_private(&paths::config_file(), format!("{header}{s}").as_bytes())
    }
}

impl Moteur {
    /// Clé API éventuelle (variable d'environnement puis fichier).
    pub fn cle(&self) -> Option<String> {
        if !self.cle_env.is_empty() {
            if let Ok(v) = std::env::var(&self.cle_env) {
                if !v.trim().is_empty() {
                    return Some(v.trim().to_string());
                }
            }
        }
        if self.cle_fichier.is_empty() {
            return None;
        }
        let txt = std::fs::read_to_string(paths::expand(&self.cle_fichier)).ok()?;
        for line in txt.lines() {
            let line = line.trim().trim_start_matches("export ").trim();
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                if (self.cle_env.is_empty() && k.ends_with("KEY")) || k == self.cle_env {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    }
}

/// Raccourcis : `taille` → `maki.taille`, etc.
fn canonical(path: &str) -> String {
    let p = path.trim().replace(' ', ".");
    let short = ["actif", "texte", "theme", "espece", "taille", "acrobaties", "numeros_min", "zone", "ecrans_multiples", "parole_min", "sommeil", "bulle_s", "conseils_h"];
    if short.contains(&p.as_str()) {
        return format!("maki.{p}");
    }
    match p.as_str() {
        "historique" | "presse" => "presse_papiers.actif".into(),
        "flux" => "actualites.flux".into(),
        _ => p,
    }
}

/// Valeur saisie → valeur TOML du même genre que l'ancienne (« 1,5 » est un nombre
/// décimal pour `taille`, mais « 23,7 » est une liste pour `sommeil`).
fn guess(v: &str, old: &toml::Value) -> toml::Value {
    let t = v.trim();
    match t.to_lowercase().as_str() {
        "oui" | "true" | "vrai" | "on" | "yes" => return toml::Value::Boolean(true),
        "non" | "false" | "faux" | "off" | "no" => return toml::Value::Boolean(false),
        _ => {}
    }
    if let Ok(i) = t.parse::<i64>() {
        return toml::Value::Integer(i);
    }
    if !old.is_array() {
        if let Ok(f) = t.replace(',', ".").parse::<f64>() {
            return toml::Value::Float(f);
        }
    }
    if t.contains(',') || old.is_array() {
        let parts: Vec<&str> = t.split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
        if parts.iter().all(|x| x.parse::<i64>().is_ok()) {
            return toml::Value::Array(parts.iter().map(|x| toml::Value::Integer(x.parse().unwrap())).collect());
        }
        return toml::Value::Array(parts.iter().map(|x| toml::Value::String((*x).into())).collect());
    }
    toml::Value::String(t.into())
}

fn slot<'a>(root: &'a mut toml::Value, path: &str) -> Result<&'a mut toml::Value, String> {
    let mut cur = root;
    for key in path.split('.').filter(|k| !k.is_empty()) {
        cur = match cur {
            toml::Value::Table(t) => t.get_mut(key).ok_or_else(|| t!("réglage inconnu : {}", key))?,
            toml::Value::Array(a) => {
                let n: usize = key.parse().map_err(|_| t!("numéro attendu à la place de « {} »", key))?;
                let len = a.len();
                a.get_mut(n.wrapping_sub(1)).ok_or_else(|| t!("numéro {} inexistant : il y en a {}", n, len))?
            }
            _ => return Err(t!("« {} » : ce réglage n'a pas de sous-réglage", key)),
        };
    }
    Ok(cur)
}

impl Config {
    /// Modifie un réglage par son chemin (`zone`, `maki.taille`, `moteur.2.modele`…),
    /// vérifie qu'il reste valide, et enregistre.
    pub fn set_path(&mut self, path: &str, value: &str) -> Result<String, String> {
        let path = canonical(path);
        let root = toml::Value::try_from(&*self).map_err(|e| e.to_string())?;
        let old = {
            let mut r = root.clone();
            slot(&mut r, &path)?.clone()
        };
        if old.is_table() {
            return Err(t!("« {} » regroupe plusieurs réglages : précise lequel (/reglages)", path));
        }
        let mut tries = vec![guess(value, &old)];
        if !matches!(tries[0], toml::Value::String(_)) {
            tries.push(toml::Value::String(value.trim().into()));
        }
        let mut last_err = String::new();
        for v in tries {
            let mut r = root.clone();
            *slot(&mut r, &path)? = v.clone();
            match r.try_into::<Config>() {
                Ok(c) if std::mem::discriminant(&old) == std::mem::discriminant(&v) || old.is_float() && v.is_integer() => {
                    let c = c.normalized();
                    *self = c;
                    self.save().map_err(|e| e.to_string())?;
                    return Ok(format!("{path} = {}", fmt_value(&slot(&mut toml::Value::try_from(&*self).unwrap(), &path)?.clone())));
                }
                Ok(_) => last_err = t!("type attendu : {}", type_name(&old)),
                Err(_) => last_err = t!("attendu : {}", type_name(&old)),
            }
        }
        Err(t!("valeur « {} » refusée pour {} ({})", value.trim(), path, last_err))
    }

    /// Valeur d'un réglage par son chemin (mêmes noms que `set_path`).
    pub fn get_path(&self, path: &str) -> Option<toml::Value> {
        let mut root = toml::Value::try_from(self).ok()?;
        slot(&mut root, &canonical(path)).ok().cloned()
    }

    pub fn get_bool(&self, path: &str) -> bool {
        self.get_path(path).and_then(|v| v.as_bool()).unwrap_or(false)
    }

    pub fn get_num(&self, path: &str) -> f64 {
        self.get_path(path).and_then(|v| v.as_float().or(v.as_integer().map(|i| i as f64))).unwrap_or(0.0)
    }

    /// Bornes raisonnables pour les nombres.
    fn normalized(mut self) -> Config {
        self.maki.taille = self.maki.taille.clamp(0.5, 3.0);
        self.maki.texte = self.maki.texte.clamp(0.8, 2.2);
        self.maki.sommeil = [self.maki.sommeil[0] % 24, self.maki.sommeil[1] % 24];
        match crate::mascot::especes::Espece::parse(&self.maki.espece) {
            Some(e) => self.maki.espece = e.id().into(),
            None => self.maki.espece = "catta".into(),
        }
        if !["auto", "clair", "sombre"].contains(&self.maki.theme.as_str()) {
            self.maki.theme = "auto".into();
        }
        if !["proposer", "auto", "non"].contains(&self.maj.as_str()) {
            self.maj = "proposer".into();
        }
        if !["bords", "libre"].contains(&self.maki.zone.as_str()) {
            self.maki.zone = "bords".into();
        }
        self.presse_papiers.taille = self.presse_papiers.taille.clamp(1, 500);
        self
    }

    /// Résumé lisible des réglages, avec les noms à utiliser pour les changer.
    pub fn summary(&self) -> String {
        let m = &self.maki;
        let mut out = vec![
            t!("prenom = {}", self.prenom),
            t!("demarrage_auto = {} · maj = {}   (maj : proposer | auto | non)", oui(self.demarrage_auto), self.maj),
            t!("langue = {}   (auto | fr | en)", self.langue),
            t!("theme = {}   (auto | clair | sombre)", m.theme),
            t!("espece = {}   (catta, sifaka, indri, ayeaye, microcebe, vari — /especes)", m.espece),
            t!("zone = {}   (bords | libre)", m.zone),
            t!("taille = {}   (relancer le maki pour l'appliquer)", m.taille),
            t!("acrobaties = {} · numeros_min = {} · ecrans_multiples = {}", oui(m.acrobaties), m.numeros_min, oui(m.ecrans_multiples)),
            t!("parole_min = {} · bulle_s = {} · sommeil = {},{} · conseils_h = {} (astuce toutes les N h, 0 = jamais)", m.parole_min, m.bulle_s, m.sommeil[0], m.sommeil[1], m.conseils_h),
            t!("presse = {} (historique du presse-papiers, {} entrées)", oui(self.presse_papiers.actif), self.presse_papiers.taille),
            t!("Moteurs (essayés dans l'ordre) :").into(),
        ];
        for (i, e) in self.moteur.iter().enumerate() {
            out.push(t!(
                "  moteur {} : {} — modele = {}{} {}",
                i + 1,
                e.nom,
                e.modele,
                if e.replis.is_empty() { String::new() } else { t!(" (replis : {})", e.replis.join(", ")) },
                if e.actif { "" } else { t!("[coupé]") }
            ));
        }
        out.push(t!("flux = {}", self.actualites.flux.join(", ")));
        out.join("\n")
    }
}

fn oui(b: bool) -> &'static str {
    if b { t!("oui") } else { t!("non") }
}

fn type_name(v: &toml::Value) -> &'static str {
    match v {
        toml::Value::Boolean(_) => t!("oui/non"),
        toml::Value::Integer(_) => t!("nombre entier"),
        toml::Value::Float(_) => t!("nombre"),
        toml::Value::Array(_) => t!("liste séparée par des virgules"),
        _ => t!("texte"),
    }
}

fn fmt_value(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Boolean(b) => oui(*b).into(),
        toml::Value::Array(a) => a.iter().map(fmt_value).collect::<Vec<_>>().join(", "),
        toml::Value::Float(f) => {
            let t = format!("{:.2}", f);
            t.trim_end_matches('0').trim_end_matches('.').to_string()
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_toml() {
        let c = Config::default();
        let s = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.moteur.len(), c.moteur.len());
        assert_eq!(back.maki.sommeil, [23, 7]);
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let c: Config = toml::from_str("prenom = \"Rabe\"\n[maki]\ntaille = 1.5\n").unwrap();
        assert_eq!(c.prenom, "Rabe");
        assert_eq!(c.maki.taille, 1.5);
        assert!(c.maki.actif);
    }

    #[test]
    fn set_by_path() {
        let mut c = Config::default();
        // Pas d'écriture disque dans les tests : on vérifie via une copie.
        let root = toml::Value::try_from(&c).unwrap();
        assert!(root.get("maki").is_some());
        assert_eq!(canonical("zone"), "maki.zone");
        assert_eq!(canonical("moteur 2 modele"), "moteur.2.modele");
        let (f, a, t) = (toml::Value::Float(1.0), toml::Value::Array(vec![]), toml::Value::String(String::new()));
        assert_eq!(guess("oui", &t), toml::Value::Boolean(true));
        assert_eq!(guess("1,5", &f), toml::Value::Float(1.5));
        assert_eq!(guess("23,7", &a), toml::Value::Array(vec![toml::Value::Integer(23), toml::Value::Integer(7)]));
        assert_eq!(guess("https://a/b", &a), toml::Value::Array(vec![toml::Value::String("https://a/b".into())]));
        assert_eq!(guess("qwen3.5:4b", &t), toml::Value::String("qwen3.5:4b".into()));
        let mut r = toml::Value::try_from(&c).unwrap();
        assert!(slot(&mut r, "moteur.9.modele").is_err());
        assert!(slot(&mut r, "maki.inconnu").is_err());
        c.maki.zone = "nimporte".into();
        assert_eq!(c.normalized().maki.zone, "bords");
    }
}
