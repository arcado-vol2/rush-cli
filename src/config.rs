use std::{fs, io, path::{Path, PathBuf}};
use rush_lib::{Config, DocOut, DocStyle, Theme};
use serde::{Deserialize, Serialize};

pub(crate) const PROJECT_FILE: &str = "project.toml";

fn style_from_name(name: &str) -> Option<DocStyle> {
    match name {
        "gost_7_32" => Some(DocStyle::Gost7_32),
        "modern" => Some(DocStyle::Modern),
        "blog" => Some(DocStyle::Blog),
        _ => None,
    }
}

fn theme_from_name(name: &str) -> Option<Theme> {
    match name {
        "auto" => Some(Theme::Auto),
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    }
}

pub(crate) fn theme_name(theme: Theme) -> &'static str {
    match theme {
        Theme::Auto => "auto",
        Theme::Light => "light",
        Theme::Dark => "dark",
    }
}

fn out_from_name(name: &str) -> Option<DocOut> {
    match name {
        "html" => Some(DocOut::Html),
        "markdown" => Some(DocOut::Markdown),
        "pdf" => Some(DocOut::Pdf),
        "odt" => Some(DocOut::Odt),
        "docx" => Some(DocOut::Docx),
        _ => None
    }
}

fn style_name(style: DocStyle) -> &'static str {
    match style {
        DocStyle::Gost7_32 => "gost_7_32",
        DocStyle::Modern => "modern",
        DocStyle::Blog => "blog",
    }
}

