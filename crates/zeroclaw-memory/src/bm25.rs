//! In-memory BM25 scorer for hybrid retrieval.
//!
//! The SQLite memory backend uses FTS5 for keyword scoring, which is the
//! right call when the entire corpus lives on disk. The Qdrant backend
//! holds documents remotely, so we instead **over-fetch** by dense
//! vector and run BM25 over the fetched batch. That's enough to recover
//! the wins of hybrid search without a separate FTS sidecar.
//!
//! Implementation notes:
//! - Tokeniser: lowercase, split on non-alphanumeric, drop tokens
//!   shorter than 2 chars. Latin/CJK aware (alphanumeric covers most).
//! - Default k1=1.5, b=0.75 (Robertson/Walker defaults; Lucene's
//!   `BM25Similarity` uses 1.2/0.75 — both are within typical ranges).
//! - IDF clamped at 0 so very common terms don't go negative and
//!   pull scores into the noise floor.
//! - Output is `(id, raw_bm25_score)` — `vector::hybrid_merge`
//!   normalises before fusion.

const DEFAULT_K1: f32 = 1.5;
const DEFAULT_B: f32 = 0.75;
const MIN_TOKEN_LEN: usize = 2;

pub struct Bm25Scorer {
    pub k1: f32,
    pub b: f32,
}

impl Default for Bm25Scorer {
    fn default() -> Self {
        Self {
            k1: DEFAULT_K1,
            b: DEFAULT_B,
        }
    }
}

/// Tokenise: lowercase + split on non-alphanumeric + drop short tokens.
fn tokenize(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            current.extend(ch.to_lowercase());
        } else if !current.is_empty() {
            if current.len() >= MIN_TOKEN_LEN {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.len() >= MIN_TOKEN_LEN {
        out.push(current);
    }
    out
}

impl Bm25Scorer {
    /// Score every doc against the query and return ranked
    /// `(id, score)` pairs descending by score. Documents that don't
    /// share a single term with the query are dropped.
    pub fn rank(&self, query: &str, docs: &[(&str, &str)]) -> Vec<(String, f32)> {
        let q_tokens = tokenize(query);
        if q_tokens.is_empty() || docs.is_empty() {
            return Vec::new();
        }

        // Pre-tokenise each doc once.
        let doc_tokens: Vec<(String, Vec<String>)> = docs
            .iter()
            .map(|(id, content)| ((*id).to_string(), tokenize(content)))
            .collect();

        // Average document length (in tokens).
        let total_len: usize = doc_tokens.iter().map(|(_, t)| t.len()).sum();
        let n = doc_tokens.len() as f32;
        let avgdl = if n > 0.0 {
            total_len as f32 / n
        } else {
            1.0
        };

        // Document frequency per query term (within this batch).
        let unique_q_terms: Vec<&String> = {
            let mut seen = std::collections::HashSet::new();
            q_tokens.iter().filter(|t| seen.insert(t.as_str())).collect()
        };
        let mut df: std::collections::HashMap<&str, f32> =
            std::collections::HashMap::with_capacity(unique_q_terms.len());
        for t in &unique_q_terms {
            let count = doc_tokens
                .iter()
                .filter(|(_, dt)| dt.iter().any(|dt_t| dt_t == t.as_str()))
                .count();
            df.insert(t.as_str(), count as f32);
        }

        // Score each doc.
        let mut results: Vec<(String, f32)> = Vec::with_capacity(doc_tokens.len());
        for (id, dt) in &doc_tokens {
            let dl = dt.len() as f32;
            if dl == 0.0 {
                continue;
            }
            let mut score = 0.0_f32;
            for term in &unique_q_terms {
                let n_t = *df.get(term.as_str()).unwrap_or(&0.0);
                if n_t == 0.0 {
                    continue;
                }
                // IDF clamped: log((N - df + 0.5)/(df + 0.5) + 1)
                let idf = (((n - n_t + 0.5) / (n_t + 0.5)) + 1.0).ln().max(0.0);
                let f_td = dt.iter().filter(|x| x.as_str() == term.as_str()).count() as f32;
                if f_td == 0.0 {
                    continue;
                }
                let denom = f_td + self.k1 * (1.0 - self.b + self.b * dl / avgdl);
                let tf = (f_td * (self.k1 + 1.0)) / denom;
                score += idf * tf;
            }
            if score > 0.0 {
                results.push((id.clone(), score));
            }
        }

        results.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_lowercase_and_strip_punctuation() {
        let toks = tokenize("Hello, World! 123 a");
        assert_eq!(toks, vec!["hello", "world", "123"]);
    }

    #[test]
    fn rank_prefers_doc_with_more_query_terms() {
        let scorer = Bm25Scorer::default();
        let docs = vec![
            ("a", "the value equation has four drivers"),
            ("b", "value equation drives perceived value"),
            ("c", "this document is about cats and dogs"),
        ];
        let results = scorer.rank("value equation drivers", &docs);
        assert!(!results.is_empty());
        // doc "a" has all three terms; doc "b" two; doc "c" zero
        assert_eq!(results[0].0, "a");
        // doc "c" should not appear (no shared terms)
        assert!(results.iter().all(|(id, _)| id != "c"));
    }

    #[test]
    fn rank_returns_empty_on_no_match() {
        let scorer = Bm25Scorer::default();
        let docs = vec![("a", "completely unrelated text")];
        let r = scorer.rank("foo bar baz", &docs);
        assert!(r.is_empty());
    }

    #[test]
    fn rank_empty_query_returns_empty() {
        let scorer = Bm25Scorer::default();
        let docs = vec![("a", "hello world")];
        assert!(scorer.rank("", &docs).is_empty());
        assert!(scorer.rank("!!!", &docs).is_empty());
    }

    #[test]
    fn rank_handles_repeated_terms_in_doc() {
        let scorer = Bm25Scorer::default();
        // term-frequency saturates by k1 — many repeats shouldn't dominate
        let docs = vec![
            ("a", "anchor anchor anchor anchor anchor"),
            ("b", "anchor pricing strategy works"),
        ];
        let r = scorer.rank("anchor pricing", &docs);
        // both should be present, "b" should win because it covers both terms
        assert_eq!(r[0].0, "b");
    }
}
