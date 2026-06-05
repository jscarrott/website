//! Canonical profile data for the site.
//!
//! This is the single source of truth for John's identity and contact details.
//! The terminal UI in `main.rs` renders its header from here rather than from
//! hardcoded strings, and `scripts/check-cv-sync.sh` cross-checks that the
//! LaTeX CV at the repo root (`cv.tex`) carries the same email and phone — so
//! the website and the CV can't silently drift apart again.
//!
//! When any of these details change, edit them *here* and re-run the CV sync
//! check (or update `cv.tex` to match).

/// Identity and contact details shown in the welcome header.
pub struct Profile {
    pub name: &'static str,
    pub position: &'static str,
    pub email: &'static str,
    pub phone: &'static str,
    pub homepage: &'static str,
    pub github: &'static str,
    pub location: &'static str,
}

pub const PROFILE: Profile = Profile {
    name: "John Scarrott",
    position: "Senior Software Engineer • Systems Specialist",
    email: "john@scarrotts.uk",
    phone: "(+44) 7733298950",
    homepage: "www.jscarrott.com",
    github: "github.com/jscarrott",
    location: "Barnstaple, Devon, UK",
};
