use core::fmt;
use std::{io::Write, env, fs::{self, OpenOptions}, io, path::{Path, PathBuf}, process::ExitCode, slice};

use rush_lib::{DocStyle, Project};
use serde::Serialize;

use crate::config::{ConfigWarning, ContentEntry, ProjectFile, PROJECT_FILE};
use clap::{Arg, Parser, ValueEnum, Subcommand};

mod config;

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Style {
    /// document will be formatted in pure GOST 7.32, with all it pros and cons. 
    /// Perfect for printing, though less convenuent for digital view.
    Gost7_32,
    /// document will be formatted in more pleased to digital view way. 
    /// Perfect for digital articles.
    Modern,
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gost7_32 => write!(f, "gost_7_32"),
            Self::Modern => write!(f, "modern"),
        }
    }
}


#[derive(ValueEnum, Clone, Copy, Debug)]
enum OutType {
    /// Create HTML tree in output with single CSS file.
    /// In this out type all images will be copied to out directory.
    Html, 
    /// Create a single multipadge HTML+CSS file in output.
    /// In this out type all images will be copied to out directory.
    HtmlPadges,
    /// Create Markdown MDX tree in output
    /// In this out type all images will be copied to out directory.
    Markdown
}

impl fmt::Display for OutType { 
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Html => write!(f, "html"),
            Self::HtmlPadges => write!(f, "html-padges"),
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
        #[arg(short, long, value_enum, default_value_t = Style::Gost7_32)]
        /// Determines the style of the render.
        style: Style,
        #[arg(short, long, value_enum, default_value_t = OutType::Html)]
        /// Determines the render filetype.
        out_type: OutType,
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

    },
    /// Add new .rush file
    Add {
        /// Name of the new file
        name: String,
        /// Relative path to new file, from src. If not set will be crated in src root. With filename same as 'name'
        path: Option<PathBuf>
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
#[command(version, about)]
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
        Command::New { name, style, out_type, src_name, out_name } => {
            if let Err(e) = new(&name, &src_name, &out_name, style, out_type) {
                eprintln!("Failed to create new project: {e}");
                return  ExitCode::FAILURE;
            }
            println!("Successfully create new project '{name}'");
            return ExitCode::SUCCESS;
        },
        Command::Add { name, path } => {
            match add(&name, path) {
                Ok(path) => { 
                    println!("Successfully added new file: '{path}'");
                    return ExitCode::SUCCESS;
                },
                Err(e) => {
                    eprintln!("Failed to add new file: {e}");
                    return  ExitCode::FAILURE;
                }
            }
        }
        Command::Build { path, out } => {
            match build(path, out) {
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

fn new(name: &str, src_name: &str, out_name: &str, style: Style, out_type: OutType) -> io::Result<()> {
    let mut root = env::current_dir()?;
    root.push(file_name(name)?);
    let src_dir = root.join(src_name);
    let out_dir = root.join(out_name);
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&out_dir)?;

    let (src_name, out_name) = (toml_str(src_name), toml_str(out_name));
    let content = format!("[document]\nstyle=\"{style}\"\nout=\"{out_type}\"\n\n[build]\ninput={src_name}\noutput={out_name}\n\n");
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

fn add(name: &str, rel_path: Option<PathBuf>) -> io::Result<String> {
    let root = env::current_dir()?;
    let mut proj_file = OpenOptions::new().append(true).open(PROJECT_FILE).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found, command must be called from project directory. Create project first with 'rush-cli new <NAME>'")),
        _ => e,
    })?;

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


fn build(root: Option<PathBuf>, redef_out_type: Option<OutType>) -> io::Result<Vec<Warning>> {
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
            OutType::HtmlPadges => file.config.out = rush_lib::DocOut::Html,
            OutType::Markdown => file.config.out = rush_lib::DocOut::Markdown
        }
    }
    
    let mut warns: Vec<Warning> = config_warns.into_iter().map(Warning::from).collect();
    let (mut project, project_warns) = Project::index_dir_common(&file.input)?;
    warns.extend(project_warns.into_iter().map(Warning::from));
    for entry in file.content {
        project.add_content(entry.name, entry.path);
    }


    let (out_files, build_warns) = rush_lib::build(&file.config, &project).map_err(io::Error::other)?;
    warns.extend(build_warns.into_iter().map(Warning::from));

    for out in out_files {
        let dst = file.output.join(&out.path);
        create_parent(&dst)?;
        fs::write(dst, out.data)?;
    }
    for image in project.images() {
        let dst = file.output.join(image);
        create_parent(&dst)?;
        fs::copy(file.input.join(image), dst)?;
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
    Ok(())
}