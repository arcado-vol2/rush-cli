use core::fmt;
use std::{io::Write, env, fs::{self, OpenOptions}, io, path::{Path, PathBuf}, process::ExitCode, slice};

use rush_lib::{Project, Theme};
use serde::Serialize;

use crate::clean::Dirs;
use crate::config::{ConfigWarning, ContentEntry, Named, ProjectFile, PROJECT_FILE};
use clap::{Parser, ValueEnum, Subcommand};

mod clean;
mod config;

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Style {
    /// Pure GOST 7.32: the external standard for academic and scientific papers printed on A4.
    Gost7_32,
    /// Readable academic paper: prints on A4 as well as GOST 7.32,
    /// but with serif text and warm markers the eye catches easily.
    Modern,
    /// Main modern web format: documentation, technical books and articles.
    /// Supports light and dark themes.
    Blog,
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gost7_32 => write!(f, "gost_7_32"),
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
    /// Create HTML tree in output with single CSS file,
    /// or one multipage HTML file if the project is single.
    /// In this out type all images will be copied to out directory.
    Html,
    /// Create Markdown MDX tree in output,
    /// or one Markdown file if the project is single.
    /// In this out type all images will be copied to out directory.
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
    /// Create new project
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
        /// Render the whole project to one file, instead of one file per .rush file.
        single: bool,
        #[arg(long)]
        /// Static output without scripts: no menu button, theme is fixed.
        no_js: bool,
        #[arg(long, value_enum, default_value_t = ThemeArg::Auto)]
        /// Starting theme; fixed one without scripts.
        theme: ThemeArg,
        #[arg(long, default_value = "src")]
        /// How to name src folder
        src_name: String,
        #[arg(long, default_value = "out")]
        /// How to name out folder
        out_name: String
    },
    /// Build current project
    Build {
        /// Target path. If not set, the current directory is used.
        path: Option<PathBuf>,

        #[arg(value_enum, long)]
        /// Render to differnt output type, without changing type in 'project.toml'
        out: Option<OutType>,

        #[arg(long)]
        /// Render to one file or to one file per .rush file, without changing 'single' in 'project.toml'
        single: Option<bool>,

        #[arg(long)]
        /// Render with or without scripts, without changing 'no_js' in 'project.toml'
        no_js: Option<bool>,

        #[arg(value_enum, long)]
        /// Render with another theme, without changing 'theme' in 'project.toml'
        theme: Option<ThemeArg>,

    },
    /// Add new .rush file, or a folder with --dir
    Add {
        /// Name of the new file or folder
        name: String,
        /// Relative path to new file, from src. If not set will be crated in src root. With filename same as 'name'
        path: Option<PathBuf>,
        #[arg(long)]
        /// Create a folder and give it the name in the menu
        dir: bool,
    },
    /// Rename a file or a folder in the menu, the files on disk stay as they are
    Rename {
        /// Current name in content or folders, or a folder name on disk
        old_name: String,
        new_name: String,
    },
    /// Remove .rush file from project
    Remove {
        /// Name of the file in project
        name: String
    },
    /// Show all .rush files in project
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
            println!("Successfully create new project '{name}'");
            return ExitCode::SUCCESS;
        },
        Command::Add { name, path, dir } => {
            match add(&name, path, dir) {
                Ok(path) => {
                    println!("Successfully added new {}: '{path}'", if dir { "folder" } else { "file" });
                    return ExitCode::SUCCESS;
                },
                Err(e) => {
                    eprintln!("Failed to add new file: {e}");
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
                    println!("Project builded! {} warinings", warnings.len());
                    return ExitCode::SUCCESS;
                },
                Err(e) => {
                    eprintln!("Failed to build a project: {e}");
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
                    println!("Not founded file '{name}' in project");
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
                eprintln!("Fauled to list a project: {e}");
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

    let (src_name, out_name) = (toml_str(src_name), toml_str(out_name));
    let theme = config::theme_name(theme.into());
    let content = format!("[document]\nstyle=\"{style}\"\nout=\"{out_type}\"\nsingle={single}\nno_js={no_js}\ntheme=\"{theme}\"\n\n[build]\ninput={src_name}\noutput={out_name}\n\n");
    fs::write(root.join(PROJECT_FILE), content)?;
    Ok(())
}

fn toml_str(s: &str) -> String {
    toml::Value::from(s).to_string()
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

#[derive(Serialize)]
struct ContentAppend<'a> {
    content: &'a [ContentEntry],
}

#[derive(Serialize)]
struct DirsAppend<'a> {
    dirs: &'a [ContentEntry],
}

fn add(name: &str, rel_path: Option<PathBuf>, dir: bool) -> io::Result<String> {
    let root = env::current_dir()?;
    let mut proj_file = OpenOptions::new().append(true).open(PROJECT_FILE).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found, command must be called from project directory. Create project first with 'rush-cli new <NAME>'")),
        _ => e,
    })?;

    if dir {
        let rel = match rel_path {
            Some(path) => path,
            None => PathBuf::from(file_name(name)?),
        };
        let (project, _) = ProjectFile::load(&root)?;
        fs::create_dir_all(project.input.join(&rel))?;
        let path = slashed(&rel);
        let entry = ContentEntry { name: name.to_owned(), path: path.clone() };
        let table = toml::to_string(&DirsAppend { dirs: slice::from_ref(&entry) }).map_err(io::Error::other)?;
        write!(proj_file, "\n{table}")?;
        return Ok(path);
    }
    let rel = match rel_path {
        Some(mut path) => {
            path.set_extension("rush");
            path
        }
        None => PathBuf::from(format!("{}.rush", file_name(name)?)),
    };
    let (project, _) = ProjectFile::load(&root)?;
    let path = project.input.join(&rel);
    create_parent(&path)?;
    OpenOptions::new().write(true).create_new(true).open(&path)?;
    let path = slashed(&rel);
    let entry = ContentEntry { name: name.to_owned(), path: path.clone()};
    let table = toml::to_string(&ContentAppend { content: slice::from_ref(&entry) }).map_err(io::Error::other)?;
    write!(proj_file, "\n{table}")?;
    Ok(path)
}


