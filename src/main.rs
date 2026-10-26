use core::fmt;
use std::{io::Write, env, fs::{self, OpenOptions}, io, path::{Path, PathBuf}, process::ExitCode, slice};

use rush_lib::{DocStyle, Project};
use serde::Serialize;

use crate::config::{ConfigWarning, ContentEntry, ProjectFile, PROJECT_FILE};
use clap::{Arg, Parser, ValueEnum, Subcommand};

mod config;

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Style {
    /// GOST - document will be formatted in pure GOST 7.32, with all it pros and cons. 
    /// Perfect for printing, though less convenuent for digital view.
    Gost,
    /// GOST like - document will look simmular to GOST 7.32, but more addapted for digital view.
    GostLike 
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gost => write!(f, "gost"),
            Self::GostLike => write!(f, "gostlike")
        }
    }
}


#[derive(ValueEnum, Clone, Copy, Debug)]
enum OutType {
    /// Depending on style on render project will create one or many html documents with single css file. 
    /// In this out type all images will be copied to out directory.
    /// If it possible in selected style in out directory file tree will be the same as in input.
    Html
}

impl fmt::Display for OutType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Html => write!(f, "html")
        }
    }
}


#[derive(Subcommand, Debug)]
enum Command {
    /// Create new project 
    New {
        /// Name of the project
        name: String,
        #[arg(value_enum, default_value_t = Style::GostLike)]
        /// Determines the style of the render.
        style: Style,
        #[arg(value_enum, default_value_t = OutType::Html)]
        /// Determines the render filetype.
        out_type: OutType,
        #[arg(short, long, default_value = "src")]
        /// How to name src folder
        src_name: String,
        #[arg(short, long, default_value = "out")]
        /// How to name out folder
        out_name: String
    },
    /// Build current project
    Build {
        #[arg(value_enum, long)]
        /// Render to differnt output type, withou changing type in 'project.toml'
        out: Option<OutType>
    },
    /// Add new .rush file
    Add {
        /// Name of the new file
        name: String,
        /// Relative path to new file, from src. If not set will be crated in src root. With filename same as 'name'
        path: Option<PathBuf>
    }
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
        Command::Build { out } => {
            match build(out) {
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


fn build(redef_out_type: Option<OutType>) -> io::Result<Vec<Warning>> {
    let root = env::current_dir()?;
    let (mut file, config_warns) = ProjectFile::load(&root).map_err(|e|
        match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("{PROJECT_FILE} not found, command must be called from project directory. Create project first with 'rush-cli new <NAME>'")),
            _ => e,
        }
    )?;
    if let Some(redef_out_type ) = redef_out_type {
        match redef_out_type {
            OutType::Html => file.config.out = rush_lib::DocOut::Html
        }
    }
    

    let mut warns: Vec<Warning> = config_warns.into_iter().map(Warning::from).collect();

    let (mut project, project_warns) = Project::index_dir_common(&file.input)?;
    println!("{:?}", project); 
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
