use std::{fs, io, path::{Path, PathBuf}};
use rush_lib::{Config, DocOut, DocStyle};
use serde::{Deserialize, Serialize};

pub(crate) const PROJECT_FILE: &str = "project.toml";

fn style_from_name(name: &str) -> Option<DocStyle> {
    match name {
        "gost_7_32" => Some(DocStyle::Gost7_32),
        "modern" => Some(DocStyle::Modern),
        "light" => Some(DocStyle::Light),
        _ => None,
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
        DocStyle::Light => "light",
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
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ContentEntry {
    pub(crate) name: String,
    pub(crate) path: String,
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self {
            config: Config::default(),
            input: PathBuf::from("src"),
            output: PathBuf::from("out"),
            content: Vec::new(),
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
}


#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct RawProjectFile {
    document: RawDocument,
    build: RawBuild,
    content: Option<toml::Value>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct RawDocument {
    style: Option<toml::Value>,
    out: Option<toml::Value>,
    single: Option<toml::Value>,
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
}

#[derive(Serialize)]
struct OutDocument {
    style: &'static str,
    out: &'static str,
    single: bool,
}

#[derive(Serialize)]
struct OutBuild {
    input: String,
    output: String,
}

fn relative_to(path: &Path, root: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned()
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
            },
            build: OutBuild {
                input: relative_to(&self.input, root),
                output: relative_to(&self.output, root),
            },
            content: &self.content,
        };
        toml::to_string(&raw).expect("ProjectFile всегда сериализуется в TOML")
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
        if let Some(input) = path_param(raw.build.input, "build.input", &mut warnings, true) {
            out.input = input;
        }
        if let Some(output) = path_param(raw.build.output, "build.output", &mut warnings, true) {
            out.output = output;
        }
        if let Some(content) = raw.content {
            out.content = content_param(content, &mut warnings);
        }
        (out, warnings)
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
    allow_empty: bool,
) -> Option<PathBuf> {
    match value? {
        toml::Value::String(s) if s.is_empty() && !allow_empty => {
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

fn content_param(value: toml::Value, warnings: &mut Vec<ConfigWarning>) -> Vec<ContentEntry> {
    let toml::Value::Array(items) = value else {
        warnings.push(ConfigWarning::WrongType { key: "content" });
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let toml::Value::Table(mut table) = item else {
            warnings.push(ConfigWarning::WrongType { key: "content" });
            continue;
        };
        let mut get = |key: &'static str| match table.remove(key) {
            Some(toml::Value::String(s)) => Some(s),
            Some(_) => {
                warnings.push(ConfigWarning::ContentWrongType { index, key });
                None
            }
            None => {
                warnings.push(ConfigWarning::ContentMissingKey { index, key });
                None
            }
        };
        let (name, path) = (get("name"), get("path"));
        let (Some(name), Some(path)) = (name, path) else { continue };
        if path.is_empty() {
            warnings.push(ConfigWarning::ContentEmptyPath { index });
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
        check("[document]\nstyle = \"light\"", DocStyle::Light, vec![]);
    }

    #[test]
    fn missing_is_default_without_warnings() {
        check("", DocStyle::Gost7_32, vec![]);
        check("[document]", DocStyle::Gost7_32, vec![]);
        check("[other]\nx = 1", DocStyle::Gost7_32, vec![]);
    }

    #[test]
    fn unknown_value_falls_back() {
        check(
            "[document]\nstyle = \"Gost\"",
            DocStyle::Gost7_32,
            vec![ConfigWarning::UnknownValue { key: "document.style", value: "Gost".into() }],
        );
    }

    #[test]
    fn wrong_type_falls_back() {
        check("[document]\nstyle = 5", DocStyle::Gost7_32, vec![ConfigWarning::WrongType { key: "document.style" }]);
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
        assert_eq!(out, ProjectFile { config: Config { style: DocStyle::Modern, out: DocOut::Pdf, single: false }, ..ProjectFile::default() });
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
            config: Config { style: DocStyle::Light, out: DocOut::Html, single: true },
            input: root.join("text"),
            output: abs.clone(),
            content: vec![entry("Введение", "intro"), entry("Установка", "guide/install.rush")],
        };

        project.save(&root).unwrap();
        let saved = fs::read_to_string(root.join(PROJECT_FILE)).unwrap();
        let (loaded, w) = ProjectFile::load(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert!(saved.contains("input = \"text\""), "{saved}");
        assert_eq!(loaded, project);
        assert!(w.is_empty());
    }
}
