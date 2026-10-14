//! Streaming byte masker. See ADR 0002 "Masking".
//!
//! One masker per child stream. Between chunks it holds back at most
//! `longest_secret_len - 1` bytes, and only a tail that could still start a
//! secret, so a value split across two reads is still replaced.

use zeroize::Zeroizing;

pub(crate) const MASK: &[u8] = b"<hidden by hush>";

/// Values shorter than this are not masked; `hush run` warns instead.
pub(crate) const MIN_MASKED_LEN: usize = 8;

pub(crate) struct Masker {
    /// Longest first, so a value that starts with a shorter value still masks whole.
    secrets: Vec<Zeroizing<Vec<u8>>>,
    pending: Zeroizing<Vec<u8>>,
}

impl Masker {
    pub(crate) fn new(values: impl IntoIterator<Item = Zeroizing<Vec<u8>>>) -> Self {
        let mut secrets: Vec<_> = values
            .into_iter()
            .filter(|value| value.len() >= MIN_MASKED_LEN)
            .collect();
        secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
        secrets.dedup_by(|left, right| left[..] == right[..]);
        Self {
            secrets,
            pending: Zeroizing::new(Vec::new()),
        }
    }

    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        // Grow into a fresh buffer: an in-place realloc would free the old
        // secret-bearing allocation without zeroing it.
        let mut grown = Zeroizing::new(Vec::with_capacity(self.pending.len() + chunk.len()));
        grown.extend_from_slice(&self.pending);
        grown.extend_from_slice(chunk);
        self.pending = grown;
        self.drain(false)
    }

    pub(crate) fn finish(&mut self) -> Vec<u8> {
        self.drain(true)
    }

    fn drain(&mut self, at_eof: bool) -> Vec<u8> {
        let buf = std::mem::replace(&mut self.pending, Zeroizing::new(Vec::new()));
        let mut out = Vec::with_capacity(buf.len());
        let mut at = 0;
        while at < buf.len() {
            let rest = &buf[at..];
            // Before EOF, hold the tail only where a secret could still start,
            // so a value split across reads is never cut in half.
            if !at_eof
                && self
                    .secrets
                    .iter()
                    .any(|secret| secret.len() > rest.len() && secret.starts_with(rest))
            {
                break;
            }
            match self.secrets.iter().find(|secret| rest.starts_with(secret)) {
                Some(secret) => {
                    out.extend_from_slice(MASK);
                    at += secret.len();
                }
                None => {
                    out.push(buf[at]);
                    at += 1;
                }
            }
        }
        self.pending = Zeroizing::new(buf[at..].to_vec());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn masker(values: &[&str]) -> Masker {
        Masker::new(
            values
                .iter()
                .map(|value| Zeroizing::new(value.as_bytes().to_vec())),
        )
    }

    fn run(masker: &mut Masker, chunks: &[&str]) -> String {
        let mut out = Vec::new();
        for chunk in chunks {
            out.extend(masker.push(chunk.as_bytes()));
        }
        out.extend(masker.finish());
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn value_in_one_chunk_is_replaced() {
        let mut m = masker(&["FAKE-value-123"]);
        assert_eq!(
            run(&mut m, &["token=FAKE-value-123 done\n"]),
            "token=<hidden by hush> done\n"
        );
    }

    #[test]
    fn value_split_across_chunks_is_replaced() {
        let mut m = masker(&["FAKE-value-123"]);
        assert_eq!(
            run(&mut m, &["a FAKE-", "val", "ue-123 b"]),
            "a <hidden by hush> b"
        );
    }

    #[test]
    fn hold_back_never_exceeds_longest_minus_one_bytes() {
        let mut m = masker(&["FAKE-value-123"]);
        assert_eq!(m.push(b"xx FAKE-value-12"), b"xx ");
        assert_eq!(m.finish().len(), 14 - 1);
    }

    #[test]
    fn only_a_possible_secret_start_is_held_back() {
        let mut m = masker(&["FAKE-value-123"]);
        assert_eq!(m.push(b"line done\n"), b"line done\n");
        assert_eq!(m.push(b"next FAKE-va"), b"next ");
        assert_eq!(m.finish(), b"FAKE-va");
    }

    #[test]
    fn short_values_are_not_masked() {
        let mut m = masker(&["short"]);
        assert_eq!(run(&mut m, &["a short b"]), "a short b");
    }

    #[test]
    fn longer_value_wins_over_its_prefix() {
        let mut m = masker(&["FAKE-abcdefgh", "FAKE-abcdefgh-longer"]);
        assert_eq!(
            run(&mut m, &["x FAKE-abcdefgh-longer y FAKE-abcdefgh z"]),
            "x <hidden by hush> y <hidden by hush> z"
        );
    }
}
