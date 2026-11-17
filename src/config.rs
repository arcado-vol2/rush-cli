use std::{fs, io, path::{Path, PathBuf}};
use rush_lib::{Config, ConfigConst};
use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};

pub(crate) const PROJECT_FILE: &str = "project.yaml";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProjectFile {
    pub(crate) config: Config,
    pub(crate) input: PathBuf,
    pub(crate) output: PathBuf,
    pub(crate) content: Vec<ContentEntry>,
}

/// File or folder of the menu. `path` is always relative to the source root,
/// also for entries nested in a folder.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ContentEntry {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) kind: ContentEntryKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ContentEntryKind {
    File,
    Dir { childs: Vec<ContentEntry> },
}

impl ContentEntry {
    pub(crate) fn file(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self { name: name.into(), path: path.into(), kind: ContentEntryKind::File }
    }

    pub(crate) fn dir(name: impl Into<String>, path: impl Into<String>, childs: Vec<ContentEntry>) -> Self {
        Self { name: name.into(), path: path.into(), kind: ContentEntryKind::Dir { childs } }
    }

    pub(crate) fn is_dir(&self) -> bool {
        matches!(self.kind, ContentEntryKind::Dir { .. })
    }

    pub(crate) fn childs(&self) -> Option<&Vec<ContentEntry>> {
        match &self.kind {
            ContentEntryKind::Dir { childs } => Some(childs),
            ContentEntryKind::File => None,
        }
    }

    fn childs_mut(&mut self) -> Option<&mut Vec<ContentEntry>> {
        match &mut self.kind {
            ContentEntryKind::Dir { childs } => Some(childs),
            ContentEntryKind::File => None,
        }
    }
}

impl Serialize for ContentEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let childs = self.childs();
        let mut map = serializer.serialize_map(Some(if childs.is_some() { 4 } else { 3 }))?;
        map.serialize_entry("kind", if childs.is_some() { "dir" } else { "file" })?;
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("path", &self.path)?;
        if let Some(childs) = childs {
            map.serialize_entry("content", childs)?;
        }
        map.end()
    }
}

const FILE_KEYS: &[&str] = &["kind", "name", "path"];
const DIR_KEYS: &[&str] = &["kind", "name", "path", "content"];

