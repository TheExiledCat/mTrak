pub fn split_keep_delim<'a>(s: &'a str, pat: &str) -> Vec<&'a str> {
    let mut result = Vec::new();
    let mut last = 0;

    for (idx, _) in s.match_indices(pat) {
        if last != idx {
            result.push(&s[last..idx]); // before delimiter
        }
        result.push(&s[idx..idx + pat.len()]); // the delimiter itself
        last = idx + pat.len();
    }

    if last < s.len() {
        result.push(&s[last..]); // remainder
    }

    result
}
