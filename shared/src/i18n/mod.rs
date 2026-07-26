pub mod en_us;
pub mod zh_cn;
pub mod zh_tw;

use crate::Locale;
use serde::Deserialize;
use std::sync::OnceLock;

static CURRENT_LOCALE: OnceLock<Locale> = OnceLock::new();

#[derive(Debug, PartialEq, Eq)]
struct LocaleResolution {
    locale: Locale,
    unsupported_config_locale: Option<String>,
}

#[derive(Deserialize)]
struct LocaleConfig {
    locale: Option<String>,
}

fn normalize_locale(locale: &str) -> String {
    locale.trim().replace('_', "-").to_ascii_lowercase()
}

fn parse_config_locale(contents: &str) -> Option<String> {
    let locale = serde_yaml_ng::from_str::<LocaleConfig>(contents)
        .ok()?
        .locale?;
    let locale = locale.trim();
    (!locale.is_empty()).then(|| locale.to_owned())
}

fn resolve_system_locale(locale: Option<&str>) -> Option<Locale> {
    let locale = normalize_locale(locale?);
    let locale = locale.split(['.', '@']).next()?;
    let mut parts = locale.split('-');
    match parts.next()? {
        "en" => Some(Locale::EnUs),
        "zh" => {
            let subtags: Vec<_> = parts.collect();
            if subtags.contains(&"hans") {
                Some(Locale::ZhCn)
            } else if subtags.contains(&"hant") {
                Some(Locale::ZhTw)
            } else if subtags.iter().any(|part| matches!(*part, "cn" | "sg")) {
                Some(Locale::ZhCn)
            } else {
                Some(Locale::ZhTw)
            }
        }
        _ => None,
    }
}

fn resolve_locale(
    config_locale: Option<&str>,
    system_locale: impl FnOnce() -> Option<String>,
) -> LocaleResolution {
    if let Some(config_locale) = config_locale
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let locale = match normalize_locale(config_locale).as_str() {
            "zh-tw" => Some(Locale::ZhTw),
            "en-us" => Some(Locale::EnUs),
            "zh-cn" => Some(Locale::ZhCn),
            _ => None,
        };
        if let Some(locale) = locale {
            return LocaleResolution {
                locale,
                unsupported_config_locale: None,
            };
        }

        return LocaleResolution {
            locale: Locale::ZhTw,
            unsupported_config_locale: Some(config_locale.to_owned()),
        };
    }

    LocaleResolution {
        locale: resolve_system_locale(system_locale().as_deref()).unwrap_or(Locale::ZhTw),
        unsupported_config_locale: None,
    }
}

fn initialize_locale(
    config_locale: Option<&str>,
    system_locale: impl FnOnce() -> Option<String>,
    warn_unsupported: impl FnOnce(&str),
) -> Locale {
    let resolution = resolve_locale(config_locale, system_locale);
    if let Some(unsupported) = resolution.unsupported_config_locale.as_deref() {
        warn_unsupported(unsupported);
    }
    resolution.locale
}

fn read_config_locale() -> Option<String> {
    let contents = fs_err::read_to_string(crate::get_config_path().ok()?).ok()?;
    parse_config_locale(&contents)
}

pub fn current_locale() -> Locale {
    *CURRENT_LOCALE.get_or_init(|| {
        let configured = read_config_locale();
        let locale = initialize_locale(configured.as_deref(), sys_locale::get_locale, |locale| {
            log::warn!("Locale '{locale}' is unsupported.");
        });
        log::debug!("Use locate: {locale:?}");
        locale
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn supported_config_wins_without_querying_system() {
        for (configured, expected) in [
            ("zh-TW", Locale::ZhTw),
            ("ZH_tw", Locale::ZhTw),
            ("en-US", Locale::EnUs),
            ("EN_us", Locale::EnUs),
            ("zh-CN", Locale::ZhCn),
            ("ZH_cn", Locale::ZhCn),
        ] {
            let queries = Cell::new(0);
            let resolution = resolve_locale(Some(configured), || {
                queries.set(queries.get() + 1);
                Some("ja-JP".to_owned())
            });

            assert_eq!(resolution.locale, expected);
            assert_eq!(resolution.unsupported_config_locale, None);
            assert_eq!(queries.get(), 0);
        }
    }

    #[test]
    fn absent_or_empty_config_queries_system() {
        for configured in [None, Some(""), Some("   ")] {
            let queries = Cell::new(0);
            let resolution = resolve_locale(configured, || {
                queries.set(queries.get() + 1);
                Some("en-US".to_owned())
            });

            assert_eq!(resolution.locale, Locale::EnUs);
            assert_eq!(resolution.unsupported_config_locale, None);
            assert_eq!(queries.get(), 1);
        }
    }

    #[test]
    fn unsupported_config_is_recorded_without_querying_system() {
        let queries = Cell::new(0);
        let resolution = resolve_locale(Some(" fr-FR "), || {
            queries.set(queries.get() + 1);
            Some("zh-TW".to_owned())
        });

        assert_eq!(resolution.locale, Locale::ZhTw);
        assert_eq!(
            resolution.unsupported_config_locale.as_deref(),
            Some("fr-FR")
        );
        assert_eq!(queries.get(), 0);
    }

    #[test]
    fn config_parser_treats_missing_null_empty_and_invalid_as_unset() {
        for yaml in [
            "{}",
            "locale: null",
            "locale: ''",
            "locale: '   '",
            "locale: [",
        ] {
            assert_eq!(parse_config_locale(yaml), None);
        }
        assert_eq!(
            parse_config_locale("locale: zh_TW\nother: ignored"),
            Some("zh_TW".to_owned())
        );
    }

    #[test]
    fn supported_system_locale_variants_are_recognized() {
        for (locale, expected) in [
            ("zh", Locale::ZhTw),
            ("zh-TW", Locale::ZhTw),
            ("zh_Hant_HK", Locale::ZhTw),
            ("zh-MO", Locale::ZhTw),
            ("zh-CN", Locale::ZhCn),
            ("zh_Hans_SG", Locale::ZhCn),
            ("en", Locale::EnUs),
            ("en-US", Locale::EnUs),
            ("en_GB", Locale::EnUs),
        ] {
            assert_eq!(resolve_system_locale(Some(locale)), Some(expected));
        }
    }

    #[test]
    fn other_or_missing_system_locales_are_unrecognized() {
        for locale in [None, Some(""), Some("ja-JP"), Some("zhfoo")] {
            assert_eq!(resolve_system_locale(locale), None);
        }
    }

    #[test]
    fn unsupported_config_warns_once_and_skips_system() {
        let system_queries = Cell::new(0);
        let warnings = Cell::new(0);
        let mut warned_locale = None;

        let locale = initialize_locale(
            Some("fr-FR"),
            || {
                system_queries.set(system_queries.get() + 1);
                Some("zh-TW".to_owned())
            },
            |unsupported| {
                warnings.set(warnings.get() + 1);
                warned_locale = Some(unsupported.to_owned());
            },
        );

        assert_eq!(locale, Locale::ZhTw);
        assert_eq!(system_queries.get(), 0);
        assert_eq!(warnings.get(), 1);
        assert_eq!(warned_locale.as_deref(), Some("fr-FR"));
    }
}
