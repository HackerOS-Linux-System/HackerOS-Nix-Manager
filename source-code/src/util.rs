pub fn split_name_version(s: &str) -> (String, String) {
    let bytes = s.as_bytes();
    let mut split_at = None;
    for i in (1..bytes.len()).rev() {
        if bytes[i - 1] == b'-' && bytes[i].is_ascii_digit() {
            split_at = Some(i - 1);
            break;
        }
    }
    match split_at {
        Some(i) => (s[..i].to_string(), s[i + 1..].to_string()),
        None => (s.to_string(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_name_version() {
        assert_eq!(
            split_name_version("ripgrep-14.1.1"),
            ("ripgrep".to_string(), "14.1.1".to_string())
        );
    }

    #[test]
    fn no_version_returns_whole_string() {
        assert_eq!(
            split_name_version("noversion"),
            ("noversion".to_string(), String::new())
        );
    }

    #[test]
    fn handles_dashes_inside_name() {
        assert_eq!(
            split_name_version("python3.11-requests-2.31.0"),
            ("python3.11-requests".to_string(), "2.31.0".to_string())
        );
    }
}