fn out_name(out: DocOut) -> &'static str {
    match out {
        DocOut::Html => "html",
        DocOut::Markdown => "markdown",
        DocOut::Pdf => "pdf",
        DocOut::Odt => "odt",
        DocOut::Docx => "docx",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProjectFile {
    pub(crate) config: Config,
    pub(crate) input: PathBuf,
    pub(crate) output: PathBuf,
    pub(crate) content: Vec<ContentEntry>,
    pub(crate) dirs: Vec<ContentEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ContentEntry {
    pub(crate) name: String,
    pub(crate) path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Named {
    Content(usize),
    Dir(usize),
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self {
            config: Config::default(),
            input: PathBuf::from("src"),
            output: PathBuf::from("out"),
            content: Vec::new(),
            dirs: Vec::new(),
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ConfigWarning {
    _Unreadable(String),
    InvalidToml(String),
    WrongType { key: &'static str },
    UnknownValue { key: &'static str, value: String },
    EmptyPath { key: &'static str },
    ContentMissingKey { index: usize, key: &'static str },
    ContentWrongType { index: usize, key: &'static str },
    ContentEmptyPath { index: usize },
    DirsMissingKey { index: usize, key: &'static str },
    DirsWrongType { index: usize, key: &'static str },
    DirsEmptyPath { index: usize },
}


#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct RawProjectFile {
    document: RawDocument,
    build: RawBuild,
    content: Option<toml::Value>,
    dirs: Option<toml::Value>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct RawDocument {
    style: Option<toml::Value>,
    out: Option<toml::Value>,
    single: Option<toml::Value>,
    no_js: Option<toml::Value>,
    theme: Option<toml::Value>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct RawBuild {
    input: Option<toml::Value>,
    output: Option<toml::Value>
}

#[derive(Serialize)]
struct OutProjectFile<'a> {
    document: OutDocument,
    build: OutBuild,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    content: &'a Vec<ContentEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dirs: &'a Vec<ContentEntry>,
}

#[derive(Serialize)]
struct OutDocument {
    style: &'static str,
    out: &'static str,
    single: bool,
    no_js: bool,
    theme: &'static str,
}

#[derive(Serialize)]
struct OutBuild {
    input: String,
    output: String,
}

fn relative_to(path: &Path, root: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned();
    if rel.is_empty() { ".".into() } else { rel }
}

impl ProjectFile {
    pub(crate) fn save(&self, root: &Path) -> io::Result<()> {
        fs::write(root.join(PROJECT_FILE), self.to_toml(root))
    }

    pub(crate) fn to_toml(&self, root: &Path) -> String {
        let raw = OutProjectFile {
            document: OutDocument {
                style: style_name(self.config.style),
                out: out_name(self.config.out),
                single: self.config.single,
                no_js: self.config.no_js,
                theme: theme_name(self.config.theme),
            },
            build: OutBuild {
                input: relative_to(&self.input, root),
                output: relative_to(&self.output, root),
            },
            content: &self.content,
            dirs: &self.dirs,
        };
        toml::to_string(&raw).expect("ProjectFile must be serialesable  to TOML")
    }

    pub(crate) fn load(root: &Path) -> io::Result<(ProjectFile, Vec<ConfigWarning>)> {
        let (mut project, warnings) = Self::parse(&fs::read_to_string(root.join(PROJECT_FILE))?);
        project.input = root.join(&project.input);
        project.output = root.join(&project.output);
        Ok((project, warnings))
    }

    pub(crate) fn parse(src: &str) -> (ProjectFile, Vec<ConfigWarning>) {
        let mut warnings = Vec::new();
        let raw: RawProjectFile = match toml::from_str(src) {
            Ok(raw) => raw,
            Err(e) => {
                warnings.push(ConfigWarning::InvalidToml(e.to_string()));
                return (ProjectFile::default(), warnings);
            }
        };
        let mut out = ProjectFile::default();
        if let Some(style) = enum_param(raw.document.style, "document.style", style_from_name, &mut warnings) {
            out.config.style = style;
        }
        if let Some(doc_out) = enum_param(raw.document.out, "document.out", out_from_name, &mut warnings) {
            out.config.out = doc_out;
        }
        if let Some(single) = bool_param(raw.document.single, "document.single", &mut warnings) {
            out.config.single = single;
        }
        if let Some(no_js) = bool_param(raw.document.no_js, "document.no_js", &mut warnings) {
            out.config.no_js = no_js;
        }
        if let Some(theme) = enum_param(raw.document.theme, "document.theme", theme_from_name, &mut warnings) {
            out.config.theme = theme;
        }
        if let Some(input) = path_param(raw.build.input, "build.input", &mut warnings) {
            out.input = input;
        }
        if let Some(output) = path_param(raw.build.output, "build.output", &mut warnings) {
            out.output = output;
        }
        if let Some(content) = raw.content {
            out.content = entries_param(content, "content", &mut warnings);
        }
        if let Some(dirs) = raw.dirs {
            out.dirs = entries_param(dirs, "dirs", &mut warnings);
        }
        (out, warnings)
    }

    pub(crate) fn find_name(&self, name: &str) -> Vec<Named> {
        let content = self.content.iter().enumerate().filter(|(_, e)| e.name == name).map(|(i, _)| Named::Content(i));
        let dirs = self.dirs.iter().enumerate().filter(|(_, e)| e.name == name).map(|(i, _)| Named::Dir(i));
        content.chain(dirs).collect()
    }

    pub(crate) fn entry_mut(&mut self, at: Named) -> &mut ContentEntry {
        match at {
            Named::Content(i) => &mut self.content[i],
            Named::Dir(i) => &mut self.dirs[i],
        }
    }
    
    pub(crate) fn remove_content_by_name(&mut self, name: &str) -> Option<(usize, ContentEntry)> {
        let pos = self.content.iter().position(|entry| entry.name == name)?;
        Some((pos, self.content.remove(pos)))
    }
}

fn path_param(
    value: Option<toml::Value>,
    key: &'static str,
    warnings: &mut Vec<ConfigWarning>,
) -> Option<PathBuf> {
    match value? {
        toml::Value::String(s) if s.is_empty() => {
            warnings.push(ConfigWarning::EmptyPath { key });
            None
        }
        toml::Value::String(s) => Some(PathBuf::from(s)),
        _ => {
            warnings.push(ConfigWarning::WrongType { key });
            None
        }
    }
}

fn bool_param(value: Option<toml::Value>, key: &'static str, warnings: &mut Vec<ConfigWarning>) -> Option<bool> {
    match value? {
        toml::Value::Boolean(b) => Some(b),
        _ => {
            warnings.push(ConfigWarning::WrongType { key });
            None
        }
    }
}

fn entries_param(value: toml::Value, key: &'static str, warnings: &mut Vec<ConfigWarning>) -> Vec<ContentEntry> {
    let toml::Value::Array(items) = value else {
        warnings.push(ConfigWarning::WrongType { key });
        return Vec::new();
    };
    let dirs = key == "dirs";
    let mut out = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let toml::Value::Table(mut table) = item else {
            warnings.push(ConfigWarning::WrongType { key });
            continue;
        };
        let mut get = |field: &'static str| match table.remove(field) {
            Some(toml::Value::String(s)) => Some(s),
            Some(_) => {
                warnings.push(if dirs { ConfigWarning::DirsWrongType { index, key: field } } else { ConfigWarning::ContentWrongType { index, key: field } });
                None
            }
            None => {
                warnings.push(if dirs { ConfigWarning::DirsMissingKey { index, key: field } } else { ConfigWarning::ContentMissingKey { index, key: field } });
                None
            }
        };
        let (name, path) = (get("name"), get("path"));
        let (Some(name), Some(mut path)) = (name, path) else { continue };
        if dirs {
            path = path.trim_end_matches(['/', '\\']).to_string();
        }
        if path.is_empty() {
            warnings.push(if dirs { ConfigWarning::DirsEmptyPath { index } } else { ConfigWarning::ContentEmptyPath { index } });
            continue;
        }
        out.push(ContentEntry { name, path });
    }
    out
}

fn enum_param<T>(
    value: Option<toml::Value>,
    key: &'static str,
    from_name: fn(&str) -> Option<T>,
    warnings: &mut Vec<ConfigWarning>,
) -> Option<T> {
    match value? {
        toml::Value::String(s) => {
            let parsed = from_name(&s);
            if parsed.is_none() {
                warnings.push(ConfigWarning::UnknownValue { key, value: s });
            }
            parsed
        }
        _ => {
            warnings.push(ConfigWarning::WrongType { key });
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(src: &str, style: DocStyle, warnings: Vec<ConfigWarning>) {
        let (out, w) = ProjectFile::parse(src);
        assert_eq!(out.config.style, style, "вход: {src:?}");
        assert_eq!(w, warnings, "вход: {src:?}");
    }

    #[test]
    fn valid_values() {
        check("[document]\nstyle = \"gost_7_32\"", DocStyle::Gost7_32, vec![]);
        check("[document]\nstyle = \"modern\"", DocStyle::Modern, vec![]);
        check("[document]\nstyle = \"blog\"", DocStyle::Blog, vec![]);
    }

    #[test]
    fn missing_is_default_without_warnings() {
        check("", DocStyle::Blog, vec![]);
        check("[document]", DocStyle::Blog, vec![]);
        check("[other]\nx = 1", DocStyle::Blog, vec![]);
    }

    #[test]
    fn unknown_value_falls_back() {
        check(
            "[document]\nstyle = \"Gost\"",
            DocStyle::Blog,
            vec![ConfigWarning::UnknownValue { key: "document.style", value: "Gost".into() }],
        );
    }

    #[test]
    fn wrong_type_falls_back() {
        check("[document]\nstyle = 5", DocStyle::Blog, vec![ConfigWarning::WrongType { key: "document.style" }]);
        check("[document]\nstyle = \"light\"", DocStyle::Blog, vec![ConfigWarning::UnknownValue { key: "document.style", value: "light".into() }]);
    }

    #[test]
    fn broken_toml_falls_back() {
        let (out, w) = ProjectFile::parse("[document\nstyle = ");
        assert_eq!(out, ProjectFile::default());
        assert!(matches!(w.as_slice(), [ConfigWarning::InvalidToml(_)]));
    }

    fn check_out(src: &str, doc_out: DocOut, warnings: Vec<ConfigWarning>) {
        let (out, w) = ProjectFile::parse(src);
        assert_eq!(out.config.out, doc_out, "вход: {src:?}");
        assert_eq!(w, warnings, "вход: {src:?}");
    }

    #[test]
    fn out_valid_values() {
        check_out("[document]
out = \"html\"", DocOut::Html, vec![]);
        check_out("[document]
out = \"markdown\"", DocOut::Markdown, vec![]);
        check_out("[document]
out = \"pdf\"", DocOut::Pdf, vec![]);
        check_out("[document]
out = \"odt\"", DocOut::Odt, vec![]);
        check_out("[document]
out = \"docx\"", DocOut::Docx, vec![]);
    }

    #[test]
    fn out_missing_is_html() {
        check_out("", DocOut::Html, vec![]);
        check_out("[document]
style = \"gost_7_32\"", DocOut::Html, vec![]);
    }

    #[test]
    fn out_bad_falls_back_to_html() {
        check_out(
            "[document]
out = \"exe\"",
            DocOut::Html,
            vec![ConfigWarning::UnknownValue { key: "document.out", value: "exe".into() }],
        );
        check_out("[document]
out = true", DocOut::Html, vec![ConfigWarning::WrongType { key: "document.out" }]);
    }

    #[test]
    fn style_and_out_together() {
        let (out, w) = ProjectFile::parse("[document]
style = \"modern\"
out = \"pdf\"");
        assert_eq!(out, ProjectFile { config: Config { style: DocStyle::Modern, out: DocOut::Pdf, ..Config::default() }, ..ProjectFile::default() });
        assert!(w.is_empty());
    }

    #[test]
    fn single_value() {
        let (out, w) = ProjectFile::parse("[document]\nsingle = true");
        assert!(out.config.single);
        assert!(w.is_empty());
        assert!(!ProjectFile::parse("").0.config.single);
    }

    #[test]
    fn single_wrong_type_falls_back() {
        let (out, w) = ProjectFile::parse("[document]\nsingle = \"да\"");
        assert!(!out.config.single);
        assert_eq!(w, vec![ConfigWarning::WrongType { key: "document.single" }]);
    }

    #[test]
    fn html_padges_is_unknown() {
        check_out(
            "[document]\nout = \"html-padges\"",
            DocOut::Html,
            vec![ConfigWarning::UnknownValue { key: "document.out", value: "html-padges".into() }],
        );
    }



    fn check_paths(src: &str, input: &str, output: &str, warnings: Vec<ConfigWarning>) {
        let (out, w) = ProjectFile::parse(src);
        assert_eq!(out.input, PathBuf::from(input), "вход: {src:?}");
        assert_eq!(out.output, PathBuf::from(output), "вход: {src:?}");
        assert_eq!(w, warnings, "вход: {src:?}");
    }

    #[test]
    fn build_paths() {
        check_paths("", "src", "out", vec![]);
        check_paths("[build]\ninput = \"docs\"\noutput = \"dist/html\"", "docs", "dist/html", vec![]);
        check_paths("[build]\ninput = 1\noutput = \"\"", "src", "out", vec![
            ConfigWarning::WrongType { key: "build.input" },
            ConfigWarning::EmptyPath { key: "build.output" },
        ]);
    }

    fn entry(name: &str, path: &str) -> ContentEntry {
        ContentEntry { name: name.into(), path: path.into() }
    }

    #[test]
    fn content_valid() {
        let (out, w) = ProjectFile::parse("[[content]]
name = \"Введение\"
path = \"intro\"
[[content]]
name = \"Установка\"
path = \"guide/install.rush\"");
        assert_eq!(out.content, vec![entry("Введение", "intro"), entry("Установка", "guide/install.rush")]);
        assert!(w.is_empty());
    }

    #[test]
    fn content_missing_is_empty() {
        let (out, w) = ProjectFile::parse("[document]
style = \"gost_7_32\"");
        assert!(out.content.is_empty());
        assert!(w.is_empty());
    }

    #[test]
    fn content_bad_entries_skipped() {
        let (out, w) = ProjectFile::parse("[[content]]
path = \"a\"
[[content]]
name = 1
path = \"b\"
[[content]]
name = \"C\"
path = \"\"
[[content]]
name = \"D\"
path = \"d\"");
        assert_eq!(out.content, vec![entry("D", "d")]);
        assert_eq!(w, vec![
            ConfigWarning::ContentMissingKey { index: 0, key: "name" },
            ConfigWarning::ContentWrongType { index: 1, key: "name" },
            ConfigWarning::ContentEmptyPath { index: 2 },
        ]);
    }

    #[test]
    fn content_wrong_type() {
        let (out, w) = ProjectFile::parse("content = \"a\"");
        assert!(out.content.is_empty());
        assert_eq!(w, vec![ConfigWarning::WrongType { key: "content" }]);
    }

    #[test]
    fn build_paths_resolve_from_root() {
        let root = std::env::temp_dir().join("rush_cli_build_paths");
        fs::create_dir_all(&root).unwrap();
        let abs = std::env::temp_dir().join("abs_out");
        let src = format!("[build]\ninput = \"text\"\noutput = '{}'", abs.display());
        fs::write(root.join(PROJECT_FILE), src).unwrap();

        let (out, w) = ProjectFile::load(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(out.input, root.join("text"));
        assert_eq!(out.output, abs);
        assert!(w.is_empty());
    }

    #[test]
    fn empty_content_not_serialized() {
        let root = PathBuf::from("root");
        let src = ProjectFile::default().to_toml(&root);
        assert!(!src.contains("content"), "{src}");

        let appended = format!("{src}\n[[content]]\nname = \"A\"\npath = \"a.rush\"\n");
        let (out, w) = ProjectFile::parse(&appended);
        assert_eq!(out.content, vec![entry("A", "a.rush")]);
        assert!(w.is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let root = std::env::temp_dir().join("rush_cli_save_roundtrip");
        fs::create_dir_all(&root).unwrap();
        let abs = std::env::temp_dir().join("abs_out");
        let project = ProjectFile {
            config: Config { style: DocStyle::Blog, out: DocOut::Html, single: true, no_js: true, theme: Theme::Dark },
            input: root.join("text"),
            output: abs.clone(),
            content: vec![entry("Введение", "intro"), entry("Установка", "guide/install.rush")],
            dirs: vec![entry("Руководство", "guide")],
        };

        project.save(&root).unwrap();
        let saved = fs::read_to_string(root.join(PROJECT_FILE)).unwrap();
        let (loaded, w) = ProjectFile::load(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert!(saved.contains("input = \"text\""), "{saved}");
        assert_eq!(loaded, project);
        assert!(w.is_empty());
    }

    #[test]
    fn no_js_and_theme() {
        let (out, w) = ProjectFile::parse("[document]\nno_js = true\ntheme = \"dark\"");
        assert!(out.config.no_js);
        assert_eq!(out.config.theme, Theme::Dark);
        assert!(w.is_empty());

        let (out, _) = ProjectFile::parse("");
        assert!(!out.config.no_js);
        assert_eq!(out.config.theme, Theme::Auto);

        let (out, w) = ProjectFile::parse("[document]\nno_js = \"да\"\ntheme = \"night\"");
        assert!(!out.config.no_js);
        assert_eq!(out.config.theme, Theme::Auto);
        assert_eq!(w, vec![
            ConfigWarning::WrongType { key: "document.no_js" },
            ConfigWarning::UnknownValue { key: "document.theme", value: "night".into() },
        ]);
    }

    #[test]
    fn dirs_entries() {
        let (out, w) = ProjectFile::parse("[[dirs]]\nname = \"Руководство\"\npath = \"guide/\"\n[[dirs]]\npath = \"a\"\n[[dirs]]\nname = \"B\"\npath = 2\n[[dirs]]\nname = \"C\"\npath = \"/\"");
        assert_eq!(out.dirs, vec![entry("Руководство", "guide")]);
        assert_eq!(w, vec![
            ConfigWarning::DirsMissingKey { index: 1, key: "name" },
            ConfigWarning::DirsWrongType { index: 2, key: "path" },
            ConfigWarning::DirsEmptyPath { index: 3 },
        ]);
    }

    #[test]
    fn find_name_in_content_then_dirs() {
        let project = ProjectFile {
            content: vec![entry("A", "a"), entry("B", "b")],
            dirs: vec![entry("A", "x"), entry("C", "c")],
            ..ProjectFile::default()
        };
        assert_eq!(project.find_name("A"), vec![Named::Content(0), Named::Dir(0)]);
        assert_eq!(project.find_name("C"), vec![Named::Dir(1)]);
        assert!(project.find_name("Z").is_empty());
    }

    #[test]
    fn project_root_saved_as_dot() {
        let root = PathBuf::from("root");
        let project = ProjectFile { input: root.clone(), output: root.join("out"), ..ProjectFile::default() };
        let src = project.to_toml(&root);
        assert!(src.contains("input = \".\""), "{src}");
        let (out, w) = ProjectFile::parse(&src);
        assert_eq!(out.input, PathBuf::from("."));
        assert!(w.is_empty(), "{w:?}");
    }
}
