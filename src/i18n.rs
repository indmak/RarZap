//! i18n: 8 locales, English fallback, auto-match system locale.
//! Locales live in `locales/*.yml` and are embedded at compile time.
//! 注意：`rust_i18n::i18n!` 必须在 crate root（lib.rs）调用，
//! 它生成的后端挂在 crate 根上，`t!` 才能解析到。

pub use rust_i18n::t;

/// 支持的语言（代码, 原生显示名）。代码与 `locales/<code>.yml` 对应。
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("zh-CN", "简体中文"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("ru", "Русский"),
    ("ja", "日本語"),
    ("ko", "한국어"),
];

/// Normalize e.g. `zh-CN`, `zh_CN`, `zh-Hans-CN` -> `zh-CN`; `en-US` -> `en`.
pub fn normalize_locale(tag: &str) -> String {
    let t = tag.replace('_', "-");
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("zh") {
        return "zh-CN".to_string();
    }
    let lang = t.split('-').next().unwrap_or("en").to_ascii_lowercase();
    match lang.as_str() {
        "fr" | "de" | "es" | "ru" | "ja" | "ko" => lang,
        _ => "en".to_string(),
    }
}

/// 是否为受支持的规范化代码。
pub fn is_supported(code: &str) -> bool {
    LANGUAGES.iter().any(|(c, _)| *c == code)
}

/// Pick best locale for the OS. Falls back to English.
pub fn resolve_system_locale() -> String {
    let tag = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
    let norm = normalize_locale(&tag);
    if is_supported(&norm) {
        norm
    } else {
        "en".to_string()
    }
}

/// 应用语言：`pref` 为 None 时跟随系统。返回实际生效代码。
pub fn apply_locale(pref: Option<&str>) -> String {
    let code = match pref {
        Some(c) if is_supported(c) => c.to_string(),
        _ => resolve_system_locale(),
    };
    rust_i18n::set_locale(&code);
    code
}

/// Set global rust-i18n locale from config/OS. Call once at startup.
pub fn init_locale() {
    let locale = apply_locale(None);
    tracing::info!(locale = %locale, "locale initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_fallback() {
        assert_eq!(normalize_locale("en-US"), "en");
        assert_eq!(normalize_locale("zh_CN"), "zh-CN");
        assert_eq!(normalize_locale("xx-YY"), "en");
    }
}
