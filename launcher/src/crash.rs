//! Analyse d'un plantage : extrait des logs (`latest.log`, fin du journal du jeu) et du dernier rapport de crash des causes
//! lisibles en français. Le parsing est pur (aucun accès disque ni réseau) ; `service::crash_view` lit les fichiers.
use serde::Serialize;

const MAX_FINDINGS: usize = 8;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Finding {
    /// `missingMod`, `modVersion`, `incompatible`, `java`, `mixin`, `suspect`, `memory`, `error`.
    pub kind: &'static str,
    pub text: String,
}

fn f(kind: &'static str, text: String) -> Finding {
    Finding { kind, text }
}

/// Texte entre la première paire de `'` après `from` : `Mod 'Nom' (id)` -> `Nom`.
fn quoted(s: &str) -> Option<&str> {
    let a = s.find('\'')? + 1;
    let b = s[a..].find('\'')? + a;
    Some(&s[a..b])
}

/// Ligne Fabric `Mod 'Nom' (id) 1.0 requires … of dep, which is missing!` (et variantes).
fn fabric_line(l: &str) -> Option<Finding> {
    let rest = l.trim_start_matches([' ', '\t', '-']).trim();
    let name = quoted(rest.strip_prefix("Mod ")?)?;
    let after = &rest[rest.find("' ")? + 2..];
    if let Some(i) = after.find(" requires ") {
        let req = &after[i + " requires ".len()..];
        let dep = req.split(" of ").nth(1).and_then(|d| d.split([',', ' ', '!']).next()).filter(|d| !d.is_empty())?;
        let (range, _) = req.split_once(" of ")?;
        if req.contains("which is missing") {
            return Some(f("missingMod", format!("Le mod « {name} » requiert « {dep} », absent.")));
        }
        return Some(f("modVersion", format!("Le mod « {name} » requiert {} de « {dep} », mais la version présente ne convient pas.", french_range(range))));
    }
    for (kw, what) in
        [(" conflicts with mod ", "est en conflit avec"), (" is incompatible with mod ", "est incompatible avec"), (" breaks mod ", "est incompatible avec")]
    {
        if let Some(i) = after.find(kw) {
            let other = quoted(&after[i + kw.len() - 1..]).unwrap_or("un autre mod");
            return Some(f("incompatible", format!("Le mod « {name} » {what} « {other} ».")));
        }
    }
    None
}

fn french_range(r: &str) -> String {
    match r.trim() {
        "any version" => "n'importe quelle version".into(),
        v => format!("la version {}", v.trim_start_matches("version ")),
    }
}

/// Forge / NeoForge : `Mod ID: 'jei', Requested by: 'exemple', Expected range: '[19.0,)', Actual version: '[MISSING]'`.
fn forge_line(l: &str) -> Option<Finding> {
    let l = l.trim();
    let after_id = l.strip_prefix("Mod ID: ")?;
    let id = quoted(after_id)?;
    let by = quoted(l.split("Requested by: ").nth(1)?)?;
    let range = quoted(l.split("Expected range: ").nth(1)?)?;
    let actual = quoted(l.split("Actual version: ").nth(1)?)?;
    Some(if actual.contains("MISSING") {
        f("missingMod", format!("Le mod « {by} » requiert « {id} », absent."))
    } else {
        f("modVersion", format!("Le mod « {by} » requiert « {id} » {range}, mais la version présente est {actual}."))
    })
}

/// `class file version 65.0 … up to 61.0` -> version de Java requise et utilisée.
fn java_line(l: &str) -> Option<Finding> {
    if !l.contains("UnsupportedClassVersionError") && !l.contains("class file version") {
        return None;
    }
    let num = |marker: &str| -> Option<u32> {
        let s = &l[l.find(marker)? + marker.len()..];
        s.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
    };
    let need = num("class file version ")?;
    let have = num("versions up to ").or_else(|| num("up to "));
    Some(f(
        "java",
        match have {
            Some(h) => format!(
                "Version de Java trop ancienne : Java {} requis, Java {} utilisé. Vérifie et répare le pack pour réinstaller Java.",
                need.saturating_sub(44),
                h.saturating_sub(44)
            ),
            None => format!("Version de Java trop ancienne : Java {} requis.", need.saturating_sub(44)),
        },
    ))
}

