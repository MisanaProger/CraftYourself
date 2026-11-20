use std::path::PathBuf;

use est_core::{ProjectId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::language::LanguageDetection;

/// A connected Git repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    /// Upstream URL, if known.
    pub repository_url: Option<String>,
    /// Working tree EST reads from. For local projects this is the user's
    /// own checkout; for remote ones a clone in the EST cache.
    pub local_path: PathBuf,
    /// Helper attribute only, never a restriction.
    pub primary_language: Option<String>,
    pub languages: Vec<LanguageDetection>,
    /// SPDX expression, if detected.
    pub license: Option<String>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub repository_url: Option<String>,
    pub local_path: PathBuf,
    pub languages: Vec<LanguageDetection>,
    pub license: Option<String>,
}

impl NewProject {
    /// The language with the most files.
    pub fn primary_language(&self) -> Option<String> {
        self.languages
            .iter()
            .max_by_key(|l| l.files)
            .map(|l| l.language.clone())
    }
}
