use std::{fs, io, path::{Component, Path, PathBuf}};

/// List of files from the previous build, used when the output overlaps the sources
const MANIFEST: &str = ".rush-build";

/// Build folders in a normalized form: `root.join("")` and `root` must be equal
pub(crate) struct Dirs {
    pub(crate) input: PathBuf,
    output: PathBuf,
    root: PathBuf,
}

impl Dirs {
    pub(crate) fn resolve(root: &Path, input: &Path, output: &Path) -> io::Result<Self> {
        fs::create_dir_all(output)?;
        Ok(Self { input: input.canonicalize()?, output: output.canonicalize()?, root: root.canonicalize()? })
    }

    pub(crate) fn same(&self) -> bool {
        self.input == self.output
    }

    /// Output inside the sources: its contents are not indexed
    pub(crate) fn nested_output(&self) -> Option<&Path> {
        (!self.same() && self.output.starts_with(&self.input)).then_some(self.output.as_path())
    }

    /// Other files may lie next to the output, only files from our own build can be deleted
    pub(crate) fn overlap(&self) -> bool {
        self.output.starts_with(&self.input) || self.input.starts_with(&self.output) || self.root.starts_with(&self.output)
    }

    pub(crate) fn clean(&self) -> io::Result<()> {
        if self.overlap() {
            self.remove_listed()
        } else {
            self.remove_all()
        }
    }

    fn remove_all(&self) -> io::Result<()> {
        for entry in fs::read_dir(&self.output)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                fs::remove_dir_all(entry.path())?;
            } else {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    fn remove_listed(&self) -> io::Result<()> {
        let manifest = self.output.join(MANIFEST);
        let list = match fs::read_to_string(&manifest) {
            Ok(list) => list,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        for rel in list.lines().filter_map(inside_output) {
            let path = self.output.join(rel);
            match fs::remove_file(&path) {
                Ok(()) => self.remove_empty_parents(&path),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e),
            }
        }
        fs::remove_file(manifest)
    }

    fn remove_empty_parents(&self, path: &Path) {
        let parents = path.ancestors().skip(1).take_while(|dir| *dir != self.output);
        for dir in parents {
            if fs::remove_dir(dir).is_err() {
                break;
            }
        }
    }

    pub(crate) fn save_manifest(&self, files: &[String]) -> io::Result<()> {
        let mut list = files.join("\n");
        list.push('\n');
        fs::write(self.output.join(MANIFEST), list)
    }
}

/// Path from the manifest; anything that could lead outside the output is dropped
fn inside_output(line: &str) -> Option<PathBuf> {
    let path = Path::new(line);
    let normal = path.components().all(|c| matches!(c, Component::Normal(_)));
    (normal && !line.is_empty()).then(|| path.components().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);

    impl Tmp {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("rush_cli_clean_{name}"));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self, rel: &str) {
            let path = self.0.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }

        fn has(&self, rel: &str) -> bool {
            self.0.join(rel).exists()
        }

        fn dirs(&self, input: &str, output: &str) -> Dirs {
            Dirs::resolve(&self.0, &self.0.join(input), &self.0.join(output)).unwrap()
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn separate_dirs_are_cleaned_fully() {
        let t = Tmp::new("separate");
        t.file("project.yaml");
        t.file("src/a.rush");
        t.file("out/old.html");
        t.file("out/fonts/a.ttf");
        let dirs = t.dirs("src", "out");
        assert!(!dirs.overlap());
        dirs.clean().unwrap();
        assert!(t.has("out") && !t.has("out/old.html") && !t.has("out/fonts"));
        assert!(t.has("src/a.rush") && t.has("project.yaml"));
    }

    #[test]
    fn same_dir_removes_only_listed() {
        let t = Tmp::new("same");
        for f in ["project.yaml", "a.rush", "pic.png", "mine.md", "a.md", "fonts/a.ttf", "keep/x.ttf", "keep/mine.txt", ".git/HEAD"] {
            t.file(f);
        }
        let dirs = t.dirs("", "");
        assert!(dirs.same() && dirs.overlap() && dirs.nested_output().is_none());
        dirs.save_manifest(&["a.md".into(), "fonts/a.ttf".into(), "keep/x.ttf".into(), "gone.md".into()]).unwrap();
        dirs.clean().unwrap();

        assert!(!t.has("a.md") && !t.has("fonts") && !t.has("keep/x.ttf") && !t.has(MANIFEST));
        for f in ["project.yaml", "a.rush", "pic.png", "mine.md", "keep/mine.txt", ".git/HEAD"] {
            assert!(t.has(f), "{f}");
        }
    }

    #[test]
    fn first_build_without_manifest_keeps_all() {
        let t = Tmp::new("first");
        t.file("a.rush");
        t.file("a.md");
        t.dirs("", "").clean().unwrap();
        assert!(t.has("a.rush") && t.has("a.md"));
    }

    #[test]
    fn output_inside_input() {
        let t = Tmp::new("nested_out");
        t.file("docs/a.rush");
        t.file("docs/out/a.html");
        t.file("docs/out/mine.txt");
        let dirs = t.dirs("docs", "docs/out");
        assert!(dirs.overlap());
        assert_eq!(dirs.nested_output(), Some(dirs.output.as_path()));
        dirs.save_manifest(&["a.html".into()]).unwrap();
        dirs.clean().unwrap();
        assert!(!t.has("docs/out/a.html") && t.has("docs/out/mine.txt") && t.has("docs/a.rush"));
    }

    #[test]
    fn input_inside_output() {
        let t = Tmp::new("nested_in");
        t.file("project.yaml");
        t.file("src/a.rush");
        t.file("a.html");
        let dirs = t.dirs("src", "");
        assert!(dirs.overlap() && dirs.nested_output().is_none());
        dirs.save_manifest(&["a.html".into()]).unwrap();
        dirs.clean().unwrap();
        assert!(!t.has("a.html") && t.has("src/a.rush") && t.has("project.yaml"));
    }

    #[test]
    fn project_root_inside_output_is_overlap() {
        let t = Tmp::new("root_in");
        t.file("proj/project.yaml");
        t.file("elsewhere/a.rush");
        let dirs = Dirs::resolve(&t.0.join("proj"), &t.0.join("elsewhere"), &t.0.join("proj")).unwrap();
        assert!(dirs.overlap());
    }

    #[test]
    fn manifest_cannot_escape_output() {
        let t = Tmp::new("escape");
        t.file("secret.txt");
        t.file("out/src/a.rush");
        let abs = t.0.join("secret.txt").to_string_lossy().into_owned();
        let dirs = t.dirs("out/src", "out");
        dirs.save_manifest(&["../secret.txt".into(), "src/../../secret.txt".into(), abs, String::new()]).unwrap();
        dirs.clean().unwrap();
        assert!(t.has("secret.txt"));
    }
}
