use core::fmt;
use std::{error::Error, env, fs::{self, OpenOptions}, io, path::{Path, PathBuf}, process::ExitCode};

use rush_lib::{Config, ConfigConst, DocOut, DocStyle, Project, Theme};

use crate::{AddError::UnsupportedExtension, clean::Dirs};
use crate::config::{ConfigWarning, ContentEntry, ContentEntryKind, ProjectFile, PROJECT_FILE};
use clap::{Parser, ValueEnum, Subcommand};

mod clean;
mod config;

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Style {
    /// Pure GOST 7.32: the external standard for academic and scientific papers printed on A4.
    Gost7_32,
    /// Readable academic paper: prints on A4 just like GOST 7.32,
    /// but with sans-serif text and warm markers that the eye catches easily.
    Modern,
    /// The main modern web format: documentation, technical books and articles.
    /// Supports light and dark themes.
    Blog,
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gost7_32 => write!(f, "gost7-32"),
            Self::Modern => write!(f, "modern"),
            Self::Blog => write!(f, "blog"),
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum ThemeArg {
    /// System theme where the output supports it, light otherwise
    Auto,
    Light,
    /// Only the blog style has a dark theme
    Dark,
}

impl From<ThemeArg> for Theme {
    fn from(t: ThemeArg) -> Self {
        match t {
            ThemeArg::Auto => Theme::Auto,
            ThemeArg::Light => Theme::Light,
            ThemeArg::Dark => Theme::Dark,
        }
    }
}



#[derive(ValueEnum, Clone, Copy, Debug)]
enum OutType {
    /// Create an HTML tree in the output with a single CSS file,
    /// or one multipage HTML file if the project is single.
    /// With this output type, all images are copied to the output directory.
    Html,
    /// Create a Markdown MDX tree in the output,
    /// or one Markdown file if the project is single.
    /// With this output type, all images are copied to the output directory.
    Markdown
}

impl fmt::Display for OutType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Html => write!(f, "html"),
            Self::Markdown => write!(f, "markdown")
        }
    }
}


#[derive(Subcommand, Debug)]
enum Command {
    /// Create a new project
    New {
        /// Name of the project
        name: String,
        #[arg(short, long, value_enum, default_value_t = Style::Blog)]
        /// Determines the style of the render.
        style: Style,
        #[arg(short, long, value_enum, default_value_t = OutType::Html)]
        /// Determines the render filetype.
        out_type: OutType,
        #[arg(long)]
        /// Render the whole project to one file instead of one file per .rush file.
        single: bool,
        #[arg(long)]
        /// Static output without scripts: no menu button, theme is fixed.
        no_js: bool,
        #[arg(long, value_enum, default_value_t = ThemeArg::Auto)]
        /// Starting theme; a fixed one without scripts.
        theme: ThemeArg,
        #[arg(long, default_value = "src")]
        /// How to name the src folder
        src_name: String,
        #[arg(long, default_value = "out")]
        /// How to name the out folder
        out_name: String
    },
    /// Build the current project
    Build {
        /// Target path. If not set, the current directory is used.
        path: Option<PathBuf>,

        #[arg(value_enum, long)]
        /// Render to a different output type without changing the type in 'project.yaml'
        out: Option<OutType>,

        #[arg(long)]
        /// Render to one file or to one file per .rush file without changing 'single' in 'project.yaml'
        single: Option<bool>,

        #[arg(long)]
        /// Render with or without scripts without changing 'no_js' in 'project.yaml'
        no_js: Option<bool>,

        #[arg(value_enum, long)]
        /// Render with another theme without changing 'theme' in 'project.yaml'
        theme: Option<ThemeArg>,

    },
    /// Add a new .rush file, or a folder with --dir
    Add {
        /// Path to the new file, relative to src. If it points into a dir that exists in project.yaml, the file is added to that dir,
        /// otherwise it is added to the project root, regardless of the file path.
        path: PathBuf,
        /// Name of the new file or folder. If it is not specified, the object is created with a name that matches the original file name.
        name: Option<String>,
        #[arg(long)]
        /// Create a folder and give it the name in the menu
        dir: bool,
    },
    /// Rename a file or a folder in the menu; the files on disk stay as they are
    Rename {
        /// Current name in the content or folders, or a folder name on disk
        old_name: String,
        new_name: String,
    },
    /// Remove a .rush file from the project
    Remove {
        /// Name of the file in the project
        name: String
    },
    /// Show all .rush files in the project
    List
}

