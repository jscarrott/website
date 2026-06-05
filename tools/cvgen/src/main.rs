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

// ----------------------------------------------------------------------------
// HTML emission (accessible, no-JS / low-power fallback site + view toggle)
// ----------------------------------------------------------------------------

/// Escape text for HTML element content.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn html_entries(out: &mut String, title: &str, entries: &[Entry]) {
    out.push_str(&format!("      <section>\n        <h2>{}</h2>\n", esc(title)));
    for e in entries {
        out.push_str("        <article class=\"entry\">\n");
        out.push_str(&format!(
            "          <h3>{} {}<span class=\"org\"> — {}</span></h3>\n",
            esc(&e.emoji),
            esc(&e.title),
            esc(&e.org)
        ));
        out.push_str(&format!(
            "          <p class=\"meta\">{} · {}</p>\n",
            esc(&e.date),
            esc(&e.location)
        ));
        out.push_str("          <ul>\n");
        for b in &e.bullets {
            if b.lead.is_empty() {
                out.push_str(&format!("            <li>{}</li>\n", esc(&b.rest)));
            } else {
                out.push_str(&format!(
                    "            <li><strong>{}</strong> {}</li>\n",
                    esc(&b.lead),
                    esc(&b.rest)
                ));
            }
        }
        out.push_str("          </ul>\n        </article>\n");
    }
    out.push_str("      </section>\n");
}

fn html_about(out: &mut String, about: &[String]) {
    out.push_str("      <section>\n        <h2>About</h2>\n");
    let mut para: Vec<&str> = Vec::new();
    let mut list_open = false;
    for line in about {
        let t = line.trim();
        if let Some(item) = t.strip_prefix('•') {
            if !para.is_empty() {
                out.push_str(&format!("        <p>{}</p>\n", esc(&para.join(" "))));
                para.clear();
            }
            if !list_open {
                out.push_str("        <ul>\n");
                list_open = true;
            }
            out.push_str(&format!("          <li>{}</li>\n", esc(item.trim())));
        } else if t.is_empty() {
            if !para.is_empty() {
                out.push_str(&format!("        <p>{}</p>\n", esc(&para.join(" "))));
                para.clear();
            }
            if list_open {
                out.push_str("        </ul>\n");
                list_open = false;
            }
        } else {
            if list_open {
                out.push_str("        </ul>\n");
                list_open = false;
            }
            para.push(t);
        }
    }
    if !para.is_empty() {
        out.push_str(&format!("        <p>{}</p>\n", esc(&para.join(" "))));
    }
    if list_open {
        out.push_str("        </ul>\n");
    }
    out.push_str("      </section>\n");
}

fn html_skills(out: &mut String, skills: &[Skill]) {
    out.push_str("      <section>\n        <h2>Skills</h2>\n        <dl class=\"skills\">\n");
    for s in skills {
        out.push_str(&format!(
            "          <dt>{}</dt>\n          <dd>{}</dd>\n",
            esc(&s.name),
            esc(&s.items)
        ));
    }
    out.push_str("        </dl>\n      </section>\n");
}

fn emit_cv_html(
    profile: &[(String, String)],
    about: &[String],
    skills: &[Skill],
    experience: &[Entry],
    education: &[Entry],
    projects: &[Entry],
) -> String {
    let first = field(profile, "first_name");
    let last = field(profile, "last_name");
    let email = field(profile, "email");
    let phone = field(profile, "phone");
    let homepage = field(profile, "homepage");
    let github = field(profile, "github");
    let location = field(profile, "location");

    let mut o = String::new();
    o.push_str("      <header class=\"cv-header\">\n");
    o.push_str(&format!(
        "        <h1>{} <span class=\"accent\">{}</span></h1>\n",
        esc(&first),
        esc(&last)
    ));
    o.push_str(&format!(
        "        <p class=\"tagline\">{}</p>\n",
        esc(&field(profile, "cv_position"))
    ));
    o.push_str("        <p class=\"contact\">\n");
    o.push_str(&format!(
        "          <a href=\"mailto:{}\">{}</a>\n",
        esc(&email),
        esc(&email)
    ));
    o.push_str(&format!("          <span>{}</span>\n", esc(&phone)));
    o.push_str(&format!(
        "          <a href=\"https://{}\">{}</a>\n",
        esc(&homepage),
        esc(&homepage)
    ));
    o.push_str(&format!(
        "          <a href=\"https://github.com/{}\">github.com/{}</a>\n",
        esc(&github),
        esc(&github)
    ));
    o.push_str(&format!("          <span>{}</span>\n", esc(&location)));
    o.push_str("        </p>\n      </header>\n");

    html_about(&mut o, about);
    html_entries(&mut o, "Experience", experience);
    html_skills(&mut o, skills);
    html_entries(&mut o, "Projects", projects);
    html_entries(&mut o, "Education", education);
    o
}

