//! Drop-in scenes. A pack is a directory whose name is the scene id:
//!
//! ```text
//! <root>/<id>/scene.toml
//! <root>/<id>/scene.wgsl      # kind = "shader"
//! <root>/<id>/<file>          # kind = "wallpaper"
//! ```
//!
//! Roots, first match wins: `~/.config/asciiwall/packs`, then
//! `~/.local/share/asciiwall/packs`. Built-in scene ids always win over a pack
//! with the same name. `"packs"` in the config expands to every valid pack;
//! a pack is otherwise absent from rotation until its id is listed.

use super::shader::{ShaderData, Spec};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

const LICENSES: [&str; 7] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "CC-BY-4.0",
    "Unlicense",
];
const RESERVED: [&str; 3] = ["images", "shaders", "packs"];
const MAX_SOURCE: usize = 64 * 1024;
const MAX_WALLPAPER: u64 = 25 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Shader,
    Wallpaper,
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub id: String,
    pub dir: PathBuf,
    pub kind: Kind,
    pub shaped: bool,
    pub contrast: f32,
    pub points: u32,
    pub masks: [String; 4],
    /// Multiplier already applied when the user picks Natural.
    pub pulse: f32,
    pub font_scale: f32,
    pub wallpaper: Option<PathBuf>,
    pub license: String,
    pub credit: String,
    pub author: String,
}

pub fn roots() -> Vec<PathBuf> {
    let home = crate::config::home();
    vec![
        home.join(".config/asciiwall/packs"),
        home.join(".local/share/asciiwall/packs"),
    ]
}

pub fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    (2..=32).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && !id.ends_with('-')
        && !id.contains("--")
        && !id.starts_with("image--")
        && !RESERVED.contains(&id)
        && !super::builtin_names().any(|n| n == id)
}