#[derive(Parser, Debug)]
#[command(name="rush", version, about)]
struct Args {
    #[command(subcommand)]
    command: Command,
}



#[derive(Debug, Clone, PartialEq)]
enum Warning {
    Config(ConfigWarning),
    Lib(rush_lib::Warning),
}

impl From<ConfigWarning> for Warning {
    fn from(w: ConfigWarning) -> Self {
        Self::Config(w)
    }
}

impl From<rush_lib::Warning> for Warning {
    fn from(w: rush_lib::Warning) -> Self {
        Self::Lib(w)
    }
}

impl From<rush_lib::ProjectWarning> for Warning {
    fn from(w: rush_lib::ProjectWarning) -> Self {
        Self::Lib(w.into())
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    match args.command {
        Command::New { name, style, out_type, single, no_js, theme, src_name, out_name } => {
            if let Err(e) = new(&name, &src_name, &out_name, style, out_type, single, no_js, theme) {
                eprintln!("Failed to create new project: {e}");
                return  ExitCode::FAILURE;
            }
            println!("Successfully created new project '{name}'");
            return ExitCode::SUCCESS;
        },
        Command::Add { name, path, dir } => {
            match add(name.as_deref(), path, dir) {
                Ok(path) => {
                    println!("Successfully added new {}: '{path}'", if dir { "folder" } else { "file" });
                    return ExitCode::SUCCESS;
                },
                Err(e) => {
                    eprintln!("Failed to add new file or dir: {e}");
                    return  ExitCode::FAILURE;
                }
            }
        }
        Command::Rename { old_name, new_name } => {
            match rename(&old_name, &new_name) {
                Ok(path) => {
                    println!("Successfully renamed '{old_name}' to '{new_name}': ./{path}");
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("Failed to rename: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Command::Build { path, out, single, no_js, theme } => {
            match build(path, out, single, no_js, theme.map(Theme::from)) {
                Ok(warnings) => {
                    for w in &warnings {
                        println!("{w:?}");
                    }
                    println!("Project has been built! {} warnings", warnings.len());
                    return ExitCode::SUCCESS;
                },
                Err(e) => {
                    eprintln!("Failed to build project: {e}");
                    return  ExitCode::FAILURE;
                }
            }
        }
        Command::Remove { name } => {
            match remove(&name) {
                Ok(Removed::Deleted) => {
                    println!("Successfully removed file '{name}'");
                    return ExitCode::SUCCESS;
                },
                Ok(Removed::MissingOnDisk) => {
                    println!("Removed '{name}' from project, file was already missing on disk");
                    return ExitCode::SUCCESS;
                },
                Ok(Removed::NotInProject) => {
                    eprintln!("File '{name}' not found in project");
                    return  ExitCode::FAILURE;
                },
                Err(e) => {
                    eprintln!("Failed to remove file: {e}");
                    return  ExitCode::FAILURE;
                }
            }
        }
        Command::List => {
            if let Err(e) = list() {
                eprintln!("Failed to list the project: {e}");
                return ExitCode::FAILURE;
            }
            return ExitCode::SUCCESS;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn new(name: &str, src_name: &str, out_name: &str, style: Style, out_type: OutType, single: bool, no_js: bool, theme: ThemeArg) -> io::Result<()> {
    let mut root = env::current_dir()?;
    root.push(file_name(name)?);
    let src_dir = root.join(src_name);
    let out_dir = root.join(out_name);
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&out_dir)?;

    let config = Config {
        style: DocStyle::from_name(&style.to_string()).expect("every Style has a DocStyle"),
        out: DocOut::from_name(&out_type.to_string()).expect("every OutType has a DocOut"),
        single,
        no_js,
        theme: theme.into(),
    };
    ProjectFile { config, input: src_dir, output: out_dir, content: Vec::new() }.save(&root)
}

fn load_project(root: &Path) -> io::Result<ProjectFile> {
    ProjectFile::load(root).map(|(project, _)| project).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found; the command must be called from the project directory. Create a project first with 'rush new <NAME>'")),
        _ => e,
    })
}

fn file_name(name: &str) -> io::Result<String> {
    let clean: String = name.chars().filter(|c| !matches!(c, '"' | '/' | '\\')).collect();
    let clean = clean.trim();
    if clean.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("name {name:?} is empty after removing quotes and slashes")));
    }
    Ok(clean.to_owned())
}

fn slashed(path: &Path) -> String {
    path.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

#[derive(Debug)]
enum AddError {
    Io(io::Error),
    UnsupportedExtension(String)
}
impl From<io::Error> for AddError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl fmt::Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "input-output error: {err}"),
            Self::UnsupportedExtension(ext) => write!(f, "unsupported extension error, got: {ext}")
        }
    }
}