const PAGE_CSS: &str = r####"
:root {
  --nord0:#2e3440; --nord3:#4c566a; --nord4:#d8dee9; --nord6:#eceff4;
  --frost:#88c0d0; --teal:#8fbcbb; --yellow:#ebcb8b;
}
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; }
body {
  background: var(--nord0); color: var(--nord6);
  font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; line-height: 1.55;
}
/* view switching, driven by <html data-view> */
html[data-view="terminal"] #cv { display: none; }
html[data-view="terminal"] body { height: 100vh; overflow: hidden; display: flex; align-items: center; justify-content: center; }
html[data-view="plain"] #terminal-root { display: none; }
/* toggle button */
#view-toggle {
  position: fixed; top: 12px; right: 12px; z-index: 1000;
  font-family: inherit; font-size: 13px; cursor: pointer; color: var(--nord0);
  background: var(--frost); border: none; padding: 8px 14px; border-radius: 6px;
  box-shadow: 0 2px 8px rgba(0,0,0,.35);
}
#view-toggle:hover { background: var(--teal); }
/* plain CV */
#cv { max-width: 820px; margin: 0 auto; padding: 56px 24px 80px; }
.cv-header h1 { font-size: 2.4rem; margin: 0 0 4px; font-weight: 700; }
.cv-header .accent { color: var(--frost); }
.tagline { color: var(--yellow); margin: 0 0 12px; font-size: 1.05rem; }
.contact { color: var(--nord4); font-size: .9rem; margin: 0; display: flex; flex-wrap: wrap; gap: 4px 10px; }
.contact a { color: var(--frost); text-decoration: none; }
.contact a:hover { text-decoration: underline; }
#cv section { margin-top: 36px; }
#cv h2 { color: var(--frost); font-size: 1.3rem; border-bottom: 1px solid var(--nord3); padding-bottom: 6px; margin: 0 0 16px; }
.entry { margin-bottom: 22px; }
.entry h3 { margin: 0; font-size: 1.05rem; }
.entry .org { color: var(--nord4); font-weight: 400; }
.entry .meta { color: var(--nord3); font-size: .85rem; margin: 2px 0 8px; }
#cv ul { margin: 0; padding-left: 22px; }
#cv li { margin-bottom: 6px; }
#cv li strong { color: var(--yellow); font-weight: 600; }
dl.skills { display: grid; grid-template-columns: max-content 1fr; gap: 6px 18px; margin: 0; }
dl.skills dt { color: var(--frost); font-weight: 600; }
dl.skills dd { margin: 0; }
@media (max-width: 560px) {
  dl.skills { grid-template-columns: 1fr; gap: 2px 0; }
  dl.skills dd { margin-bottom: 8px; }
  #cv { padding: 56px 18px 64px; }
  .cv-header h1 { font-size: 2rem; }
}
"####;

fn emit_index_html(
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
    let position = field(profile, "cv_position");
    let cv_body = emit_cv_html(profile, about, skills, experience, education, projects);

    let mut o = String::new();
    o.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    o.push_str("  <meta charset=\"UTF-8\" />\n");
    o.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n");
    o.push_str(&format!("  <title>{} — {}</title>\n", esc(&name), esc(&position)));
    o.push_str(&format!(
        "  <meta name=\"description\" content=\"{} — {}. Interactive terminal CV with a plain-text view.\" />\n",
        esc(&name),
        esc(&position)
    ));
    o.push_str("  <link rel=\"stylesheet\" href=\"https://cdnjs.cloudflare.com/ajax/libs/firacode/6.2.0/fira_code.min.css\" />\n");
    o.push_str("  <style>");
    o.push_str(PAGE_CSS);
    o.push_str("  </style>\n");
    // Decide the initial view before first paint (avoids a flash of the wrong one).
    o.push_str("  <script>\n");
    o.push_str("    (function () {\n");
    o.push_str("      var v;\n");
    o.push_str("      try { v = localStorage.getItem('cvView'); } catch (e) {}\n");
    o.push_str("      if (v !== 'plain' && v !== 'terminal') {\n");
    o.push_str("        var coarse = (window.matchMedia && matchMedia('(pointer: coarse)').matches) || window.innerWidth < 700;\n");
    o.push_str("        v = coarse ? 'plain' : 'terminal';\n");
    o.push_str("      }\n");
    o.push_str("      document.documentElement.setAttribute('data-view', v);\n");
    o.push_str("    })();\n");
    o.push_str("  </script>\n");
    o.push_str("</head>\n<body>\n");
    o.push_str("  <button id=\"view-toggle\" type=\"button\">Plain view</button>\n");
    o.push_str("  <main id=\"cv\">\n");
    o.push_str(&cv_body);
    o.push_str("  </main>\n");
    o.push_str("  <div id=\"terminal-root\"></div>\n");
    o.push_str("  <script>\n");
    o.push_str("    (function () {\n");
    o.push_str("      var btn = document.getElementById('view-toggle');\n");
    o.push_str("      var v = document.documentElement.getAttribute('data-view');\n");
    o.push_str("      btn.textContent = v === 'terminal' ? 'Plain view' : 'Terminal view';\n");
    o.push_str("      btn.addEventListener('click', function () {\n");
    o.push_str("        var cur = document.documentElement.getAttribute('data-view');\n");
    o.push_str("        var next = cur === 'terminal' ? 'plain' : 'terminal';\n");
    o.push_str("        try { localStorage.setItem('cvView', next); } catch (e) {}\n");
    o.push_str("        location.reload();\n");
    o.push_str("      });\n");
    o.push_str("    })();\n");
    o.push_str("  </script>\n");
    o.push_str("</body>\n</html>\n");
    o
}

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
        (
            root.join("jscarrott/index.html"),
            emit_index_html(&profile, &about, &skills, &experience, &education, &projects),
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
