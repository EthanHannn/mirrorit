pub fn is_safe_https_url(value: &str) -> bool {
    if value.contains(char::is_whitespace) || value.contains('\\') {
        return false;
    }
    reqwest::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_hosts_credentials_and_query_secrets() {
        for value in [
            "https://",
            "https://?token=example",
            "https://user:secret@example.com",
            "https://example.com/?token=example",
            "https://example.com/#secret",
            "http://example.com",
            "https://example.com/\nregistry=other",
            "https://example.com\\@other.com",
        ] {
            assert!(!is_safe_https_url(value), "{value}");
        }
    }

    #[test]
    fn accepts_https_registries_with_ports_and_paths() {
        assert!(is_safe_https_url(
            "https://registry.example.com:8443/repository/npm/"
        ));
    }
}
