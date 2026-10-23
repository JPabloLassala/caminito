//! Remap templates (CMN-189): named, saved sets of Key remaps, one text file each, named
//! `<slug>.remaps`. Those that come with Caminito are in `/usr/share/caminito/remap-templates/`
//! and can't be changed or deleted; the user's are in
//! `~/.local/share/caminito/remap-templates/`.
//!
//! A file is a `# name:` line, comments of `#`, and a section for All applications and one
//! for each Remap group, its name in brackets and its app ids after them. A section's lines
//! are its rules, `Alt+T = Ctrl+T`, `Alt+BackSpace = Shift+Home, BackSpace` or
//! `Alt+H = nothing`, and a group's may say `no remaps`.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::key_remaps::{self, Combination, Group, Remaps, Rule, Scope, target_text};

#[cfg(test)]
mod tests;

/// Where the templates that come with Caminito are.
const SHIPPED: &str = "/usr/share/caminito/remap-templates";
/// A template file's extension.
const EXTENSION: &str = "remaps";
/// The section of All applications' rules.
const ALL_APPLICATIONS: &str = "All applications";
/// The line that turns a group's remaps off.
const NO_REMAPS: &str = "no remaps";
/// The comment that names the template.
const NAME: &str = "# name:";

/// A template.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub name: String,
    /// Whether it comes with Caminito, so it can't be changed or deleted.
    pub shipped: bool,
    pub path: PathBuf,
    pub remaps: Remaps,
}

/// Why a template's text can't be read, at a line counted from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub cause: LineError,
}

