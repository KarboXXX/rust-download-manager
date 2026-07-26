pub fn slice_from_start(s: String, n: usize) -> String {
    s.chars().into_iter().take(n).collect()
}

pub fn slice_from_end(s: &str, n: usize) -> Option<&str> {
    s.char_indices().rev().nth(n).map(|(i, _)| &s[i..])
}