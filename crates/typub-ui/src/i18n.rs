use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    Zh,
    En,
}

static LOCALE: RwLock<Option<Locale>> = RwLock::new(None);

fn detect_locale() -> Locale {
    let locale_str = std::env::var("LC_ALL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LANG").ok().filter(|s| !s.is_empty()))
        .unwrap_or_default();

    if locale_str.to_lowercase().starts_with("zh") {
        Locale::Zh
    } else {
        Locale::En
    }
}

pub fn locale() -> Locale {
    if let Ok(guard) = LOCALE.read()
        && let Some(loc) = *guard
    {
        return loc;
    }

    let detected = detect_locale();
    if let Ok(mut guard) = LOCALE.write()
        && guard.is_none()
    {
        *guard = Some(detected);
    }
    detected
}

#[doc(hidden)]
pub fn set_locale(loc: Locale) {
    if let Ok(mut guard) = LOCALE.write() {
        *guard = Some(loc);
    }
}

pub fn t(key: &str) -> &'static str {
    let loc = locale();
    match (loc, key) {
        (Locale::Zh, "copy_button") => "复制内容",
        (Locale::Zh, "open_editor") => "打开编辑器",
        (Locale::Zh, "copy_success") => "已复制到剪贴板!",
        (Locale::Zh, "copy_failed") => "复制失败，请手动选择内容复制",
        (Locale::Zh, "slides_ready") => "张图片已准备好上传",
        (Locale::Zh, "preview_suffix") => "预览",

        (Locale::En, "copy_button") => "Copy",
        (Locale::En, "open_editor") => "Open Editor",
        (Locale::En, "copy_success") => "Copied!",
        (Locale::En, "copy_failed") => "Copy failed. Please select and copy manually.",
        (Locale::En, "slides_ready") => "slides ready for upload",
        (Locale::En, "preview_suffix") => "Preview",

        _ => "[missing translation]",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn test_t_returns_translation() {
        let result = t("copy_button");
        assert!(!result.is_empty());
        assert!(result == "复制内容" || result == "Copy");
    }

    #[test]
    fn test_t_unknown_key_returns_placeholder() {
        assert_eq!(t("unknown_key_xyz"), "[missing translation]");
    }
}