fn memory_line(l: &str) -> Option<Finding> {
    ["java.lang.OutOfMemoryError", "Could not reserve enough space", "There is insufficient memory", "GC overhead limit exceeded"]
        .iter()
        .any(|k| l.contains(k))
        .then(|| f("memory", "Mémoire insuffisante : augmente la mémoire allouée dans les réglages d'Opaline (ou ferme d'autres programmes).".into()))
}

/// `Mixin apply for mod sodium failed …` / `… from mod sodium -> net.minecraft.X` / `Critical injection failure … in sodium.mixins.json`.
fn mixin_line(l: &str) -> Option<Finding> {
    let word = |s: &str| s.split(|c: char| c.is_whitespace() || c == ',' || c == ':').next().unwrap_or("").to_string();
    let culprit = if let Some(i) = l.find("Mixin apply for mod ") {
        word(&l[i + "Mixin apply for mod ".len()..])
    } else if l.contains("Mixin") && l.contains("failed") && l.contains("from mod ") {
        word(&l[l.find("from mod ")? + "from mod ".len()..])
    } else if l.contains("InjectionError") || l.contains("InvalidInjectionException") || l.contains("Critical injection failure") {
        let cfg = l.split_whitespace().find(|w| w.contains(".mixins.json")).or_else(|| l.split_whitespace().find(|w| w.contains(".mixin.json")))?;
        cfg.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_' && c != '-').split(".mixin").next()?.to_string()
    } else {
        return None;
    };
    let culprit = culprit.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-').to_string();
    (!culprit.is_empty())
        .then(|| f("mixin", format!("Erreur de mixin causée par le mod « {culprit} » (incompatibilité avec une autre version du jeu ou d'un autre mod).")))
}

fn suspects(text: &str) -> Vec<Finding> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let v = l.strip_prefix("Suspected Mods:").or_else(|| l.strip_prefix("Suspected Mod(s):"))?.trim();
            (!v.is_empty() && !v.eq_ignore_ascii_case("none") && !v.eq_ignore_ascii_case("unknown"))
                .then(|| f("suspect", format!("Mod(s) suspecté(s) par le rapport de crash : {v}.")))
        })
        .collect()
}

/// Première ligne de l'exception du rapport de crash (`Description:` puis exception).
fn crash_report_error(text: &str) -> Option<String> {
    let mut it = text.lines();
    let desc = it.by_ref().find_map(|l| l.strip_prefix("Description:"))?.trim().to_string();
    let exc = it.map(str::trim).find(|l| {
        !l.is_empty()
            && (l.starts_with("java.")
                || l.starts_with("net.")
                || l.starts_with("org.")
                || l.starts_with("com.")
                || l.contains("Exception")
                || l.contains("Error"))
    })?;
    Some(format!("{desc} : {exc}"))
}

