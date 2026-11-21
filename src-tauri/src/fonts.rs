//! The installed fonts Ride can use for APL: those with a glyph for every
//! character the page names (the language bar's), all of one width.

use fontdb::{Database, Style, Weight};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AplFont {
    family: String,
    /// The names CSS local() matches. WebView2 matches only these, not the
    /// family name.
    full_name: String,
    postscript: String,
}

/// The face's English full name (name ID 4), else its first readable one.
fn full_name(face: &ttf_parser::Face) -> Option<String> {
    let names: Vec<_> = face
        .names()
        .into_iter()
        .filter(|n| n.name_id == ttf_parser::name_id::FULL_NAME)
        .filter_map(|n| Some((n.language(), n.to_string()?)))
        .collect();
    names
        .iter()
        .find(|(l, _)| *l == ttf_parser::Language::English_UnitedStates)
        .or_else(|| names.first())
        .map(|(_, s)| s.clone())
}

/// Every character has a glyph, and the glyphs, with the ASCII digits,
/// share one advance width.
fn fits(face: &ttf_parser::Face, chars: &[char]) -> bool {
    let mut width = None;
    chars.iter().chain(['0', '9', 'M', 'i', ' '].iter()).all(|&c| {
        let Some(g) = face.glyph_index(c) else { return false };
        let Some(w) = face.glyph_hor_advance(g) else { return false };
        *width.get_or_insert(w) == w
    })
}

/// How far a face is from regular weight, upright: the face a family is
/// listed by.
fn distance(f: &fontdb::FaceInfo) -> (bool, u16) {
    (f.style != Style::Normal, f.weight.0.abs_diff(Weight::NORMAL.0))
}

fn scan(glyphs: &str) -> Vec<AplFont> {
    let chars: Vec<char> = glyphs.chars().filter(|c| !c.is_whitespace()).collect();
    let mut db = Database::new();
    db.load_system_fonts();
    let mut best: BTreeMap<String, (&fontdb::FaceInfo, AplFont)> = BTreeMap::new();
    for info in db.faces() {
        let Some((family, _)) = info.families.first() else { continue };
        if best.get(family).is_some_and(|(b, _)| distance(b) <= distance(info)) {
            continue;
        }
        let found = db
            .with_face_data(info.id, |data, index| {
                let face = ttf_parser::Face::parse(data, index).ok()?;
                fits(&face, &chars).then(|| full_name(&face)).flatten()
            })
            .flatten();
        if let Some(full_name) = found {
            let font = AplFont { family: family.clone(), full_name, postscript: info.post_script_name.clone() };
            best.insert(family.clone(), (info, font));
        }
    }
    let mut fonts: Vec<AplFont> = best.into_values().map(|(_, f)| f).collect();
    fonts.sort_by_key(|f| f.family.to_lowercase());
    fonts
}

/// The fonts, by family, that have a glyph for each of `glyphs` in one
/// width. Reading every installed font takes a moment, so it runs off the
/// main thread.
#[tauri::command]
pub async fn list_apl_fonts(glyphs: String) -> Result<Vec<AplFont>, String> {
    tauri::async_runtime::spawn_blocking(move || scan(&glyphs)).await.map_err(|e| e.to_string())
}
