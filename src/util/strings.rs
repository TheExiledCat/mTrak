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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_delimiters_between_parts() {
        assert_eq!(
            split_keep_delim("C-4|1|40", "|"),
            vec!["C-4", "|", "1", "|", "40"]
        );
    }

    #[test]
    fn handles_edges_and_repeats() {
        assert_eq!(split_keep_delim("|a||", "|"), vec!["|", "a", "|", "|"]);
        assert_eq!(split_keep_delim("abc", "|"), vec!["abc"]);
        assert!(split_keep_delim("", "|").is_empty());
    }
}