impl Error for AddError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::UnsupportedExtension(_) => None
        }
    }
}


/// Creates the file or folder on disk and puts its entry into the content of the folder entry
/// that holds it, or into the root content when no folder entry holds it.
fn add(name: Option<&str>, rel_path: PathBuf, dir: bool) -> Result<String, AddError> {
    let root = env::current_dir()?;
    let mut project = load_project(&root)?;
    let path = add_in(&mut project, name, rel_path, dir)?;
    project.save(&root)?;
    Ok(path)
}


fn add_in(project: &mut ProjectFile, name: Option<&str>, rel: PathBuf, dir: bool) -> Result<String, AddError> {
    let entry = if dir {
        let path = slashed(&rel);
        if project.find_dir_path(&path).is_some() {
            return  Err(AddError::Io(io::Error::new(io::ErrorKind::AlreadyExists, format!("folder ./{path} is already in the content"))));
        }
        fs::create_dir_all(project.input.join(&rel))?;
        let name = match name {
            Some(name) => name,
            None => rel.file_stem().and_then(|s| s.to_str()).unwrap()
        };
        ContentEntry::dir(name, path, Vec::new())
    } else {
        let mut rel = rel;
        match rel.extension() {
            None => {rel.set_extension("rush");},
            Some(ext) if ext == "rush" => {},
            Some(ext) => return Err(UnsupportedExtension(ext.to_string_lossy().to_string()))
        }
        let full = project.input.join(&rel);
        create_parent(&full)?;
        OpenOptions::new().write(true).create_new(true).open(&full)?;
        let name = match name {
            Some(name) => name,
            None => rel.file_stem().and_then(|s| s.to_str()).unwrap()
        };
        ContentEntry::file(name, slashed(&rel))
    };
    let path = entry.path.clone();
    project.insert_entry(entry);
    Ok(path)
}


fn handle_content(data: Vec<ContentEntry>, project: &mut Project) {
    for entry in data {
        match entry.kind {
            ContentEntryKind::Dir { childs } => {
                project.add_dir(entry.name, entry.path);
                handle_content(childs, project);
            }
            ContentEntryKind::File => {
                project.add_content(entry.name, entry.path);   
            }
        }
    }
}


fn build(root: Option<PathBuf>, redef_out_type: Option<OutType>, redef_single: Option<bool>, redef_no_js: Option<bool>, redef_theme: Option<Theme>) -> io::Result<Vec<Warning>> {
    let root = root.map_or(env::current_dir()?, |root| root);
    let (mut file, config_warns) = ProjectFile::load(&root).map_err(|e|
        match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found; the command must be called from the project directory. Create a project first with 'rush new <NAME>'")),
            _ => e,
        }
    )?;
    if let Some(redef_out_type ) = redef_out_type {
        match redef_out_type {
            OutType::Html => file.config.out = rush_lib::DocOut::Html,
            OutType::Markdown => file.config.out = rush_lib::DocOut::Markdown
        }
    }
    if let Some(single) = redef_single {
        file.config.single = single;
    }
    if let Some(no_js) = redef_no_js {
        file.config.no_js = no_js;
    }
    if let Some(theme) = redef_theme {
        file.config.theme = theme;
    }

    let dirs = Dirs::resolve(&root, &file.input, &file.output)?;
    let mut warns: Vec<Warning> = config_warns.into_iter().map(Warning::from).collect();
    let (mut project, project_warns) = Project::index_dir_common(&dirs.input, dirs.nested_output())?;
    warns.extend(project_warns.into_iter().map(Warning::from));
    handle_content(file.content, &mut project);
    let (out_files, build_warns) = rush_lib::build(&file.config, &project).map_err(io::Error::other)?;
    warns.extend(build_warns.into_iter().map(Warning::from));
    

    dirs.clean()?;
    let mut written = Vec::with_capacity(out_files.len());
    for out in out_files {
        let dst = file.output.join(&out.path);
        create_parent(&dst)?;
        fs::write(dst, out.data)?;
        written.push(out.path);
    }
    if !dirs.same() {
        for image in project.images() {
            let dst = file.output.join(image);
            create_parent(&dst)?;
            fs::copy(file.input.join(image), dst)?;
            written.push(image.to_owned());
        }
    }
    if dirs.overlap() {
        dirs.save_manifest(&written)?;
    }
    Ok(warns)
}

