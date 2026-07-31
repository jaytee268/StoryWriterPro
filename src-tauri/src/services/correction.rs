use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum CorrectionKind {
    Spelling,
    Grammar,
    Punctuation,
    Capitalization,
    Whitespace,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Correction {
    pub kind: CorrectionKind,
    pub from: String,
    pub to: String,
    pub reason: String,
    pub start: usize,
    pub end: usize,
}
pub const DISALLOWED_CORRECTIONS: [&str; 5] = [
    "style_rewrite",
    "sentence_reordering",
    "content_addition",
    "content_removal",
    "synonym_replacement",
];