/// Analyse les textes fournis (logs puis rapport de crash). Résultats dédoublonnés, au plus `MAX_FINDINGS`.
pub fn analyze(logs: &[&str], crash_report: Option<&str>) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    fn push(out: &mut Vec<Finding>, x: Finding) {
        if !out.contains(&x) && out.len() < MAX_FINDINGS {
            out.push(x);
        }
    }
    for text in logs.iter().copied().chain(crash_report) {
        for l in text.lines() {
            let hit = fabric_line(l).or_else(|| forge_line(l)).or_else(|| java_line(l)).or_else(|| mixin_line(l)).or_else(|| memory_line(l));
            if let Some(x) = hit {
                push(&mut out, x);
            }
        }
    }
    if let Some(c) = crash_report {
        suspects(c).into_iter().for_each(|x| push(&mut out, x));
    }
    if out.is_empty()
        && let Some(e) = crash_report.and_then(crash_report_error)
    {
        push(&mut out, f("error", format!("Erreur du jeu : {e}")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FABRIC_LOG: &str = "[12:01:02] [main/INFO]: Loading Minecraft 1.21.1 with Fabric Loader 0.16.9
[12:01:03] [main/ERROR]: Incompatible mod set found! net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!
A potential solution has been determined, this may resolve your problem:
\t - Install lithostitched, any version.
More details:
\t - Mod 'Legendary Monuments' (legendarymonuments) 8.1 requires any version of lithostitched, which is missing!
\t - Mod 'Iris Shaders' (iris) 1.8.0 requires version 0.7.0 or later of sodium, but only the wrong version is present: 0.5.8!
\t - Mod 'Foo' (foo) 2.0 conflicts with mod 'Bar' (bar) 1.0!
";

    #[test]
    fn fabric_dependency_errors() {
        let r = analyze(&[FABRIC_LOG], None);
        assert!(r.contains(&f("missingMod", "Le mod « Legendary Monuments » requiert « lithostitched », absent.".into())), "{r:?}");
        assert!(r.iter().any(|x| x.kind == "modVersion" && x.text.contains("« Iris Shaders »") && x.text.contains("sodium")), "{r:?}");
        assert!(r.contains(&f("incompatible", "Le mod « Foo » est en conflit avec « Bar ».".into())), "{r:?}");
    }

    #[test]
    fn forge_missing_dependency() {
        let log = "Missing or unsupported mandatory dependencies:\n\tMod ID: 'jei', Requested by: 'create', Expected range: '[19.0,)', Actual version: '[MISSING]'\n\tMod ID: 'forge', Requested by: 'create', Expected range: '[47.3,)', Actual version: '47.1.0'";
        let r = analyze(&[log], None);
        assert_eq!(r[0], f("missingMod", "Le mod « create » requiert « jei », absent.".into()));
        assert!(r[1].kind == "modVersion" && r[1].text.contains("47.1.0"), "{r:?}");
    }

    #[test]
    fn java_version_and_memory() {
        let log = "Exception in thread \"main\" java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        assert_eq!(
            analyze(&[log], None)[0],
            f("java", "Version de Java trop ancienne : Java 21 requis, Java 17 utilisé. Vérifie et répare le pack pour réinstaller Java.".into())
        );
        let m = analyze(&["Caused by: java.lang.OutOfMemoryError: Java heap space"], None);
        assert_eq!(m[0].kind, "memory");
        assert_eq!(analyze(&["Error occurred during initialization of VM\nCould not reserve enough space for 8388608KB object heap"], None)[0].kind, "memory");
    }

    #[test]
    fn mixin_names_the_mod() {
        let log = "[12:00:01] [Render thread/ERROR]: Mixin apply for mod sodium failed sodium.mixins.json:core.render.WorldRendererMixin from mod sodium -> net.minecraft.client.render.WorldRenderer: org.spongepowered.asm.mixin.injection.throwables.InvalidInjectionException @Inject annotation on onRender could not find any targets";
        let r = analyze(&[log], None);
        assert_eq!(r, [f("mixin", "Erreur de mixin causée par le mod « sodium » (incompatibilité avec une autre version du jeu ou d'un autre mod).".into())]);
        let r = analyze(&["Critical injection failure: @Inject annotation on tick in lithium.mixins.json:gen.Foo failed injection check"], None);
        assert!(r[0].text.contains("« lithium »"), "{r:?}");
    }

    const CRASH: &str = "---- Minecraft Crash Report ----
// Why did you do that?

Time: 2026-11-01 12:00:00
Description: Ticking entity

java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.m_20182_()\" because \"e\" is null
\tat com.example.cobblemon.Foo.tick(Foo.java:42)

-- System Details --
\tSuspected Mods: Cobblemon (cobblemon)
";

    #[test]
    fn crash_report_suspects_and_fallback() {
        let r = analyze(&[], Some(CRASH));
        assert_eq!(r, [f("suspect", "Mod(s) suspecté(s) par le rapport de crash : Cobblemon (cobblemon).".into())]);
        let no_suspect = CRASH.replace("Cobblemon (cobblemon)", "None");
        let r = analyze(&[], Some(&no_suspect));
        assert_eq!(r.len(), 1);
        assert!(r[0].kind == "error" && r[0].text.starts_with("Erreur du jeu : Ticking entity : java.lang.NullPointerException"), "{r:?}");
        assert!(analyze(&["tout va bien"], None).is_empty());
    }

    #[test]
    fn findings_are_deduplicated_and_bounded() {
        let log = FABRIC_LOG.repeat(3);
        assert_eq!(analyze(&[&log], None).len(), 3);
        let many: String = (0..30).map(|i| format!("\t - Mod 'M{i}' (m{i}) 1 requires any version of d{i}, which is missing!\n")).collect();
        assert_eq!(analyze(&[&many], None).len(), MAX_FINDINGS);
    }
}
