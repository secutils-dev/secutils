/// Represents type of the web page tracker (e.g. resources, content, etc.).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum WebPageTrackerKind {
    WebPageResources = 0,
}

#[cfg(test)]
mod tests {
    use crate::utils::WebPageTrackerKind;
    #[test]
    fn correctly_returns_value() {
        assert_eq!(WebPageTrackerKind::WebPageResources as u8, 0);
    }
}