/// Scene source, comments removed, so a mention of the contract in a comment
/// is not mistaken for an entry point.
fn code(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            for n in chars.by_ref() {
                if n == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(n) = chars.next() {
                if n == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn validate_source(src: &str, shaped: bool, points: u32) -> Result<()> {
    if src.len() > MAX_SOURCE {
        bail!("shader source exceeds {MAX_SOURCE} bytes");
    }
    let body = code(src);
    if !body.is_ascii() {
        bail!("shader code must be ASCII; non-ASCII is allowed only in comments");
    }
    for banned in [
        "@compute",
        "@group",
        "var<storage",
        "var<uniform",
        "fn main(",
        "fn scatter(",
    ] {
        if src.contains(banned) {
            bail!("shader source must not contain `{banned}` (the prelude owns the pipeline)");
        }
    }
    let has = |sig: &str| body.contains(sig);
    if shaped {
        if !has("fn field(") {
            bail!("shaped shader must define fn field(");
        }
        if has("fn cell(") {
            bail!("shaped shader must not define fn cell( (shape.wgsl provides it)");
        }
    } else {
        if !has("fn cell(") {
            bail!("unshaped shader must define fn cell(");
        }
        if has("fn field(") {
            bail!("unshaped shader must not define fn field(");
        }
    }
    if (points > 0) != has("fn points(") {
        bail!(
            "points = {points} but fn points( is {}",
            if has("fn points(") {
                "present"
            } else {
                "absent"
            }
        );
    }
    if points > 65_536 {
        bail!("points {points} exceeds 65536");
    }
    Ok(())
}

fn required_str(table: &toml::Table, key: &str, path: &Path) -> Result<String> {
    match table.get(key).and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        _ => bail!("{}: missing {key}", path.display()),
    }
}

fn f32_key(
    table: &toml::Table,
    key: &str,
    default: f32,
    min: f32,
    max: f32,
    path: &Path,
) -> Result<f32> {
    let Some(v) = table.get(key) else {
        return Ok(default);
    };
    let n = v
        .as_float()
        .or_else(|| v.as_integer().map(|i| i as f64))
        .ok_or_else(|| anyhow::anyhow!("{}: {key} must be a number", path.display()))?;
    if !(min as f64..=max as f64).contains(&n) {
        bail!("{}: {key} must be within {min}..={max}", path.display());
    }
    Ok(n as f32)
}

/// Load and validate `dir/scene.toml`. `dir`'s name must be the scene id.
pub fn load_dir(dir: &Path) -> Result<Pack> {
    let manifest = dir.join("scene.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", manifest.display()))?;
    let table: toml::Table = toml::from_str(&text)
        .map_err(|e| anyhow::anyhow!("invalid {}: {e}", manifest.display()))?;
    let id = required_str(&table, "id", &manifest)?;
    let dirname = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if dirname != id {
        bail!(
            "{}: id {id} does not match directory {dirname}",
            manifest.display()
        );
    }
    if !valid_id(&id) {
        bail!("{}: invalid id {id}", manifest.display());
    }
    let kind = match required_str(&table, "kind", &manifest)?.as_str() {
        "shader" => Kind::Shader,
        "wallpaper" => Kind::Wallpaper,
        other => bail!("{}: unknown kind {other}", manifest.display()),
    };
    let license = required_str(&table, "license", &manifest)?;
    if !LICENSES.contains(&license.as_str()) {
        bail!(
            "{}: license {license} is not one of {}",
            manifest.display(),
            LICENSES.join(", ")
        );
    }
    let credit = required_str(&table, "credit", &manifest)?;
    let author = required_str(&table, "author", &manifest)?;
    if let Some(status) = table.get("status").and_then(|v| v.as_str())
        && !["example", "candidate", "curated"].contains(&status)
    {
        bail!(
            "{}: status must be example, candidate, or curated",
            manifest.display()
        );
    }
    let pulse = f32_key(&table, "pulse", 1.0, 0.25, 4.0, &manifest)?;
    let (shaped, contrast, points, masks, font_scale, wallpaper) = match kind {
        Kind::Shader => {
            let shaped = table
                .get("shaped")
                .and_then(|v| v.as_bool())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "{}: shader requires shaped = true|false",
                        manifest.display()
                    )
                })?;
            let contrast = f32_key(&table, "contrast", 1.0, 0.5, 3.0, &manifest)?;
            let points = table.get("points").map_or(Ok(0u32), |v| {
                v.as_integer()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| {
                        anyhow::anyhow!("{}: points must be an integer", manifest.display())
                    })
            })?;
            let masks = match table.get("masks") {
                Some(v) => {
                    let arr = v.as_array().ok_or_else(|| {
                        anyhow::anyhow!("{}: masks must be four strings", manifest.display())
                    })?;
                    if arr.len() != 4 {
                        bail!("{}: masks must be four strings", manifest.display());
                    }
                    let mut out = [String::new(), String::new(), String::new(), String::new()];
                    for (i, item) in arr.iter().enumerate() {
                        let s = item.as_str().ok_or_else(|| {
                            anyhow::anyhow!("{}: masks must be four strings", manifest.display())
                        })?;
                        if s.bytes().any(|b| !(33..127).contains(&b)) {
                            bail!(
                                "{}: masks[{i}] must be printable ASCII without space",
                                manifest.display()
                            );
                        }
                        out[i] = s.to_string();
                    }
                    out
                }
                None => [String::new(), String::new(), String::new(), String::new()],
            };
            let src_path = dir.join("scene.wgsl");
            let src = std::fs::read_to_string(&src_path)
                .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", src_path.display()))?;
            validate_source(&src, shaped, points)?;
            let font_scale = f32_key(&table, "font_scale", 1.0, 0.5, 1.5, &manifest)?;
            (shaped, contrast, points, masks, font_scale, None)
        }
        Kind::Wallpaper => {
            let file = required_str(&table, "file", &manifest)?;
            if file.contains('/') || file.contains('\\') || file == ".." || file == "." {
                bail!(
                    "{}: file must be a name inside the pack directory",
                    manifest.display()
                );
            }
            let path = dir.join(&file);
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !["jpg", "jpeg", "png", "webp"].contains(&ext.as_str()) {
                bail!(
                    "{}: wallpaper must be jpg, jpeg, png, or webp",
                    manifest.display()
                );
            }
            let meta = std::fs::metadata(&path)
                .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
            if meta.len() > MAX_WALLPAPER {
                bail!("{}: wallpaper exceeds 25 MiB", path.display());
            }
            image::open(&path)
                .map_err(|e| anyhow::anyhow!("cannot load image {}: {e}", path.display()))?;
            let font_scale = f32_key(&table, "font_scale", 0.7, 0.5, 1.5, &manifest)?;
            (
                false,
                1.0,
                0,
                [String::new(), String::new(), String::new(), String::new()],
                font_scale,
                Some(path),
            )
        }
    };
    Ok(Pack {
        id,
        dir: dir.to_path_buf(),
        kind,
        shaped,
        contrast,
        points,
        masks,
        pulse,
        font_scale,
        wallpaper,
        license,
        credit,
        author,
    })
}

pub fn find_in(id: &str, roots: &[PathBuf]) -> Result<Option<Pack>> {
    if !valid_id(id) {
        return Ok(None);
    }
    for root in roots {
        let dir = root.join(id);
        if dir.join("scene.toml").is_file() {
            return Ok(Some(load_dir(&dir)?));
        }
    }
    Ok(None)
}

pub fn find(id: &str) -> Result<Option<Pack>> {
    find_in(id, &roots())
}

