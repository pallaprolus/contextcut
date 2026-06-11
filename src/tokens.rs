use tiktoken_rs::{cl100k_base, o200k_base};

/// Claude has no public tokenizer; on code, Claude counts run ~10–25%
/// above cl100k_base empirically. We use ×1.15 and label it "approx".
const CLAUDE_FACTOR: f64 = 1.15;

pub struct Estimate {
    pub o200k: usize,
    pub cl100k: usize,
}

impl Estimate {
    pub fn claude_approx(&self) -> usize {
        (self.cl100k as f64 * CLAUDE_FACTOR).round() as usize
    }

    pub fn table(&self, exact_claude: Option<usize>) -> String {
        let claude_row = match exact_claude {
            Some(n) => format!("  Claude (exact)        {:>10}", group(n)),
            None => format!(
                "  Claude (approx ×1.15) {:>10}",
                group(self.claude_approx())
            ),
        };
        format!(
            "  ── Estimated tokens ─────────────\n  GPT (o200k_base)      {:>10}\n  GPT-4 (cl100k_base)   {:>10}\n{claude_row}\n  Gemini (approx)       {:>10}",
            group(self.o200k),
            group(self.cl100k),
            group(self.o200k),
        )
    }
}

/// Count tokens of the final Markdown with both major BPE encodings.
/// `encode_ordinary` treats special tokens (e.g. a literal
/// "<|endoftext|>" in source code) as plain text instead of erroring.
pub fn estimate(text: &str) -> Estimate {
    let o200k = o200k_base().expect("bundled o200k BPE data");
    let cl100k = cl100k_base().expect("bundled cl100k BPE data");
    Estimate {
        o200k: o200k.encode_ordinary(text).len(),
        cl100k: cl100k.encode_ordinary(text).len(),
    }
}

/// 1234567 → "1,234,567"
fn group(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_answer_hello_world() {
        let est = estimate("hello world");
        // Both encodings tokenize "hello world" as ["hello", " world"].
        assert_eq!(est.o200k, 2);
        assert_eq!(est.cl100k, 2);
    }

    #[test]
    fn empty_is_zero() {
        let est = estimate("");
        assert_eq!(est.o200k, 0);
        assert_eq!(est.cl100k, 0);
        assert_eq!(est.claude_approx(), 0);
    }

    #[test]
    fn claude_factor_applied() {
        let est = Estimate {
            o200k: 0,
            cl100k: 100,
        };
        assert_eq!(est.claude_approx(), 115);
    }

    #[test]
    fn special_tokens_counted_as_plain_text() {
        // Must not panic or treat as a single special token.
        let est = estimate("<|endoftext|>");
        assert!(est.o200k > 1);
    }

    #[test]
    fn digit_grouping() {
        assert_eq!(group(999), "999");
        assert_eq!(group(1234567), "1,234,567");
    }
}
