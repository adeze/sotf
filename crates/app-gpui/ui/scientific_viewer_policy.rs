/// Keep main-frame navigation on the owned viewer; hash-based state is local.
pub fn navigation_allowed(viewer_url: &str, candidate: &str) -> bool {
    fn parts(url: &str) -> (&str, &str) {
        let url = url.split('#').next().unwrap_or(url);
        let (base, query) = url.split_once('?').unwrap_or((url, ""));
        (base.trim_end_matches('/'), query)
    }
    parts(candidate) == parts(viewer_url)
}