fn build(root: Option<PathBuf>, redef_out_type: Option<OutType>, redef_single: Option<bool>, redef_no_js: Option<bool>, redef_theme: Option<Theme>) -> io::Result<Vec<Warning>> {
    let root = root.map_or(env::current_dir()?, |root| root);
    let (mut file, config_warns) = ProjectFile::load(&root).map_err(|e|
        match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found, command must be called from project directory. Create project first with 'rush-cli new <NAME>'")),
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
    for entry in file.content {
        project.add_content(entry.name, entry.path);
    }
    for entry in file.dirs {
        project.add_dir(entry.name, entry.path);
    }


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
    let (mut project, _) = ProjectFile::load(&root).map_err(|e|
        match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found, command must be called from project directory. Create project first with 'rush-cli new <NAME>'")),
            _ => e,
        }
    )?;
    let Some((pos, entry)) = project.remove_content_by_name(name) else {
        return Ok(Removed::NotInProject);
    };
    project.save(&root)?;

    match fs::remove_file(project.input.join(&entry.path)) {
        Ok(()) => Ok(Removed::Deleted),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Removed::MissingOnDisk),
        Err(e) => {
            project.content.insert(pos, entry);
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
            let entry = project.entry_mut(*at);
            entry.name = new_name.to_owned();
            Ok(entry.path.clone())
        }
        [] => {
            let on_disk = dirs_named(&project.input, &project.output, old_name)?;
            match on_disk.as_slice() {
                [path] => {
                    project.dirs.push(ContentEntry { name: new_name.to_owned(), path: path.clone() });
                    Ok(path.clone())
                }
                [] => Err(io::Error::new(io::ErrorKind::NotFound, format!("'{old_name}' not found in content, dirs or source folders"))),
                many => Err(ambiguous(old_name, &many.iter().map(|p| format!("folder ./{p}")).collect::<Vec<_>>())),
            }
        }
        many => {
            let places: Vec<String> = many.iter().map(|at| match at {
                Named::Content(i) => format!("content ./{}", project.content[*i].path),
                Named::Dir(i) => format!("dirs ./{}", project.dirs[*i].path),
            }).collect();
            Err(ambiguous(old_name, &places))
        }
    }
}

fn rename(old_name: &str, new_name: &str) -> io::Result<String> {
    let root = env::current_dir()?;
    let (mut project, _) = ProjectFile::load(&root).map_err(|e|
        match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found, command must be called from project directory. Create project first with 'rush-cli new <NAME>'")),
            _ => e,
        }
    )?;
    let path = rename_in(&mut project, old_name, new_name)?;
    project.save(&root)?;
    Ok(path)
}

fn list() -> io::Result<()>{
    let root = env::current_dir()?;
    let (project, _) = ProjectFile::load(&root).map_err(|e|
        match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found, command must be called from project directory. Create project first with 'rush-cli new <NAME>'")),
            _ => e,
        }
    )?;
    for content in project.content {
        println!("\"{}\": ./{}", content.name, content.path)
    }
    for dir in project.dirs {
        println!("\"{}\": ./{}/", dir.name, dir.path)
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, path: &str) -> ContentEntry {
        ContentEntry { name: name.into(), path: path.into() }
    }

    fn project_in(root: &Path) -> ProjectFile {
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(root.join("src/guide/deep")).unwrap();
        fs::create_dir_all(root.join("src/api/deep")).unwrap();
        fs::create_dir_all(root.join("src/out/guide")).unwrap();
        ProjectFile {
            input: root.join("src"),
            output: root.join("src/out"),
            content: vec![entry("Введение", "intro.rush"), entry("Общее", "a.rush"), entry("Общее", "b.rush")],
            dirs: vec![entry("API", "api")],
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
        assert_eq!(p.dirs[0].name, "Справочник");
        assert_eq!(rename_in(&mut p, "guide", "Руководство").unwrap(), "guide");
        assert_eq!(p.dirs[1], entry("Руководство", "guide"));

        let ambiguous = rename_in(&mut p, "Общее", "X").unwrap_err();
        assert!(ambiguous.to_string().contains("content ./a.rush, content ./b.rush"), "{ambiguous}");
        let on_disk = rename_in(&mut p, "deep", "X").unwrap_err();
        assert!(on_disk.to_string().contains("folder ./api/deep, folder ./guide/deep"), "{on_disk}");
        assert_eq!(rename_in(&mut p, "Нет", "X").unwrap_err().kind(), io::ErrorKind::NotFound);
        fs::remove_dir_all(&root).unwrap();
    }
}