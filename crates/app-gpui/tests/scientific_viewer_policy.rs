#[path = "../ui/scientific_viewer_policy.rs"]
mod policy;

#[test]
fn viewer_navigation_keeps_owned_origin_and_connection() {
    let url = "http://127.0.0.1:9123/?url=rerun%2Bhttp://127.0.0.1:9124/proxy";
    assert!(policy::navigation_allowed(url, url));
    assert!(policy::navigation_allowed(url, &format!("{url}#room")));
    assert!(policy::navigation_allowed(url, &url.replacen("/?", "?", 1)));
    for candidate in [
        "https://example.com/",
        "http://127.0.0.1:9123.evil.test/",
        "http://127.0.0.1:9123/?url=rerun%2Bhttp://example.com/proxy",
        "javascript:alert(1)",
        "file:///etc/passwd",
    ] {
        assert!(!policy::navigation_allowed(url, candidate));
    }
}