fn create_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(dir) => fs::create_dir_all(dir),
        None => Ok(()),
    }
}

enum Removed {
    Deleted,
    MissingOnDisk,
    NotInProject,
}

fn remove(name: &str) -> io::Result<Removed> {
    let root = env::current_dir()?;
    let mut project = load_project(&root)?;
    let files: Vec<_> = project.find_name(name).into_iter().filter(|at| !project.entry(at).is_dir()).collect();
    let pos = match files.as_slice() {
        [] => return Ok(Removed::NotInProject),
        [at] => at.clone(),
        many => {
            let places: Vec<_> = many.iter().map(|at| format!("file ./{}", project.entry(at).path)).collect();
            return Err(ambiguous(name, &places));
        }
    };
    let entry = project.remove_at(&pos);
    project.save(&root)?;

    match fs::remove_file(project.input.join(&entry.path)) {
        Ok(()) => Ok(Removed::Deleted),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Removed::MissingOnDisk),
        Err(e) => {
            project.insert_at(&pos, entry);
            match project.save(&root) {
                Ok(()) => Err(e),
                Err(save_err) => Err(io::Error::new(
                    e.kind(),
                    format!("{e}; failed to restore entry in {PROJECT_FILE}: {save_err}"),
                )),
            }
        }
    }
}

fn dirs_named(input: &Path, skip: &Path, name: &str) -> io::Result<Vec<String>> {
    fn walk(dir: &Path, rel: &Path, skip: &Path, name: &str, out: &mut Vec<String>) -> io::Result<()> {
        let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<io::Result<_>>()?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if !entry.file_type()?.is_dir() || path == skip {
                continue;
            }
            let rel = rel.join(entry.file_name());
            if entry.file_name() == name {
                out.push(slashed(&rel));
            }
            walk(&path, &rel, skip, name, out)?;
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(input, Path::new(""), skip, name, &mut out)?;
    Ok(out)
}

fn ambiguous(name: &str, places: &[String]) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, format!("name '{name}' is ambiguous, found in: {}", places.join(", ")))
}

fn rename_in(project: &mut ProjectFile, old_name: &str, new_name: &str) -> io::Result<String> {
    let found = project.find_name(old_name);
    match found.as_slice() {
        [at] => {
            let entry = project.entry_mut(at);
            entry.name = new_name.to_owned();
            Ok(entry.path.clone())
        }
        [] => {
            // A folder on disk that is not in content yet: add it to content with the new name
            let on_disk = dirs_named(&project.input, &project.output, old_name)?;
            let on_disk: Vec<_> = on_disk.into_iter().filter(|p| project.find_dir_path(p).is_none()).collect();
            match on_disk.as_slice() {
                [path] => {
                    project.insert_entry(ContentEntry::dir(new_name, path.clone(), Vec::new()));
                    Ok(path.clone())
                }
                [] => Err(io::Error::new(io::ErrorKind::NotFound, format!("'{old_name}' not found in the content or source folders"))),
                many => Err(ambiguous(old_name, &many.iter().map(|p| format!("folder ./{p}")).collect::<Vec<_>>())),
            }
        }
        many => {
            let places: Vec<String> = many.iter().map(|at| {
                let entry = project.entry(at);
                format!("{} ./{}", if entry.is_dir() { "folder" } else { "file" }, entry.path)
            }).collect();
            Err(ambiguous(old_name, &places))
        }
    }
}

fn rename(old_name: &str, new_name: &str) -> io::Result<String> {
    let root = env::current_dir()?;
    let mut project = load_project(&root)?;
    let path = rename_in(&mut project, old_name, new_name)?;
    project.save(&root)?;
    Ok(path)
}

