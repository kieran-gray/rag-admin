pub fn score(haystack_lower: &str, needle_lower: &str) -> Option<i32> {
    if needle_lower.is_empty() {
        return Some(0);
    }

    if let Some(pos) = haystack_lower.find(needle_lower) {
        let mut s = 1000 - pos as i32;
        if pos == 0 {
            s += 300;
        } else if haystack_lower.as_bytes().get(pos - 1) == Some(&b' ') {
            s += 150;
        }
        return Some(s);
    }

    subsequence_score(haystack_lower, needle_lower)
}

fn subsequence_score(haystack_lower: &str, needle_lower: &str) -> Option<i32> {
    let mut chars = haystack_lower.chars();
    let mut cur = chars.next();
    let mut matched = 0i32;

    for nc in needle_lower.chars() {
        loop {
            match cur {
                Some(c) if c == nc => {
                    matched += 1;
                    cur = chars.next();
                    break;
                }
                Some(_) => cur = chars.next(),
                None => return None,
            }
        }
    }

    Some(matched * 5)
}
