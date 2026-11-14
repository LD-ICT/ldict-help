//! LD ICT Help: the parts of this fork that are ours (see ldict/README.md).
//!
//! Everything LD ICT changes lives in this module and in ldict/. The upstream code only calls `apply()` (from
//! common.rs, before any setting is read), so a rebase onto a new RustDesk release touches as few lines as possible.

use base::config::keys;
use hbb_common::config;

/// The technical name of the program, the same in every language. No spaces: RustDesk also uses it for its URI scheme (ldict-help://), paths and the
/// Windows service. The macOS bundle id (config::ORG) stays RustDesk's: it must match the app's Info.plist.
pub const APP_NAME: &str = "LDICT-Help";

/// The name people see: Dutch on a Dutch computer (or when Dutch is chosen in the settings), English otherwise.
pub const DISPLAY_NAME: &str = "LD ICT Help";
pub const DISPLAY_NAME_NL: &str = "LD ICT Hulp";

/// The fixed name of LD ICT's support server (the dashboard points it at the active server).
pub const SERVER: &str = "hulp.ldict.nl";

/// The public key of that server, base64. It is public: it is inside every copy of the program and on the
/// server's settings page in the admin. Set in ldict/key.pub, which the release workflow refuses to build without.
pub const KEY: &str = include_str!("../ldict/key.pub");

/// Settings the user can't change, in RustDesk's three layers.
const OVERRIDE: &[(&str, &str)] = &[
    // Always our server, our relay and our key.
    (keys::OPTION_CUSTOM_RENDEZVOUS_SERVER, SERVER),
    (keys::OPTION_RELAY_SERVER, SERVER),
    (keys::OPTION_KEY, KEY_TRIMMED),
    // No RustDesk account server.
    (keys::OPTION_API_SERVER, ""),
    // No fixed password: only "Accepteren" by the person at the computer (until phase 2 adds connect tokens).
    (keys::OPTION_VERIFICATION_METHOD, "use-temporary-password"),
    (keys::OPTION_APPROVE_MODE, "click"),
    // Updates come from LD ICT, not from rustdesk.com.
    (keys::OPTION_ENABLE_CHECK_UPDATE, "N"),
    (keys::OPTION_ALLOW_AUTO_UPDATE, "N"),
    // Only through the server: no direct IP access and no LAN discovery.
    (keys::OPTION_DIRECT_SERVER, "N"),
    (keys::OPTION_ENABLE_LAN_DISCOVERY, "N"),
];

const BUILTIN: &[(&str, &str)] = &[
    (keys::OPTION_HIDE_SERVER_SETTINGS, "Y"),
    (keys::OPTION_HIDE_PROXY_SETTINGS, "Y"),
    (keys::OPTION_HIDE_WEBSOCKET_SETTINGS, "Y"),
    (keys::OPTION_HIDE_HELP_CARDS, "Y"),
    (keys::OPTION_DISABLE_CHANGE_ID, "Y"),
    (keys::OPTION_DISABLE_CHANGE_PERMANENT_PASSWORD, "Y"),
];

const HARD: &[(&str, &str)] = &[
    // No RustDesk account and no address book: that is in LD ICT's admin.
    ("disable-account", "Y"),
    ("disable-ab", "Y"),
];

const KEY_TRIMMED: &str = trim_ascii(KEY);

/// `str::trim_ascii` as a const fn, so ldict/key.pub may end with a newline.
const fn trim_ascii(value: &str) -> &str {
    let bytes = value.as_bytes();
    let (mut start, mut end) = (0, bytes.len());
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    match std::str::from_utf8(bytes.split_at(end).0.split_at(start).1) {
        Ok(trimmed) => trimmed,
        Err(_) => panic!("ldict/key.pub is not UTF-8"),
    }
}

/// The display name for a resolved RustDesk language code ("nl", "en", ...).
pub fn display_name_for(lang: &str) -> &'static str {
    if lang.to_lowercase().starts_with("nl") {
        DISPLAY_NAME_NL
    } else {
        DISPLAY_NAME
    }
}

/// The display name for this computer: the language chosen in the settings, else the system's.
pub fn display_name() -> String {
    let saved = config::LocalConfig::get_option("lang");
    let lang = if saved.is_empty() || saved == "default" {
        system_locale()
    } else {
        saved
    };
    display_name_for(&lang).to_owned()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn system_locale() -> String {
    sys_locale::get_locale().unwrap_or_default()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn system_locale() -> String {
    String::new()
}

/// Puts LD ICT's name and fixed settings in place. Safe to call more than once.
pub fn apply() {
    *config::APP_NAME.write().unwrap() = APP_NAME.to_owned();
    let mut overwrite = config::OVERWRITE_SETTINGS.write().unwrap();
    for (key, value) in OVERRIDE {
        overwrite.insert((*key).to_owned(), (*value).to_owned());
    }
    drop(overwrite);
    let mut builtin = config::BUILTIN_SETTINGS.write().unwrap();
    for (key, value) in BUILTIN {
        builtin.insert((*key).to_owned(), (*value).to_owned());
    }
    drop(builtin);
    let mut hard = config::HARD_SETTINGS.write().unwrap();
    for (key, value) in HARD {
        hard.insert((*key).to_owned(), (*value).to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_a_32_byte_public_key() {
        if KEY_TRIMMED.is_empty() {
            // Not set yet: the release workflow refuses to build, everything else may go on.
            return;
        }
        use hbb_common::base64::{engine::general_purpose::STANDARD, Engine};
        let bytes = STANDARD.decode(KEY_TRIMMED).expect("ldict/key.pub is base64");
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn apply_sets_server_key_and_name() {
        apply();
        assert_eq!(*config::APP_NAME.read().unwrap(), APP_NAME);
        let overwrite = config::OVERWRITE_SETTINGS.read().unwrap();
        assert_eq!(overwrite.get(keys::OPTION_CUSTOM_RENDEZVOUS_SERVER).unwrap(), SERVER);
        assert_eq!(overwrite.get(keys::OPTION_KEY).unwrap(), KEY_TRIMMED);
        assert_eq!(overwrite.get(keys::OPTION_APPROVE_MODE).unwrap(), "click");
    }

    #[test]
    fn display_name_follows_the_language() {
        assert_eq!(display_name_for("nl"), "LD ICT Hulp");
        assert_eq!(display_name_for("nl-NL"), "LD ICT Hulp");
        assert_eq!(display_name_for("en"), "LD ICT Help");
        assert_eq!(display_name_for("de"), "LD ICT Help");
        assert_eq!(display_name_for(""), "LD ICT Help");
    }

    #[test]
    fn app_name_fits_a_uri_scheme() {
        assert!(APP_NAME.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }
}
