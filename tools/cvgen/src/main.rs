//! Content generator: the single source of truth lives in `content/*.md`.
//!
//! This tool parses that markdown (YAML frontmatter + markdown bullet lists)
//! and emits two artifacts so the website and the CV can never drift:
//!
//!   * `jscarrott/src/generated_content.rs` — data the terminal site renders.
//!   * `cv.typ`                             — the Typst CV.
//!
//! Run `cargo run --manifest-path tools/cvgen/Cargo.toml` to regenerate, or
//! pass `--check` to verify the committed artifacts are up to date (used in CI).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

struct Entry {
    title: String,
    org: String,
    location: String,
    date: String,
    emoji: String,
    accent: String,
    bullets: Vec<Bullet>,
}

struct Bullet {
    lead: String,
    rest: String,
}

struct Skill {
    name: String,
    items: String,
}

// ----------------------------------------------------------------------------
// Parsing
// ----------------------------------------------------------------------------

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Split a `---` YAML frontmatter block (flat `key: value` pairs) from the body.
fn split_frontmatter(text: &str) -> (Vec<(String, String)>, String) {
    if !text.starts_with("---") {
        return (Vec::new(), text.to_string());
    }
    let mut fm = Vec::new();
    let mut body = String::new();
    let mut lines = text.lines();
    let _ = lines.next(); // opening ---
    let mut in_fm = true;
    for line in lines {
        if in_fm {
            if line.trim() == "---" {
                in_fm = false;
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                fm.push((k.trim().to_string(), unquote(v)));
            }
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    (fm, body)
}

fn field(fm: &[(String, String)], key: &str) -> String {
    fm.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// A bullet may begin with a bold lead: `- **Lead text:** the rest`.
fn parse_bullet(s: &str) -> Bullet {
    if let Some(after) = s.strip_prefix("**") {
        if let Some(idx) = after.find("**") {
            return Bullet {
                lead: after[..idx].to_string(),
                rest: after[idx + 2..].trim_start().to_string(),
            };
        }
    }
    Bullet {
        lead: String::new(),
        rest: s.to_string(),
    }
}

fn body_bullets(body: &str) -> Vec<Bullet> {
    body.lines()
        .map(str::trim_start)
        .filter_map(|l| l.strip_prefix("- "))
        .map(|l| parse_bullet(l.trim()))
        .collect()
}

fn parse_entry(text: &str) -> Entry {
    let (fm, body) = split_frontmatter(text);
    Entry {
        title: field(&fm, "title"),
        org: field(&fm, "org"),
        location: field(&fm, "location"),
        date: field(&fm, "date"),
        emoji: field(&fm, "emoji"),
        accent: field(&fm, "accent"),
        bullets: body_bullets(&body),
    }
}

fn parse_entries(dir: &Path) -> Vec<Entry> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |x| x == "md"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| parse_entry(&fs::read_to_string(p).unwrap()))
        .collect()
}

fn parse_skills(text: &str) -> Vec<Skill> {
    let (_, body) = split_frontmatter(text);
    body.lines()
        .map(str::trim_start)
        .filter_map(|l| l.strip_prefix("- "))
        .filter_map(|l| l.trim().split_once(": "))
        .map(|(n, i)| Skill {
            name: n.trim().to_string(),
            items: i.trim().to_string(),
        })
        .collect()
}

fn parse_about(text: &str) -> Vec<String> {
    let (_, body) = split_frontmatter(text);
    let mut lines: Vec<String> = body.lines().map(str::to_string).collect();
    while lines.last().map_or(false, |l| l.trim().is_empty()) {
        lines.pop();
    }
    lines
}

// ----------------------------------------------------------------------------
// Rust emission (site content)
// ----------------------------------------------------------------------------

/// Escape for a Rust double-quoted string literal.
fn rs(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn emit_entries_rust(o: &mut String, name: &str, entries: &[Entry]) {
    o.push_str(&format!("pub const {name}: &[Entry] = &[\n"));
    for e in entries {
        o.push_str("    Entry {\n");
        o.push_str(&format!("        title: \"{}\",\n", rs(&e.title)));
        o.push_str(&format!("        org: \"{}\",\n", rs(&e.org)));
        o.push_str(&format!("        location: \"{}\",\n", rs(&e.location)));
        o.push_str(&format!("        date: \"{}\",\n", rs(&e.date)));
        o.push_str(&format!("        emoji: \"{}\",\n", rs(&e.emoji)));
        o.push_str(&format!("        accent: \"{}\",\n", rs(&e.accent)));
        o.push_str("        bullets: &[\n");
        for b in &e.bullets {
            o.push_str(&format!(
                "            Bullet {{ lead: \"{}\", rest: \"{}\" }},\n",
                rs(&b.lead),
                rs(&b.rest)
            ));
        }
        o.push_str("        ],\n    },\n");
    }
    o.push_str("];\n\n");
}

fn emit_rust(
    profile: &[(String, String)],
    about: &[String],
    skills: &[Skill],
    experience: &[Entry],
    education: &[Entry],
    projects: &[Entry],
) -> String {
    let name = format!(
        "{} {}",
        field(profile, "first_name"),
        field(profile, "last_name")
    );
    let mut o = String::new();
    o.push_str("// @generated by tools/cvgen from content/. DO NOT EDIT BY HAND.\n");
    o.push_str("// Regenerate with: cargo run --manifest-path tools/cvgen/Cargo.toml\n\n");
    o.push_str("#![allow(dead_code)]\n\n");
    o.push_str("pub struct Profile {\n    pub name: &'static str,\n    pub position: &'static str,\n    pub email: &'static str,\n    pub phone: &'static str,\n    pub homepage: &'static str,\n    pub github: &'static str,\n    pub location: &'static str,\n}\n\n");
    o.push_str("pub struct Bullet {\n    pub lead: &'static str,\n    pub rest: &'static str,\n}\n\n");
    o.push_str("pub struct Entry {\n    pub title: &'static str,\n    pub org: &'static str,\n    pub location: &'static str,\n    pub date: &'static str,\n    pub emoji: &'static str,\n    pub accent: &'static str,\n    pub bullets: &'static [Bullet],\n}\n\n");
    o.push_str("pub struct SkillCategory {\n    pub name: &'static str,\n    pub items: &'static str,\n}\n\n");

    o.push_str(&format!(
        "pub const PROFILE: Profile = Profile {{\n    name: \"{}\",\n    position: \"{}\",\n    email: \"{}\",\n    phone: \"{}\",\n    homepage: \"{}\",\n    github: \"github.com/{}\",\n    location: \"{}\",\n}};\n\n",
        rs(&name),
        rs(&field(profile, "site_position")),
        rs(&field(profile, "email")),
        rs(&field(profile, "phone")),
        rs(&field(profile, "homepage")),
        rs(&field(profile, "github")),
        rs(&field(profile, "location")),
    ));

    o.push_str("pub const ABOUT: &[&str] = &[\n");
    for l in about {
        o.push_str(&format!("    \"{}\",\n", rs(l)));
    }
    o.push_str("];\n\n");

    o.push_str("pub const SKILLS: &[SkillCategory] = &[\n");
    for s in skills {
        o.push_str(&format!(
            "    SkillCategory {{ name: \"{}\", items: \"{}\" }},\n",
            rs(&s.name),
            rs(&s.items)
        ));
    }
    o.push_str("];\n\n");

    emit_entries_rust(&mut o, "EXPERIENCE", experience);
    emit_entries_rust(&mut o, "EDUCATION", education);
    emit_entries_rust(&mut o, "PROJECTS", projects);
    o
}

// ----------------------------------------------------------------------------
// Typst emission (CV)
// ----------------------------------------------------------------------------

/// Escape for a Typst double-quoted string literal. Content is passed to Typst
/// as *strings* (never raw markup), so markup characters like `#`, `*`, `_`
/// render literally and need no escaping — only `\` and `"`.
fn ts(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

const TYPST_HELPERS: &str = r###"#set page(paper: "a4", margin: (left: 1.4cm, right: 1.4cm, top: 1.2cm, bottom: 1.6cm))
#set text(font: "Liberation Sans", size: 9.5pt, fill: rgb("#333333"))
#set par(justify: true, leading: 0.62em)

#let accent = rgb("#27ae60")
#let muted = rgb("#5d5d5d")

#let section(title) = {
  v(8pt)
  text(size: 13pt, weight: "bold", fill: accent, smallcaps(title))
  v(1pt)
  line(length: 100%, stroke: 0.6pt + accent)
  v(3pt)
}

#let skill(name, items) = {
  grid(columns: (33%, 1fr), gutter: 10pt, text(weight: "bold", hyphenate: false, name), text(items))
  v(2pt)
}

#let item(lead, rest) = {
  if lead == "" { text(fill: muted, rest) } else { text(fill: muted, strong(lead) + " " + rest) }
}

