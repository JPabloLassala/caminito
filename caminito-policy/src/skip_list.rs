//! `show-desktop-skip-list` (164): the apps whose windows stay on screen when the desktop is
//! shown.

/// The apps `show-desktop-skip-list` names, as Marco reads the key: a comma-separated list,
/// each entry stripped of whitespace at its ends (`show_desktop_skip_list_handler`,
/// prefs.c:1317-1342), matched without regard to ASCII case (`meta_prefs_is_in_skip_list`,
/// prefs.c:1218-1227).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShowDesktopSkipList(Vec<String>);

impl ShowDesktopSkipList {
    /// The list a value of the key names. An empty entry names nothing.
    pub fn parse(value: &str) -> Self {
        Self(
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_owned)
                .collect(),
        )
    }

    /// Whether a window with `app_id`, a native window's app id or an XWayland window's
    /// `WM_CLASS` class, is listed.
    pub fn matches(&self, app_id: Option<&str>) -> bool {
        app_id.is_some_and(|app_id| {
            self.0
                .iter()
                .any(|entry| entry.eq_ignore_ascii_case(app_id))
        })
    }
}
