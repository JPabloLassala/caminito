//! The window menu's words (161): Marco's labels for its items, in its own translations,
//! each with its mnemonic, the names of the workspaces in the submenu, and the keybinding
//! shown beside an item (`menuitems` and `get_workspace_name_with_accel`, menu.c:66-216;
//! `meta_core_get_menu_accelerator`, core.c:549-651).

use caminito_policy::MenuOp;
use gio::glib;

/// Marco's translation domain. Its catalogues are installed with it, which Caminito's
/// packages depend on.
const DOMAIN: &str = "marco";

/// An item's label: its text, and the letter that chooses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Label {
    /// The text as drawn, with no mark of the mnemonic.
    pub(crate) text: String,
    /// The mnemonic: where its letter starts in `text`, in bytes, and the letter in lower
    /// case.
    pub(crate) mnemonic: Option<(usize, char)>,
}

impl Label {
    /// Reads a label in GTK's syntax: an underscore before the mnemonic's letter, and two
    /// for an underscore. The first mnemonic counts, as in GTK.
    pub(crate) fn parse(label: &str) -> Self {
        let mut text = String::with_capacity(label.len());
        let mut mnemonic = None;
        let mut chars = label.chars();
        while let Some(c) = chars.next() {
            if c != '_' {
                text.push(c);
                continue;
            }
            match chars.next() {
                Some('_') => text.push('_'),
                Some(letter) => {
                    if mnemonic.is_none() {
                        let lower = letter.to_lowercase().next().unwrap_or(letter);
                        mnemonic = Some((text.len(), lower));
                    }
                    text.push(letter);
                }
                None => {}
            }
        }
        Self { text, mnemonic }
    }

    /// Whether `typed` is the mnemonic's letter, in either case.
    pub(crate) fn is_chosen_by(&self, typed: char) -> bool {
        let typed = typed.to_lowercase().next().unwrap_or(typed);
        self.mnemonic.is_some_and(|(_, letter)| letter == typed)
    }
}

/// `msgid` in the user's language, as Marco has it. `main` sets the locale gettext reads,
/// and only one whose character set is UTF-8.
fn marco(msgid: &str) -> String {
    glib::dgettext(Some(DOMAIN), msgid).into()
}

/// The label of an item of the menu. `names` are the workspaces', from the first, for the
/// submenu's items.
pub(crate) fn item(op: MenuOp, names: &[String]) -> Label {
    let msgid = match op {
        MenuOp::Minimize => "Mi_nimize",
        MenuOp::Maximize => "Ma_ximize",
        MenuOp::Unmaximize => "Unma_ximize",
        MenuOp::Move => "_Move",
        MenuOp::Resize => "_Resize",
        MenuOp::Recover => "Move Titlebar On_screen",
        MenuOp::ToggleAbove => "Always on _Top",
        MenuOp::Stick => "_Always on Visible Workspace",
        MenuOp::Unstick => "_Only on This Workspace",
        MenuOp::MoveLeft => "Move to Workspace _Left",
        MenuOp::MoveRight => "Move to Workspace R_ight",
        MenuOp::MoveUp => "Move to Workspace _Up",
        MenuOp::MoveDown => "Move to Workspace _Down",
        MenuOp::Close => "_Close",
        MenuOp::MoveToWorkspace(workspace) => {
            let name = names.get(workspace).map_or("", String::as_str);
            return Label::parse(&workspace_label(name, workspace, marco));
        }
    };
    Label::parse(&marco(msgid))
}

/// The label of the submenu's parent.
pub(crate) fn submenu() -> Label {
    Label::parse(&marco("Move to Another _Workspace"))
}

/// The label of workspace `index`, from 0, named `name`, in GTK's syntax, as Marco's
/// `get_workspace_name_with_accel` writes it: a name of the form "Workspace N" has its
/// number for a mnemonic, up to 10, and any other has it after the name, as ` (_N)`, with
/// its underscores kept as they are.
fn workspace_label(name: &str, index: usize, translate: impl Fn(&str) -> String) -> String {
    let pattern = translate("Workspace %d");
    let number = pattern.split_once("%d").and_then(|(before, after)| {
        let digits = name.strip_prefix(before)?.strip_suffix(after)?;
        let all_digits = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
        all_digits.then(|| digits.parse::<u32>().ok()).flatten()
    });
    match number {
        Some(10) => translate("Workspace 1_0"),
        Some(number) => {
            let mnemonic = if number < 10 { "_" } else { "" };
            translate("Workspace %s%d")
                .replacen("%s", mnemonic, 1)
                .replacen("%d", &number.to_string(), 1)
        }
        None => {
            let escaped = name.replace('_', "__");
            match index {
                0..=8 => format!("{escaped} (_{})", index + 1),
                9 => format!("{escaped} (_0)"),
                _ => escaped,
            }
        }
    }
}

/// The keybinding whose combination is shown beside an item, if it has one.
pub(crate) fn binding_key(op: MenuOp) -> Option<String> {
    let key = match op {
        MenuOp::Minimize => "minimize",
        MenuOp::Maximize => "maximize",
        MenuOp::Unmaximize => "unmaximize",
        MenuOp::Move => "begin-move",
        MenuOp::Resize => "begin-resize",
        MenuOp::Recover => return None,
        MenuOp::ToggleAbove => "toggle-above",
        MenuOp::Stick | MenuOp::Unstick => "toggle-on-all-workspaces",
        MenuOp::MoveLeft => "move-to-workspace-left",
        MenuOp::MoveRight => "move-to-workspace-right",
        MenuOp::MoveUp => "move-to-workspace-up",
        MenuOp::MoveDown => "move-to-workspace-down",
        MenuOp::Close => "close",
        // Marco has twelve such keys.
        MenuOp::MoveToWorkspace(workspace) if workspace < 12 => {
            return Some(format!("move-to-workspace-{}", workspace + 1));
        }
        MenuOp::MoveToWorkspace(_) => return None,
    };
    Some(key.to_owned())
}

