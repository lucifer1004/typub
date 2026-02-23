pub trait MetadataService: Send + Sync {
    fn normalize_terms(&self, terms: &[String]) -> Vec<String>;
}

pub struct DefaultMetadataService;

impl MetadataService for DefaultMetadataService {
    fn normalize_terms(&self, terms: &[String]) -> Vec<String> {
        let mut normalized: Vec<String> = terms
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        normalized.sort_by_key(|s| s.to_lowercase());
        normalized.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_service_normalize() {
        let service = DefaultMetadataService;

        let result = service.normalize_terms(&[" tag ".into()]);
        assert_eq!(result, vec!["tag"]);

        let result = service.normalize_terms(&["Rust".into(), "rust".into(), "RUST".into()]);
        assert_eq!(result.len(), 1);

        let result = service.normalize_terms(&["Zebra".into(), "Apple".into()]);
        assert_eq!(result, vec!["Apple", "Zebra"]);

        let result = service.normalize_terms(&["".into(), "tag".into(), "  ".into()]);
        assert_eq!(result, vec!["tag"]);
    }
}
