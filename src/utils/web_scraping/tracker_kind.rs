use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TrackerKind {
    Page,
    Api,
}

impl From<TrackerKind> for (&str, &str) {
    fn from(value: TrackerKind) -> Self {
        (&value).into()
    }
}

impl From<&TrackerKind> for (&str, &str) {
    fn from(value: &TrackerKind) -> Self {
        match value {
            TrackerKind::Page => ("web_scraping", "page"),
            TrackerKind::Api => ("web_scraping", "api"),
        }
    }
}

impl TryFrom<(&str, &str)> for TrackerKind {
    type Error = ();

    fn try_from((area, resource): (&str, &str)) -> Result<Self, Self::Error> {
        match (area, resource) {
            ("web_scraping", "page") => Ok(TrackerKind::Page),
            ("web_scraping", "api") => Ok(TrackerKind::Api),
            _ => Err(()),
        }
    }
}

impl FromStr for TrackerKind {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split("__").collect::<Vec<_>>();
        if parts.len() != 2 {
            return Err(());
        }
        TrackerKind::try_from((
            parts[0].to_lowercase().as_str(),
            parts[1].to_lowercase().as_str(),
        ))
    }
}

impl Display for TrackerKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let (area, resource) = Into::<(&str, &str)>::into(self);
        write!(f, "{area}__{resource}")
    }
}

#[cfg(test)]
mod tests {
    use super::TrackerKind;

    #[test]
    fn properly_parses_tracker_kind() {
        assert_eq!(
            TrackerKind::try_from(("web_scraping", "page")),
            Ok(TrackerKind::Page)
        );
        assert_eq!(
            TrackerKind::try_from(("web_scraping", "api")),
            Ok(TrackerKind::Api)
        );

        assert!(TrackerKind::try_from(("certificates", "templates")).is_err());
        assert!(TrackerKind::try_from(("webhooks", "responders")).is_err());
        assert!(TrackerKind::try_from(("web_security", "csp")).is_err());
        assert!(TrackerKind::try_from(("web_scraping", "_page")).is_err());
    }

    #[test]
    fn correctly_converts_into_tuple() {
        type KindTuple = (&'static str, &'static str);
        assert_eq!(KindTuple::from(TrackerKind::Page), ("web_scraping", "page"));
        assert_eq!(KindTuple::from(TrackerKind::Api), ("web_scraping", "api"));
    }
}
