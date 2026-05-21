pub fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::normalize_whitespace;

    #[test]
    fn collapses_unicode_whitespace() {
        assert_eq!(normalize_whitespace("  Hello\n\tworld  "), "Hello world");
    }
}