#let entry(title, org, location, date, body) = {
  block(below: 8pt, breakable: false, {
    grid(columns: (1fr, auto), text(weight: "bold", title) + text(", " + org), text(fill: muted, date))
    text(fill: muted, style: "italic", size: 9pt, location)
    body
  })
}
"###;

fn emit_typst_entries(o: &mut String, entries: &[Entry]) {
    for e in entries {
        o.push_str(&format!(
            "#entry(\"{}\", \"{}\", \"{}\", \"{}\", list(\n",
            ts(&e.title),
            ts(&e.org),
            ts(&e.location),
            ts(&e.date)
        ));
        for b in &e.bullets {
            o.push_str(&format!("  item(\"{}\", \"{}\"),\n", ts(&b.lead), ts(&b.rest)));
        }
        o.push_str("))\n\n");
    }
}

fn emit_typst(
    profile: &[(String, String)],
    skills: &[Skill],
    experience: &[Entry],
    education: &[Entry],
    projects: &[Entry],
) -> String {
    let first = field(profile, "first_name");
    let last = field(profile, "last_name");
    let contact = format!(
        "{}  |  {}  |  {}  |  github.com/{}  |  {}",
        field(profile, "email"),
        field(profile, "phone"),
        field(profile, "homepage"),
        field(profile, "github"),
        field(profile, "location"),
    );

    let mut o = String::new();
    o.push_str("// @generated by tools/cvgen from content/. DO NOT EDIT BY HAND.\n");
    o.push_str("// Regenerate with: cargo run --manifest-path tools/cvgen/Cargo.toml\n");
    o.push_str("// Compile with:    typst compile cv.typ\n");
    o.push_str(&format!(
        "#set document(title: \"{} {} — Curriculum Vitae\", author: \"{} {}\")\n",
        ts(&first),
        ts(&last),
        ts(&first),
        ts(&last)
    ));
    o.push_str(TYPST_HELPERS);
    o.push('\n');
    o.push_str(&format!(
        "#align(center)[\n  #text(size: 26pt)[{} #text(fill: accent, weight: \"bold\")[{}]]\n  #linebreak()\n  #v(2pt)\n  #text(size: 11pt, fill: muted, \"{}\")\n  #v(3pt)\n  #text(size: 8.5pt, \"{}\")\n]\n#v(6pt)\n\n",
        first,
        last,
        ts(&field(profile, "cv_position")),
        ts(&contact)
    ));

    o.push_str("#section(\"Skills\")\n");
    for s in skills {
        o.push_str(&format!(
            "#skill(\"{}\", \"{}\")\n",
            ts(&s.name),
            ts(&s.items)
        ));
    }
    o.push('\n');

    o.push_str("#section(\"Experience\")\n");
    emit_typst_entries(&mut o, experience);

    o.push_str("#section(\"Extracurricular Activity\")\n");
    emit_typst_entries(&mut o, projects);

    o.push_str("#section(\"Education\")\n");
    emit_typst_entries(&mut o, education);

    o
}

