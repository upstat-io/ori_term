use super::validate_scheme;

#[test]
fn allowed_http_scheme() {
    assert!(validate_scheme("http://example.com").is_ok());
}

#[test]
fn allowed_https_scheme() {
    assert!(validate_scheme("https://example.com").is_ok());
}

#[test]
fn allowed_mailto_scheme() {
    assert!(validate_scheme("mailto:user@example.com").is_ok());
}

#[test]
fn scheme_validation_case_insensitive() {
    assert!(validate_scheme("HTTPS://EXAMPLE.COM").is_ok());
    assert!(validate_scheme("Http://Example.Com").is_ok());
}

#[test]
fn disallowed_file_scheme() {
    let result = validate_scheme("file:///etc/passwd");
    assert!(result.is_err());
}

#[test]
fn disallowed_javascript_scheme() {
    let result = validate_scheme("javascript:alert(1)");
    assert!(result.is_err());
}

#[test]
fn disallowed_empty_url() {
    let result = validate_scheme("");
    assert!(result.is_err());
}

#[test]
fn disallowed_no_scheme() {
    let result = validate_scheme("example.com");
    assert!(result.is_err());
}

#[test]
fn disallowed_custom_protocol() {
    let result = validate_scheme("myapp://callback");
    assert!(result.is_err());
}