fn list() -> io::Result<()> {
    fn print(entries: &[ContentEntry], depth: usize) {
        for entry in entries {
            let indent = "  ".repeat(depth);
            match &entry.kind {
                ContentEntryKind::File => println!("{indent}\"{}\": ./{}", entry.name, entry.path),
                ContentEntryKind::Dir { childs } => {
                    println!("{indent}\"{}\": ./{}/", entry.name, entry.path);
                    print(childs, depth + 1);
                }
            }
        }
    }
    let root = env::current_dir()?;
    print(&load_project(&root)?.content, 0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ContentEntry as E;

    fn project_in(root: &Path) -> ProjectFile {
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(root.join("src/guide/deep")).unwrap();
        fs::create_dir_all(root.join("src/api/deep")).unwrap();
        fs::create_dir_all(root.join("src/out/guide")).unwrap();
        ProjectFile {
            input: root.join("src"),
            output: root.join("src/out"),
            content: vec![
                E::file("Введение", "intro.rush"),
                E::file("Общее", "a.rush"),
                E::dir("API", "api", vec![E::file("Общее", "api/b.rush")]),
            ],
            ..ProjectFile::default()
        }
    }

    #[test]
    fn rename_finds_content_dirs_and_disk() {
        let root = env::temp_dir().join("rush_cli_rename");
        let mut p = project_in(&root);

        assert_eq!(rename_in(&mut p, "Введение", "Начало").unwrap(), "intro.rush");
        assert_eq!(p.content[0].name, "Начало");
        assert_eq!(rename_in(&mut p, "API", "Справочник").unwrap(), "api");
        assert_eq!(p.content[2].name, "Справочник");
        assert_eq!(rename_in(&mut p, "guide", "Руководство").unwrap(), "guide");
        assert_eq!(p.content[3], E::dir("Руководство", "guide", vec![]));

        let ambiguous = rename_in(&mut p, "Общее", "X").unwrap_err();
        assert!(ambiguous.to_string().contains("file ./a.rush, file ./api/b.rush"), "{ambiguous}");
        let on_disk = rename_in(&mut p, "deep", "X").unwrap_err();
        assert!(on_disk.to_string().contains("folder ./api/deep, folder ./guide/deep"), "{on_disk}");
        assert_eq!(rename_in(&mut p, "Нет", "X").unwrap_err().kind(), io::ErrorKind::NotFound);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn add_puts_entry_into_existing_dir_or_root() {
        let root = env::temp_dir().join("rush_cli_add");
        let mut p = project_in(&root);

        assert_eq!(add_in(&mut p, Some("В API"), "api/new".into(), false).unwrap(), "api/new.rush");
        assert_eq!(p.content[2], E::dir("API", "api", vec![E::file("Общее", "api/b.rush"), E::file("В API", "api/new.rush")]));
        assert!(root.join("src/api/new.rush").is_file());

        // guide exists on disk but not in content: goes to the root
        assert_eq!(add_in(&mut p, Some("Вне"), "guide/x".into(), false).unwrap(), "guide/x.rush");
        assert_eq!(p.content[3], E::file("Вне", "guide/x.rush"));

        assert_eq!(add_in(&mut p, Some("Глубже"), "api/deep".into(), true).unwrap(), "api/deep");
        assert_eq!(add_in(&mut p, Some("Ещё"), "api/deep/y".into(), false).unwrap(), "api/deep/y.rush");
        let ContentEntryKind::Dir { childs } = &p.content[2].kind else { panic!() };
        assert_eq!(childs[2], E::dir("Глубже", "api/deep", vec![E::file("Ещё", "api/deep/y.rush")]));

        assert_eq!(add_in(&mut p, None, "Корень".into(), false).unwrap(), "Корень.rush");
        assert_eq!(p.content[4], E::file("Корень", "Корень.rush"));

        if let Err(AddError::Io(err)) = add_in(&mut p, Some("Снова"), "api/deep".into(), true){
            assert_eq!(err.kind(), io::ErrorKind::AlreadyExists)
        }
        else {
            panic!("Cannot handle already exts case")
        }
        assert!(add_in(&mut p, Some("Дубль"), "api/new".into(), false).is_err());
        fs::remove_dir_all(&root).unwrap();
    }
}
