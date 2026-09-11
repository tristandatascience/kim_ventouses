//! Petit RAG léger : base de connaissance Markdown indexée en BM25.
//!
//! Volontairement sans embeddings : aucune dépendance externe, aucun modèle
//! d'embedding requis, mémoire quasi nulle — adapté à un VPS 1 vCore / 1 Go.
//! L'interface `search`/`format_context` pourra être branchée plus tard sur
//! le vector store de Rig (rig::vector_store) si l'infrastructure le permet.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub struct Chunk {
    pub title: String,
    pub text: String,
    tf: HashMap<String, usize>,
    len: usize,
}

pub struct RagIndex {
    chunks: Vec<Chunk>,
    df: HashMap<String, usize>,
    avgdl: f64,
    k1: f64,
    b: f64,
}

impl RagIndex {
    pub fn load(dir: &str) -> anyhow::Result<Self> {
        let dir = Path::new(dir);
        anyhow::ensure!(dir.is_dir(), "répertoire de connaissance introuvable : {dir:?}");

        let mut raw: Vec<(String, String)> = Vec::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("doc")
                .to_string();
            let content = fs::read_to_string(&path)?;
            split_sections(&stem, &content, &mut raw);
        }
        anyhow::ensure!(!raw.is_empty(), "aucun .md trouvé dans {dir:?}");

        let mut chunks = Vec::with_capacity(raw.len());
        let mut df: HashMap<String, usize> = HashMap::new();
        let mut total_len = 0usize;

        for (title, text) in raw {
            let tokens = tokenize(&format!("{title} {text}"));
            let len = tokens.len();
            total_len += len;
            let mut tf: HashMap<String, usize> = HashMap::new();
            let mut seen: HashSet<String> = HashSet::new();
            for t in &tokens {
                *tf.entry(t.clone()).or_insert(0usize) += 1;
                seen.insert(t.clone());
            }
            for t in seen {
                *df.entry(t).or_insert(0usize) += 1;
            }
            chunks.push(Chunk { title, text, tf, len });
        }

        let avgdl = total_len as f64 / chunks.len() as f64;
        tracing::info!(chunks = chunks.len(), "index BM25 construit");
        Ok(Self {
            chunks,
            df,
            avgdl,
            k1: 1.5,
            b: 0.75,
        })
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Recherche BM25 des k passages les plus pertinents pour la requête.
    pub fn search(&self, query: &str, k: usize) -> Vec<&Chunk> {
        let qtokens = tokenize(query);
        if qtokens.is_empty() {
            return Vec::new();
        }
        let n = self.chunks.len() as f64;
        let mut scored: Vec<(f64, usize)> = self
            .chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut score = 0.0f64;
                for qt in &qtokens {
                    let tf = c.tf.get(qt).copied().unwrap_or(0) as f64;
                    if tf == 0.0 {
                        continue;
                    }
                    let df = *self.df.get(qt).unwrap_or(&0) as f64;
                    let idf = (((n - df + 0.5) / (df + 0.5)) + 1.0).ln();
                    let denom =
                        tf + self.k1 * (1.0 - self.b + self.b * (c.len as f64) / self.avgdl);
                    score += idf * (tf * (self.k1 + 1.0)) / denom;
                }
                (score, i)
            })
            .filter(|(s, _)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(k)
            .map(|(_, i)| &self.chunks[i])
            .collect()
    }

    /// Contexte RAG à injecter dans le system prompt (None si rien de pertinent).
    pub fn format_context(&self, query: &str, k: usize) -> Option<String> {
        let hits = self.search(query, k);
        if hits.is_empty() {
            return None;
        }
        let mut out = String::from(
            "\n\n--- Extraits de la base de connaissance du cabinet \
             (utilise-les quand ils sont pertinents, cite les informations exactes) :\n",
        );
        for h in hits {
            out.push_str("\n[");
            out.push_str(&h.title);
            out.push_str("]\n");
            out.push_str(&h.text);
            out.push_str("\n");
        }
        Some(out)
    }
}

/// Découpe un fichier markdown en sections sur les titres `## `.
fn split_sections(file: &str, content: &str, out: &mut Vec<(String, String)>) {
    let mut title = file.to_string();
    let mut buf = String::new();
    for line in content.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            if !buf.trim().is_empty() {
                out.push((title.clone(), buf.trim().to_string()));
            }
            title = format!("{file} — {}", h.trim());
            buf.clear();
        } else {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if !buf.trim().is_empty() {
        out.push((title, buf.trim().to_string()));
    }
}

const STOPWORDS: &[&str] = &[
    // français
    "le", "la", "les", "un", "une", "des", "du", "de", "et", "ou", "au", "aux", "en", "dans",
    "pour", "par", "sur", "que", "qui", "quoi", "est", "sont", "ce", "cet", "cette", "ces",
    "mon", "ma", "mes", "votre", "vos", "notre", "nos", "nous", "vous", "je", "tu", "il",
    "elle", "on", "ils", "elles", "se", "sa", "son", "ses", "leur", "leurs", "plus", "ne",
    "pas", "comme", "avec", "sans", "cest", "jai", "est-ce", "quel", "quelle", "quels",
    "quelles", "combien", "pourquoi",
    // anglais
    "the", "a", "an", "of", "and", "or", "to", "in", "on", "for", "by", "with", "without",
    "is", "are", "be", "do", "does", "what", "which", "how", "why", "who", "can", "could",
    "will", "would", "should", "i", "you", "we", "they", "it", "my", "your", "our", "their",
    "this", "that", "these", "those", "me", "please", "how", "much", "many", "there",
];

/// Tokenisation : minuscules, suppression des accents français, découpe
/// alphanumérique, rejet des mots vides. Zéro dépendance externe.
fn tokenize(text: &str) -> Vec<String> {
    let lowered = text.to_lowercase();
    let folded: String = lowered
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' | 'æ' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'ù' | 'û' | 'ü' => 'u',
            'ô' | 'ö' => 'o',
            'î' | 'ï' => 'i',
            'ç' => 'c',
            'ÿ' => 'y',
            'œ' => 'e',
            other => other,
        })
        .collect();
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 1 && !STOPWORDS.contains(t))
        .map(|t| t.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenization_plie() {
        assert_eq!(
            tokenize("Est-ce que les ventouses font MAL ?"),
            vec!["ventouses".to_string(), "font".to_string(), "mal".to_string()]
        );
    }

    #[test]
    fn recherche_trouve_la_bonne_section() {
        let dir = std::env::temp_dir().join("soma-rag-test");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("a.md"),
            "## Douleur\nLa séance est indolore, sensation de pression inverse ferme.\n",
        )
        .unwrap();
        fs::write(
            dir.join("b.md"),
            "## Tarifs\nUne séance coûte soixante euros à Paris.\n",
        )
        .unwrap();
        let idx = RagIndex::load(dir.to_str().unwrap()).unwrap();
        let hits = idx.search("sensation de douleur pendant la séance", 1);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].title.contains("Douleur"));
        fs::remove_dir_all(&dir).ok();
    }
}
