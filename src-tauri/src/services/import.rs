use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ManuscriptChunk {
    pub index: usize,
    pub text: String,
    pub chapter: Option<String>,
    pub scene: Option<String>,
    pub word_count: usize,
}
pub fn chunk_text(text: &str, target_words: usize, overlap_words: usize) -> Vec<ManuscriptChunk> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while start < words.len() {
        let end = (start + target_words).min(words.len());
        let body = words[start..end].join(" ");
        chunks.push(ManuscriptChunk {
            index,
            text: body,
            chapter: None,
            scene: None,
            word_count: end - start,
        });
        if end == words.len() {
            break;
        }
        start = end.saturating_sub(overlap_words);
        index += 1;
    }
    chunks
}