// ----------------------------------------------------------------------------
// Driver
// ----------------------------------------------------------------------------

fn parse_profile(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), unquote(v)))
        .collect()
}

fn main() {
    let check = std::env::args().any(|a| a == "--check");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf();
    let content = root.join("content");

    let profile = parse_profile(&fs::read_to_string(content.join("profile.yaml")).unwrap());
    let about = parse_about(&fs::read_to_string(content.join("about.md")).unwrap());
    let skills = parse_skills(&fs::read_to_string(content.join("skills.md")).unwrap());
    let experience = parse_entries(&content.join("experience"));
    let education = parse_entries(&content.join("education"));
    let projects = parse_entries(&content.join("projects"));

    let outputs = [
        (
            root.join("jscarrott/src/generated_content.rs"),
            emit_rust(&profile, &about, &skills, &experience, &education, &projects),
        ),
        (
            root.join("cv.typ"),
            emit_typst(&profile, &skills, &experience, &education, &projects),
        ),
    ];

    let mut drift = false;
    for (path, generated) in &outputs {
        if check {
            let current = fs::read_to_string(path).unwrap_or_default();
            if &current != generated {
                eprintln!("✗ out of date: {}", path.display());
                drift = true;
            } else {
                println!("✓ up to date: {}", path.display());
            }
        } else {
            fs::write(path, generated).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
            println!("wrote {}", path.display());
        }
    }

    if check && drift {
        eprintln!(
            "\nGenerated files are stale. Run `cargo run --manifest-path tools/cvgen/Cargo.toml` and commit the result."
        );
        exit(1);
    }
}
