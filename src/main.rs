use std::{env, fs, io, path::{Path, PathBuf}, process::ExitCode};

use rush_lib::Project;

use crate::config::{ConfigWarning, ProjectFile};

mod config;

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
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: rush <project_path>");
        return ExitCode::from(2);
    };

    match build(&PathBuf::from(path)) {
        Ok(warnings) => {
            for w in &warnings {
                println!("{w:?}");
            }
            println!("{} warinings", warnings.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Build error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn build(root: &Path) -> io::Result<Vec<Warning>> {
    let (file, config_warns) = ProjectFile::load(root);
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