/// Position of an entry in the content tree: indexes from the root down.
pub(crate) type Named = Vec<usize>;

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
    InvalidYaml(String),
    WrongType { key: &'static str },
    UnknownValue { key: &'static str, value: String },
    EmptyPath { key: &'static str },
    ContentWrongType { expected: &'static str },
    ContentMissingKey { key: &'static str },
    ContentKeyWrongType { key: &'static str },
    ContentUnexpectedKind { value: String },
    ContentUnexpectedKey { key: String },
    ContentEmptyPath,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct RawProjectFile {
    document: RawDocumentOptions,
    build: RawBuild,
    content: Option<yaml_serde::Value>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct RawDocumentOptions {
    style: Option<yaml_serde::Value>,
    out: Option<yaml_serde::Value>,
    single: Option<yaml_serde::Value>,
    no_js: Option<yaml_serde::Value>,
    theme: Option<yaml_serde::Value>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct RawBuild {
    input: Option<yaml_serde::Value>,
    output: Option<yaml_serde::Value>,
}

#[derive(Serialize)]
struct OutProjectFile<'a> {
    document: OutDocumentOptions,
    build: OutBuild,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    content: &'a Vec<ContentEntry>,
}

#[derive(Serialize)]
struct OutDocumentOptions {
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

fn is_inside(child: &str, dir: &str) -> bool {
    let dir = dir.trim_end_matches('/');
    child.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'))
}

impl ProjectFile {
    pub(crate) fn save(&self, root: &Path) -> io::Result<()> {
        fs::write(root.join(PROJECT_FILE), self.to_yaml(root))
    }

    pub(crate) fn to_yaml(&self, root: &Path) -> String {
        let raw = OutProjectFile {
            document: OutDocumentOptions {
                style: self.config.style.out_name(),
                out: self.config.out.out_name(),
                single: self.config.single,
                no_js: self.config.no_js,
                theme: self.config.theme.out_name(),
            },
            build: OutBuild {
                input: relative_to(&self.input, root),
                output: relative_to(&self.output, root),
            },
            content: &self.content,
        };
        yaml_serde::to_string(&raw).expect("ProjectFile must be serializable to yaml")
    }

    pub(crate) fn load(root: &Path) -> io::Result<(ProjectFile, Vec<ConfigWarning>)> {
        let (mut project, warnings) = Self::parse(&fs::read_to_string(root.join(PROJECT_FILE))?);
        project.input = root.join(&project.input);
        project.output = root.join(&project.output);
        Ok((project, warnings))
    }

    pub(crate) fn parse(src: &str) -> (ProjectFile, Vec<ConfigWarning>) {
        let mut warnings = Vec::new();
        let raw: RawProjectFile = match yaml_serde::from_str(src) {
            Ok(raw) => raw,
            Err(e) => {
                warnings.push(ConfigWarning::InvalidYaml(e.to_string()));
                return (ProjectFile::default(), warnings);
            }
        };
        let mut out = ProjectFile::default();
        if let Some(style) = get_enum_param(raw.document.style, "document.style", &mut warnings) {
            out.config.style = style;
        }
        if let Some(doc_out) = get_enum_param(raw.document.out, "document.out", &mut warnings) {
            out.config.out = doc_out;
        }
        if let Some(single) = bool_param(raw.document.single, "document.single", &mut warnings) {
            out.config.single = single;
        }
        if let Some(no_js) = bool_param(raw.document.no_js, "document.no_js", &mut warnings) {
            out.config.no_js = no_js;
        }
        if let Some(theme) = get_enum_param(raw.document.theme, "document.theme", &mut warnings) {
            out.config.theme = theme;
        }
        if let Some(input) = path_param(raw.build.input, "build.input", &mut warnings) {
            out.input = input;
        }
        if let Some(output) = path_param(raw.build.output, "build.output", &mut warnings) {
            out.output = output;
        }
        if let Some(content) = raw.content {
            out.content = parse_content(&content, &mut warnings);
        }
        (out, warnings)
    }

    pub(crate) fn find_name(&self, name: &str) -> Vec<Named> {
        fn walk(entries: &[ContentEntry], name: &str, at: &mut Named, out: &mut Vec<Named>) {
            for (i, entry) in entries.iter().enumerate() {
                at.push(i);
                if entry.name == name {
                    out.push(at.clone());
                }
                if let Some(childs) = entry.childs() {
                    walk(childs, name, at, out);
                }
                at.pop();
            }
        }
        let mut out = Vec::new();
        walk(&self.content, name, &mut Vec::new(), &mut out);
        out
    }

    pub(crate) fn entry(&self, at: &[usize]) -> &ContentEntry {
        let (last, parent) = at.split_last().expect("entry position must not be empty");
        &self.list_at(parent)[*last]
    }

    pub(crate) fn entry_mut(&mut self, at: &[usize]) -> &mut ContentEntry {
        let (last, parent) = at.split_last().expect("entry position must not be empty");
        &mut self.list_at_mut(parent)[*last]
    }


    fn list_at(&self, at: &[usize]) -> &Vec<ContentEntry> {
        at.iter().fold(&self.content, |list, &i| list[i].childs().expect("position must lead through folders"))
    }

    fn list_at_mut(&mut self, at: &[usize]) -> &mut Vec<ContentEntry> {
        at.iter().fold(&mut self.content, |list, &i| list[i].childs_mut().expect("position must lead through folders"))
    }


    pub(crate) fn parent_dir_of(&self, path: &str) -> Named {
        let mut at = Vec::new();
        let mut list = &self.content;
        while let Some(i) = list.iter().position(|e| e.is_dir() && is_inside(path, &e.path)) {
            at.push(i);
            list = list[i].childs().unwrap();
        }
        at
    }

    pub(crate) fn insert_entry(&mut self, entry: ContentEntry) -> Named {
        let mut at = self.parent_dir_of(&entry.path);
        let list = self.list_at_mut(&at);
        list.push(entry);
        at.push(list.len() - 1);
        at
    }

    pub(crate) fn find_dir_path(&self, path: &str) -> Option<Named> {
        let mut at = self.parent_dir_of(path);
        let i = self.list_at(&at).iter().position(|e| e.is_dir() && e.path.trim_end_matches('/') == path)?;
        at.push(i);
        Some(at)
    }

    pub(crate) fn remove_at(&mut self, at: &[usize]) -> ContentEntry {
        let (last, parent) = at.split_last().expect("entry position must not be empty");
        self.list_at_mut(parent).remove(*last)
    }

    pub(crate) fn insert_at(&mut self, at: &[usize], entry: ContentEntry) {
        let (last, parent) = at.split_last().expect("entry position must not be empty");
        self.list_at_mut(parent).insert(*last, entry);
    }
}

fn path_param(value: Option<yaml_serde::Value>, key: &'static str, warnings: &mut Vec<ConfigWarning>) -> Option<PathBuf> {
    match value? {
        yaml_serde::Value::String(s) if s.is_empty() => {
            warnings.push(ConfigWarning::EmptyPath { key });
            None
        }
        yaml_serde::Value::String(s) => Some(PathBuf::from(s)),
        _ => {
            warnings.push(ConfigWarning::WrongType { key });
            None
        }
    }
}

fn bool_param(value: Option<yaml_serde::Value>, key: &'static str, warnings: &mut Vec<ConfigWarning>) -> Option<bool> {
    match value? {
        yaml_serde::Value::Bool(b) => Some(b),
        _ => {
            warnings.push(ConfigWarning::WrongType { key });
            None
        }
    }
}

fn parse_content(value: &yaml_serde::Value, warnings: &mut Vec<ConfigWarning>) -> Vec<ContentEntry> {
    let yaml_serde::Value::Sequence(items) = value else {
        warnings.push(ConfigWarning::ContentWrongType { expected: "sequence" });
        return Vec::new();
    };
    items.iter().filter_map(|item| parse_content_entry(item, warnings)).collect()
}

fn parse_content_entry(item: &yaml_serde::Value, warnings: &mut Vec<ConfigWarning>) -> Option<ContentEntry> {
    let yaml_serde::Value::Mapping(map) = item else {
        warnings.push(ConfigWarning::ContentWrongType { expected: "mapping" });
        return None;
    };

    let kind_key = req_str(map, "kind", warnings);
    let name = req_str(map, "name", warnings);
    let path = req_str(map, "path", warnings).filter(|p| {
        if p.is_empty() {
            warnings.push(ConfigWarning::ContentEmptyPath);
        }
        !p.is_empty()
    });
    let (allowed, kind): (&[&str], Option<ContentEntryKind>) = match kind_key.as_deref() {
        Some("file") => (FILE_KEYS, Some(ContentEntryKind::File)),
        Some("dir") => {
            let childs = match map.get("content") {
                Some(value) => parse_content(value, warnings),
                None => Vec::new(),
            };
            (DIR_KEYS, Some(ContentEntryKind::Dir { childs }))
        }
        Some(other) => {
            warnings.push(ConfigWarning::ContentUnexpectedKind { value: other.to_owned() });
            (DIR_KEYS, None)
        }
        None => (DIR_KEYS, None),
    };

    for k in map.keys() {
        if !k.as_str().is_some_and(|s| allowed.contains(&s)) {
            let key = k.as_str().map_or_else(|| format!("{k:?}"), str::to_owned);
            warnings.push(ConfigWarning::ContentUnexpectedKey { key });
        }
    }

    Some(ContentEntry { name: name?, path: path?, kind: kind? })
}

fn req_str(map: &yaml_serde::Mapping, key: &'static str, warnings: &mut Vec<ConfigWarning>) -> Option<String> {
    match map.get(key) {
        Some(yaml_serde::Value::String(s)) => Some(s.clone()),
        Some(_) => {
            warnings.push(ConfigWarning::ContentKeyWrongType { key });
            None
        }
        None => {
            warnings.push(ConfigWarning::ContentMissingKey { key });
            None
        }
    }
}

fn get_enum_param<T: ConfigConst>(value: Option<yaml_serde::Value>, key: &'static str, warnings: &mut Vec<ConfigWarning>) -> Option<T> {
    match value? {
        yaml_serde::Value::String(s) => {
            let parsed = T::from_name(&s);
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
    use rush_lib::{DocOut, DocStyle, Theme};

    fn check(src: &str, style: DocStyle, warnings: Vec<ConfigWarning>) {
        let (out, w) = ProjectFile::parse(src);
        assert_eq!(out.config.style, style, "вход: {src:?}");
        assert_eq!(w, warnings, "вход: {src:?}");
    }

    #[test]
    fn valid_values() {
        check("document:\n  style: gost7-32", DocStyle::Gost7_32, vec![]);
        check("document:\n  style: modern", DocStyle::Modern, vec![]);
        check("document:\n  style: \"blog\"", DocStyle::Blog, vec![]);
    }

    #[test]
    fn missing_is_default_without_warnings() {
        check("", DocStyle::Blog, vec![]);
        check("document: {}", DocStyle::Blog, vec![]);
        check("other:\n  x: 1", DocStyle::Blog, vec![]);
    }

    #[test]
    fn unknown_value_falls_back() {
        check(
            "document:\n  style: Gost",
            DocStyle::Blog,
            vec![ConfigWarning::UnknownValue { key: "document.style", value: "Gost".into() }],
        );
    }

    #[test]
    fn wrong_type_falls_back() {
        check("document:\n  style: 5", DocStyle::Blog, vec![ConfigWarning::WrongType { key: "document.style" }]);
        check("document:\n  style: light", DocStyle::Blog, vec![ConfigWarning::UnknownValue { key: "document.style", value: "light".into() }]);
    }

    #[test]
    fn broken_yaml_falls_back() {
        let (out, w) = ProjectFile::parse("document:\n  style: [blog\n");
        assert_eq!(out, ProjectFile::default());
        assert!(matches!(w.as_slice(), [ConfigWarning::InvalidYaml(_)]), "{w:?}");
    }

    fn check_out(src: &str, doc_out: DocOut, warnings: Vec<ConfigWarning>) {
        let (out, w) = ProjectFile::parse(src);
        assert_eq!(out.config.out, doc_out, "вход: {src:?}");
        assert_eq!(w, warnings, "вход: {src:?}");
    }

    #[test]
    fn out_valid_values() {
        check_out("document:\n  out: html", DocOut::Html, vec![]);
        check_out("document:\n  out: markdown", DocOut::Markdown, vec![]);
        check_out("document:\n  out: pdf", DocOut::Pdf, vec![]);
        check_out("document:\n  out: odt", DocOut::Odt, vec![]);
        check_out("document:\n  out: docx", DocOut::Docx, vec![]);
    }

    #[test]
    fn out_missing_is_html() {
        check_out("", DocOut::Html, vec![]);
        check_out("document:\n  style: gost7-32", DocOut::Html, vec![]);
    }

    #[test]
    fn out_bad_falls_back_to_html() {
        check_out(
            "document:\n  out: exe",
            DocOut::Html,
            vec![ConfigWarning::UnknownValue { key: "document.out", value: "exe".into() }],
        );
        check_out("document:\n  out: true", DocOut::Html, vec![ConfigWarning::WrongType { key: "document.out" }]);
    }

    #[test]
    fn style_and_out_together() {
        let (out, w) = ProjectFile::parse("document:\n  style: modern\n  out: pdf");
        assert_eq!(out, ProjectFile { config: Config { style: DocStyle::Modern, out: DocOut::Pdf, ..Config::default() }, ..ProjectFile::default() });
        assert!(w.is_empty());
    }

    #[test]
    fn single_value() {
        let (out, w) = ProjectFile::parse("document:\n  single: true");
        assert!(out.config.single);
        assert!(w.is_empty());
        assert!(!ProjectFile::parse("").0.config.single);
    }

    #[test]
    fn single_wrong_type_falls_back() {
        let (out, w) = ProjectFile::parse("document:\n  single: \"да\"");
        assert!(!out.config.single);
        assert_eq!(w, vec![ConfigWarning::WrongType { key: "document.single" }]);
    }

    #[test]
    fn html_padges_is_unknown() {
        check_out(
            "document:\n  out: html-padges",
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
        check_paths("build:\n  input: docs\n  output: dist/html", "docs", "dist/html", vec![]);
        check_paths("build:\n  input: 1\n  output: \"\"", "src", "out", vec![
            ConfigWarning::WrongType { key: "build.input" },
            ConfigWarning::EmptyPath { key: "build.output" },
        ]);
    }

    use ContentEntry as E;

    #[test]
    fn content_valid() {
        let (out, w) = ProjectFile::parse("
content:
  - kind: file
    name: Введение
    path: intro.rush
  - kind: dir
    name: Руководство
    path: guide
    content:
      - kind: file
        name: Установка
        path: guide/install.rush
      - kind: dir
        name: Пусто
        path: guide/empty
");
        assert_eq!(out.content, vec![
            E::file("Введение", "intro.rush"),
            E::dir("Руководство", "guide", vec![
                E::file("Установка", "guide/install.rush"),
                E::dir("Пусто", "guide/empty", vec![]),
            ]),
        ]);
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn content_bad_entries_are_skipped() {
        let (out, w) = ProjectFile::parse("
content:
  - kind: file
    name: A
  - kind: folder
    name: B
    path: b
  - kind: file
    name: C
    path: c.rush
    content: []
  - 5
  - kind: file
    name: D
    path: \"\"
");
        assert_eq!(out.content, vec![E::file("C", "c.rush")]);
        assert_eq!(w, vec![
            ConfigWarning::ContentMissingKey { key: "path" },
            ConfigWarning::ContentUnexpectedKind { value: "folder".into() },
            ConfigWarning::ContentUnexpectedKey { key: "content".into() },
            ConfigWarning::ContentWrongType { expected: "mapping" },
            ConfigWarning::ContentEmptyPath,
        ]);
    }

    #[test]
    fn content_missing_is_empty() {
        let (out, w) = ProjectFile::parse("document:\n  style: gost7-32");
        assert!(out.content.is_empty());
        assert!(w.is_empty());
    }

    #[test]
    fn content_wrong_type() {
        let (out, w) = ProjectFile::parse("content: a");
        assert!(out.content.is_empty());
        assert_eq!(w, vec![ConfigWarning::ContentWrongType { expected: "sequence" }]);
    }

    #[test]
    fn build_paths_resolve_from_root() {
        let root = std::env::temp_dir().join("rush_cli_build_paths");
        fs::create_dir_all(&root).unwrap();
        let abs = std::env::temp_dir().join("abs_out");
        let src = format!("build:\n  input: text\n  output: '{}'", abs.display());
        fs::write(root.join(PROJECT_FILE), src).unwrap();

        let (out, w) = ProjectFile::load(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(out.input, root.join("text"));
        assert_eq!(out.output, abs);
        assert!(w.is_empty());
    }

    #[test]
    fn empty_content_not_serialized() {
        let src = ProjectFile::default().to_yaml(Path::new("root"));
        assert!(!src.contains("content"), "{src}");
        let (out, w) = ProjectFile::parse(&src);
        assert_eq!(out, ProjectFile::default());
        assert!(w.is_empty(), "{w:?}");
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
            content: vec![
                E::file("Введение", "intro"),
                E::dir("Руководство", "guide", vec![E::file("Установка", "guide/install.rush"), E::dir("Пусто", "guide/empty", vec![])]),
            ],
        };

        project.save(&root).unwrap();
        let saved = fs::read_to_string(root.join(PROJECT_FILE)).unwrap();
        let (loaded, w) = ProjectFile::load(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert!(saved.contains("input: text"), "{saved}");
        assert!(saved.contains("kind: dir"), "{saved}");
        assert_eq!(loaded, project);
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn no_js_and_theme() {
        let (out, w) = ProjectFile::parse("document:\n  no_js: true\n  theme: dark");
        assert!(out.config.no_js);
        assert_eq!(out.config.theme, Theme::Dark);
        assert!(w.is_empty());

        let (out, _) = ProjectFile::parse("");
        assert!(!out.config.no_js);
        assert_eq!(out.config.theme, Theme::Auto);

        let (out, w) = ProjectFile::parse("document:\n  no_js: \"да\"\n  theme: night");
        assert!(!out.config.no_js);
        assert_eq!(out.config.theme, Theme::Auto);
        assert_eq!(w, vec![
            ConfigWarning::WrongType { key: "document.no_js" },
            ConfigWarning::UnknownValue { key: "document.theme", value: "night".into() },
        ]);
    }

    #[test]
    fn project_root_saved_as_dot() {
        let root = PathBuf::from("root");
        let project = ProjectFile { input: root.clone(), output: root.join("out"), ..ProjectFile::default() };
        let src = project.to_yaml(&root);
        let (out, w) = ProjectFile::parse(&src);
        assert_eq!(out.input, PathBuf::from("."), "{src}");
        assert!(w.is_empty(), "{w:?}");
    }

    fn tree() -> ProjectFile {
        ProjectFile {
            content: vec![
                E::file("A", "a.rush"),
                E::dir("Guide", "guide", vec![E::dir("Deep", "guide/deep", vec![])]),
            ],
            ..ProjectFile::default()
        }
    }

    #[test]
    fn insert_goes_into_existing_dir_or_root() {
        let mut p = tree();
        assert_eq!(p.insert_entry(E::file("In guide", "guide/x.rush")), vec![1, 1]);
        assert_eq!(p.insert_entry(E::file("In deep", "guide/deep/y.rush")), vec![1, 0, 0]);
        assert_eq!(p.insert_entry(E::file("Unknown dir", "other/z.rush")), vec![2]);
        assert_eq!(p.insert_entry(E::file("Prefix only", "guidebook/z.rush")), vec![3]);
        assert_eq!(p.insert_entry(E::dir("New", "guide/new", vec![])), vec![1, 2]);
        assert_eq!(p.entry(&[1, 0, 0]).name, "In deep");
        assert_eq!(p.find_dir_path("guide/deep"), Some(vec![1, 0]));
        assert_eq!(p.find_dir_path("other"), None);
    }

    #[test]
    fn find_and_remove_in_tree() {
        let mut p = tree();
        p.insert_entry(E::file("A", "guide/deep/a.rush"));
        assert_eq!(p.find_name("A"), vec![vec![0], vec![1, 0, 0]]);
        let removed = p.remove_at(&[1, 0, 0]);
        assert_eq!(removed, E::file("A", "guide/deep/a.rush"));
        p.insert_at(&[1, 0, 0], removed);
        assert_eq!(p.find_name("A").len(), 2);
    }
}