/// A mouse button as an item's hint names one (184), by X's number for it: the five with a
/// name, and a further side button by its number. Marco's menu shows no button and has no
/// words for one, so they aren't translated, as the modifiers' names aren't.
pub(crate) fn button(button: u32) -> String {
    match button {
        1 => "Left Button".into(),
        2 => "Middle Button".into(),
        3 => "Right Button".into(),
        8 => "Back Button".into(),
        9 => "Forward Button".into(),
        other => format!("Button {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn english(msgid: &str) -> String {
        msgid.to_owned()
    }

    #[test]
    fn a_label_has_its_mnemonic_taken_out_of_its_text() {
        let label = Label::parse("Mi_nimize");
        assert_eq!(label.text, "Minimize");
        assert_eq!(label.mnemonic, Some((2, 'n')));
        assert!(label.is_chosen_by('n') && label.is_chosen_by('N'));
        assert!(!label.is_chosen_by('m'));

        let label = Label::parse("_Close");
        assert_eq!(
            (label.text.as_str(), label.mnemonic),
            ("Close", Some((0, 'c')))
        );
    }

    #[test]
    fn two_underscores_are_one_and_only_the_first_mnemonic_counts() {
        let label = Label::parse("a__b _c _d");
        assert_eq!(label.text, "a_b c d");
        assert_eq!(label.mnemonic, Some((4, 'c')));

        let label = Label::parse("Plain_");
        assert_eq!((label.text.as_str(), label.mnemonic), ("Plain", None));
    }

    #[test]
    fn a_mnemonic_after_letters_of_more_than_one_byte_is_placed_in_bytes() {
        let label = Label::parse("Año _Nuevo");
        assert_eq!(label.text, "Año Nuevo");
        assert_eq!(label.mnemonic, Some((5, 'n')));
    }

    #[test]
    fn a_default_workspace_name_has_its_number_for_a_mnemonic() {
        assert_eq!(workspace_label("Workspace 1", 0, english), "Workspace _1");
        assert_eq!(workspace_label("Workspace 9", 8, english), "Workspace _9");
        assert_eq!(workspace_label("Workspace 10", 9, english), "Workspace 1_0");
        assert_eq!(workspace_label("Workspace 11", 10, english), "Workspace 11");
        // By the number in the name, not the workspace's place.
        assert_eq!(workspace_label("Workspace 3", 0, english), "Workspace _3");
    }

    #[test]
    fn any_other_name_has_the_workspaces_number_after_it() {
        assert_eq!(workspace_label("Mail", 0, english), "Mail (_1)");
        assert_eq!(workspace_label("Mail", 9, english), "Mail (_0)");
        assert_eq!(workspace_label("Mail", 10, english), "Mail");
        assert_eq!(workspace_label("my_work", 1, english), "my__work (_2)");
        assert_eq!(
            workspace_label("Workspace 1b", 2, english),
            "Workspace 1b (_3)"
        );
        assert_eq!(workspace_label("Workspace ", 2, english), "Workspace  (_3)");
    }

    #[test]
    fn a_translated_default_name_is_known_by_its_translation() {
        let spanish = |msgid: &str| match msgid {
            "Workspace %d" => "Área de trabajo %d".to_owned(),
            "Workspace %s%d" => "Área de trabajo %s%d".to_owned(),
            "Workspace 1_0" => "Área de trabajo 1_0".to_owned(),
            other => other.to_owned(),
        };
        assert_eq!(
            workspace_label("Área de trabajo 2", 1, spanish),
            "Área de trabajo _2"
        );
        assert_eq!(
            workspace_label("Workspace 2", 1, spanish),
            "Workspace 2 (_2)"
        );
    }

    #[test]
    fn the_items_have_marcos_labels() {
        let names = ["Workspace 1".to_owned(), "Mail".to_owned()];
        assert_eq!(item(MenuOp::Recover, &names).text, "Move Titlebar Onscreen");
        assert_eq!(item(MenuOp::Recover, &names).mnemonic, Some((16, 's')));
        assert_eq!(item(MenuOp::MoveToWorkspace(0), &names).text, "Workspace 1");
        assert_eq!(item(MenuOp::MoveToWorkspace(1), &names).text, "Mail (2)");
        assert_eq!(submenu().text, "Move to Another Workspace");
    }

    #[test]
    fn the_items_with_a_keybinding_name_it() {
        assert_eq!(binding_key(MenuOp::Move).as_deref(), Some("begin-move"));
        assert_eq!(binding_key(MenuOp::Recover), None);
        assert_eq!(
            binding_key(MenuOp::MoveToWorkspace(11)).as_deref(),
            Some("move-to-workspace-12")
        );
        assert_eq!(binding_key(MenuOp::MoveToWorkspace(12)), None);
    }

    #[test]
    fn a_mouse_button_has_a_name_or_its_number() {
        assert_eq!(button(1), "Left Button");
        assert_eq!(button(2), "Middle Button");
        assert_eq!(button(3), "Right Button");
        assert_eq!(button(8), "Back Button");
        assert_eq!(button(9), "Forward Button");
        assert_eq!(button(10), "Button 10");
        assert_eq!(button(20), "Button 20");
    }
}