pub fn ids_in(roots: &[PathBuf]) -> Vec<String> {
    let mut ids = Vec::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else {
            continue;
        };
        let mut found: Vec<String> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join("scene.toml").is_file())
            .filter_map(|p| match load_dir(&p) {
                Ok(pack) => Some(pack.id),
                Err(e) => {
                    eprintln!("asciiwall: {e:#}");
                    None
                }
            })
            .collect();
        found.sort();
        for id in found {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

pub fn ids() -> Vec<String> {
    ids_in(&roots())
}

pub fn pulse_of(id: &str) -> f32 {
    find(id).ok().flatten().map(|p| p.pulse).unwrap_or(1.0)
}

pub fn font_scale_of(id: &str) -> f32 {
    find(id).ok().flatten().map(|p| p.font_scale).unwrap_or(1.0)
}

pub fn shader_view<'a>(pack: &'a Pack, src: &'a str) -> Result<Spec<'a>> {
    if pack.kind != Kind::Shader {
        bail!("pack {} is not a shader", pack.id);
    }
    let masks = [
        pack.masks[0].as_str(),
        pack.masks[1].as_str(),
        pack.masks[2].as_str(),
        pack.masks[3].as_str(),
    ];
    Ok(Spec {
        name: &pack.id,
        src,
        shaped: pack.shaped,
        contrast: pack.contrast,
        masks,
        data: ShaderData::None,
        points: pack.points,
    })
}

pub fn read_shader(pack: &Pack) -> Result<String> {
    let path = pack.dir.join("scene.wgsl");
    std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))
}

/// Validate built-in shader sources and every discovered pack. `path`, when
/// set, is one pack directory instead of the discovered roots.
pub fn check(path: Option<&Path>) -> Result<()> {
    if let Some(dir) = path {
        let pack = load_dir(dir)?;
        println!(
            "ok {} ({}) {} by {} — {}",
            pack.id,
            if pack.kind == Kind::Shader {
                "shader"
            } else {
                "wallpaper"
            },
            pack.license,
            pack.author,
            pack.credit
        );
        return Ok(());
    }
    let mut failed = false;
    for spec in super::shader::SPECS {
        match validate_source(spec.source(), spec.shaped, spec.points) {
            Ok(()) => println!("ok {}", spec.name),
            Err(e) => {
                eprintln!("asciiwall: {}: {e:#}", spec.name);
                failed = true;
            }
        }
    }
    for id in ids() {
        match find(&id) {
            Ok(Some(_)) => println!("ok {id}"),
            Ok(None) => {
                eprintln!("asciiwall: {id}: disappeared during check");
                failed = true;
            }
            Err(e) => {
                eprintln!("asciiwall: {e:#}");
                failed = true;
            }
        }
    }
    if failed {
        bail!("pack check failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pack(dir: &Path, toml: &str, wgsl: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("scene.toml"), toml).unwrap();
        std::fs::write(dir.join("scene.wgsl"), wgsl).unwrap();
    }

    #[test]
    fn accepts_a_shaped_pack_and_rejects_a_pipeline_takeover() {
        let root = std::env::temp_dir().join(format!("asciiwall-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("drift");
        write_pack(
            &dir,
            r#"
id = "drift"
kind = "shader"
shaped = true
license = "MIT"
credit = "original"
author = "test"
pulse = 0.5
"#,
            "fn field(p: vec2<f32>) -> vec3<f32> { return vec3<f32>(0.0); }\n",
        );
        let pack = load_dir(&dir).unwrap();
        assert_eq!(pack.pulse, 0.5);
        assert!(find_in("drift", &[root.clone()]).unwrap().is_some());
        assert!(find_in("waves", &[root.clone()]).unwrap().is_none());
        std::fs::write(
            dir.join("scene.wgsl"),
            "fn field(p: vec2<f32>) -> vec3<f32> {}\n@compute fn main() {}\n",
        )
        .unwrap();
        assert!(load_dir(&dir).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shipped_example_pack_checks() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/packs/drift");
        let pack = load_dir(&dir).unwrap();
        assert_eq!(pack.id, "drift");
        assert_eq!(pack.kind, Kind::Shader);
    }

    #[test]
    fn accepts_a_wallpaper_and_rejects_a_path_escape() {
        let root = std::env::temp_dir().join(format!("asciiwall-wall-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("harbor");
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbImage::new(2, 2)
            .save(dir.join("harbor.png"))
            .unwrap();
        std::fs::write(
            dir.join("scene.toml"),
            r#"
id = "harbor"
kind = "wallpaper"
file = "harbor.png"
license = "CC-BY-4.0"
credit = "original photograph"
author = "test"
"#,
        )
        .unwrap();
        let pack = load_dir(&dir).unwrap();
        assert_eq!(pack.font_scale, 0.7);
        assert!(pack.wallpaper.unwrap().ends_with("harbor.png"));
        std::fs::write(
            dir.join("scene.toml"),
            r#"
id = "harbor"
kind = "wallpaper"
file = "../harbor.png"
license = "MIT"
credit = "original"
author = "test"
"#,
        )
        .unwrap();
        assert!(load_dir(&dir).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shipped_shaders_match_the_pack_contract() {
        for spec in super::super::shader::SPECS {
            validate_source(spec.source(), spec.shaped, spec.points)
                .unwrap_or_else(|e| panic!("{}: {e}", spec.name));
        }
    }
}