/// What's wrong with a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineError {
    /// A line that's no section, no rule and no `no remaps`.
    NoRule(String),
    /// A `[` with no `]`.
    UnclosedSection,
    /// `no remaps` in All applications', which can't be turned off.
    AllApplicationsOff,
    /// A rule's combination or target that can't be read.
    Combination(key_remaps::ParseError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: ", self.line)?;
        match &self.cause {
            LineError::NoRule(text) => write!(f, "“{text}” isn't a rule"),
            LineError::UnclosedSection => write!(f, "a group's name has no closing ]"),
            LineError::AllApplicationsOff => {
                write!(f, "{ALL_APPLICATIONS} can't have {NO_REMAPS}")
            }
            LineError::Combination(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Reads a template's text: its name, if a `# name:` line gives one, and its remaps.
pub fn parse(text: &str) -> Result<(Option<String>, Remaps), ParseError> {
    let mut name = None;
    let mut remaps = Remaps::default();
    // Rules before any section are All applications'.
    let mut scope = Scope::AllApplications;
    for (index, line) in text.lines().enumerate() {
        let error = |cause| ParseError {
            line: index + 1,
            cause,
        };
        let line = line.trim();
        if let Some(named) = line.strip_prefix(NAME) {
            name.get_or_insert_with(|| named.trim().to_owned());
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[') {
            let (group, app_ids) = section
                .split_once(']')
                .ok_or(error(LineError::UnclosedSection))?;
            let group = group.trim();
            scope = if group == ALL_APPLICATIONS {
                Scope::AllApplications
            } else {
                let index = remaps.add_group(group);
                remaps.groups[index].app_ids =
                    app_ids.split_whitespace().map(str::to_owned).collect();
                Scope::Group(index)
            };
            continue;
        }
        if line.eq_ignore_ascii_case(NO_REMAPS) {
            match scope {
                Scope::AllApplications => return Err(error(LineError::AllApplicationsOff)),
                Scope::Group(index) => remaps.set_no_remaps(index, true),
            }
            continue;
        }
        let (combination, target) = line
            .split_once('=')
            .ok_or_else(|| error(LineError::NoRule(line.to_owned())))?;
        let rule = Rule {
            combination: Combination::parse(combination)
                .map_err(|err| error(LineError::Combination(err)))?,
            steps: key_remaps::parse_target(target)
                .map_err(|err| error(LineError::Combination(err)))?,
        };
        remaps.set_rule(scope, rule);
    }
    Ok((name, remaps))
}

/// The text of a template named `name` of `remaps`, which [`parse`] reads back.
pub fn write(name: &str, remaps: &Remaps) -> String {
    fn rules(text: &mut String, rules: &[Rule]) {
        for rule in rules {
            let target = target_text(&rule.steps);
            text.push_str(&format!("{} = {target}\n", rule.combination));
        }
    }
    let mut text = format!("{NAME} {name}\n[{ALL_APPLICATIONS}]\n");
    rules(&mut text, &remaps.all);
    for Group {
        name,
        app_ids,
        no_remaps,
        rules: group_rules,
    } in &remaps.groups
    {
        text.push_str(&format!("\n[{name}]"));
        for app_id in app_ids {
            text.push_str(&format!(" {app_id}"));
        }
        text.push('\n');
        if *no_remaps {
            text.push_str(&format!("{NO_REMAPS}\n"));
        }
        rules(&mut text, group_rules);
    }
    text
}

/// The file name a template named `name` is saved under, without its extension: its letters
/// and digits in lower case, the rest as one `-` between them.
pub fn slug(name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    words.join("-")
}

/// The first of `templates` whose remaps are `remaps`, so the page knows whether the remaps
/// in use are saved in one.
pub fn holding<'a>(templates: &'a [Template], remaps: &Remaps) -> Option<&'a Template> {
    templates.iter().find(|template| template.remaps == *remaps)
}

/// Why a template can't be read, saved or deleted.
#[derive(Debug)]
pub enum Error {
    /// A template of this name comes with Caminito.
    Shipped(String),
    /// A name with no letter or digit, which gives no file name.
    NoName(String),
    /// The file or its directory can't be read.
    Read(PathBuf, io::Error),
    /// The file isn't a template.
    Parse(PathBuf, ParseError),
    /// The file or its directory can't be written, or removed.
    Write(PathBuf, io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Shipped(name) => write!(f, "“{name}” comes with Caminito"),
            Error::NoName(name) => write!(f, "“{name}” has no letter or digit"),
            Error::Read(path, err) => write!(f, "can't read {}: {err}", path.display()),
            Error::Parse(path, err) => write!(f, "{}: {err}", path.display()),
            Error::Write(path, err) => write!(f, "can't write {}: {err}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

/// The two directories of templates.
pub struct Templates {
    shipped: PathBuf,
    user: PathBuf,
}

impl Templates {
    /// The templates of `shipped`, Caminito's, and `user`, the user's.
    pub fn new(shipped: PathBuf, user: PathBuf) -> Self {
        Templates { shipped, user }
    }

    /// The installed templates, and the user's in their data directory.
    pub fn installed() -> Self {
        let user = gio::glib::user_data_dir().join("caminito/remap-templates");
        Templates::new(PathBuf::from(SHIPPED), user)
    }

    /// Caminito's templates, then the user's, each by name, and the files that couldn't be
    /// read. A directory that isn't there has none.
    pub fn list(&self) -> (Vec<Template>, Vec<Error>) {
        let mut problems = Vec::new();
        let mut shipped = read_dir(&self.shipped, true, &mut problems);
        let mut user = read_dir(&self.user, false, &mut problems);
        for templates in [&mut shipped, &mut user] {
            templates.sort_by_key(|template| template.name.to_lowercase());
        }
        shipped.append(&mut user);
        (shipped, problems)
    }

    /// Saves `remaps` as the user's template `name`, in place of the user's file of its slug.
    /// A name one of Caminito's has is refused.
    pub fn save_as(&self, name: &str, remaps: &Remaps) -> Result<Template, Error> {
        let name = name.trim();
        let (listed, _) = self.list();
        let shipped = listed
            .iter()
            .any(|template| template.shipped && template.name.eq_ignore_ascii_case(name));
        if shipped {
            return Err(Error::Shipped(name.to_owned()));
        }
        let slug = slug(name);
        if slug.is_empty() {
            return Err(Error::NoName(name.to_owned()));
        }
        fs::create_dir_all(&self.user).map_err(|err| Error::Write(self.user.clone(), err))?;
        let path = self.user.join(format!("{slug}.{EXTENSION}"));
        fs::write(&path, write(name, remaps)).map_err(|err| Error::Write(path.clone(), err))?;
        Ok(Template {
            name: name.to_owned(),
            shipped: false,
            path,
            remaps: remaps.clone(),
        })
    }

    /// Deletes the user's `template`. Caminito's are refused.
    pub fn delete(&self, template: &Template) -> Result<(), Error> {
        if template.shipped {
            return Err(Error::Shipped(template.name.clone()));
        }
        fs::remove_file(&template.path).map_err(|err| Error::Write(template.path.clone(), err))
    }
}

/// The templates in `dir`, each marked `shipped` or not, with the files that can't be read
/// added to `problems`.
fn read_dir(dir: &Path, shipped: bool, problems: &mut Vec<Error>) -> Vec<Template> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            problems.push(Error::Read(dir.to_owned(), err));
            return Vec::new();
        }
    };
    let mut templates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_none_or(|extension| extension != EXTENSION)
        {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                problems.push(Error::Read(path, err));
                continue;
            }
        };
        match parse(&text) {
            Ok((name, remaps)) => {
                let stem = || {
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into()
                };
                templates.push(Template {
                    name: name.unwrap_or_else(stem),
                    shipped,
                    path,
                    remaps,
                });
            }
            Err(err) => problems.push(Error::Parse(path, err)),
        }
    }
    templates
}
