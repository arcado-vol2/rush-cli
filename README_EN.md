# rush

[**РУССКАЯ ВЕРСИЯ**](README.md)

**Rush** — "RU Simple Help", "Russian Student Helper", "Russian Utility Syntax for Humans", "because RUssian Students Hate formatting". Call it what you like, the idea stays the same.

**Rush** — a minimalist markup language designed specifically for formatting texts in Russian without switching the keyboard layout.

This repository wraps [rush-lib](https://github.com/arcado-vol2/rush-lib) in a command-line application that lets you manage rush projects quickly.

For now the installer is built only for Windows with [Inno Setup](https://jrsoftware.org/isinfo.php). On other operating systems you can build the application from source:

```
git clone https://github.com/arcado-vol2/rush-cli.git
cd rush-cli
git submodule update --init --recursive
cargo build --release
```

The `rush` executable will be in `target/release/`. You will need to add it to `PATH` yourself.

## Quick start

After installing and adding it to `PATH`, create a new project:

```
rush new <PROJECT NAME>
```

This creates a `<PROJECT NAME>` directory with an `out` directory (your files appear here after the build), a `src` directory (sources go here) and a `project.yaml` file that holds all build settings.

```
cd <PROJECT NAME>
```

Then add a new file with

```
rush add <FILE NAME>
```

Write text in it using the [rush syntax](https://github.com/arcado-vol2/rush-lib/blob/master/README.md).

Then build the project:

```
rush build
```

An html output file will appear in the `out` directory.

> To try out the language features, build `examples/example`

## project.yaml parameters

1  `document` parameters

| Name | Possible values | What it does | Note |
| :-: | :-: | :-: | :-: |
| `style` | `blog`/`modern`/`gost7-32` | Defines how the output file looks | Styles are described below |
| `out` | `html`/`markdown` | Defines the output format of the project | For `html` the fonts are also put into the output directory |
| `single` | `true`/`false` | If `false`, each `.rush` document gets its own output document and, where possible, a navigation panel between documents. If `true`, the whole project is built into one file | For `html` it is a paged `index.html`, for `markdown` — `index.md`. Only files from `content` are included, in their order |
| `no_js` | `true`/`false` | With `html` output the `modern` and `blog` styles can contain JavaScript features (side hamburger menu, code copy buttons, theme switch button). Set `no_js: true` to turn them off when the output goes where JavaScript is unreliable | Copy buttons and the hamburger menu are in `modern` and `blog`, the theme switch is only in `blog` |
| `theme` | `auto`/`dark`/`light` | Defines the starting theme of the document; with `no_js: true` the document always keeps it | Different themes are available only in `blog` |

**Styles**

- `blog` — a style inspired by tech blog articles and web documentation: generous spacing, a search menu and a clean modern look. The main style for the web, supports light and dark themes.
- `modern` — a style for printing modern academic papers on A4: sans-serif font, color contrasts.
- `gost7-32` — a style implementing the Russian `GOST 7.32` — the standard for academic and scientific papers printed on A4. It is the standard for lab reports in Russian universities.

2  `build` parameters

| Name | Possible values | What it does | Note |
| :-: | :-: | :-: | :-: |
| `input` | Any relative path | Defines the directory with the source text files and images | To use the project root, set `.` |
| `output` | Any relative path | Defines the directory where files are saved after the build | The directory is cleaned before the build. If it overlaps with `input` or the project root, only files from the previous build are removed: their list is kept in `.rush-build` |

**`content`** — the project contents, used to build the navigation. Every entry requires `kind`, `name` and `path`. `kind` is either `dir` or `file`; a `kind: dir` entry may also have `content` with nested entries.

## CLI commands

All commands except `new` are run from the project directory, where `project.yaml` is. Help for any command is available with `rush <COMMAND> --help`, the version with `rush --version`.

### `new`

Creates a new project in the current directory.

```
rush new <NAME> [OPTIONS]
```

*Arguments*

**`<NAME>`** — required, the name of the project and its directory.

**`-s, --style <STYLE>`** — project style: `blog`, `modern` or `gost7-32`. Defaults to `blog`.

**`-o, --out-type <OUTPUT TYPE>`** — project output format: `html` or `markdown`. Defaults to `html`.

**`--single`** — build the whole project into one file.

**`--no-js`** — build `html` without JavaScript.

**`--theme <THEME>`** — starting theme: `auto`, `light` or `dark`. Defaults to `auto`.

**`--src-name <NAME>`** — name of the source directory. Defaults to `src`.

**`--out-name <NAME>`** — name of the output directory. Defaults to `out`.

### `build`

Builds the project. The options override the settings for one build without changing `project.yaml`.

```
rush build [PATH] [OPTIONS]
```

*Arguments*

**`[PATH]`** — path to the project directory. Defaults to the current directory.

**`--out <OUTPUT TYPE>`** — build to `html` or `markdown`.

**`--single <true|false>`** — build into one file or into one file per document.

**`--no-js <true|false>`** — build with or without JavaScript.

**`--theme <THEME>`** — build with the `auto`, `light` or `dark` theme.

### `add`

Creates a new `.rush` file or a new directory and adds its entry to `content`.

```
rush add <PATH> [NAME] [--dir]
```

*Arguments*

**`<PATH>`** — path to the new file relative to the source directory. The `.rush` extension is added automatically, other extensions are rejected. If the path leads into a directory that is already in `content`, the entry goes there, otherwise to the root of `content`.

**`[NAME]`** — name of the file or directory in the navigation. Defaults to the file name without extension.

**`--dir`** — create a directory instead of a file.

### `rename`

Renames a file or directory in the navigation; files on disk stay as they are.

```
rush rename <OLD NAME> <NEW NAME>
```

*Arguments*

**`<OLD NAME>`** — current name in `content`. If there is no such name, a directory with that name is searched on disk and added to `content` under the new name.

**`<NEW NAME>`** — the new name.

If the name is found in several places, the command changes nothing and lists the matches.

### `remove`

Removes a `.rush` file from `content` and from disk.

```
rush remove <NAME>
```

*Arguments*

**`<NAME>`** — name of the file in `content`. If the file is already missing on disk, only the entry is removed.

### `list`

Prints the `content` tree: the name and path of every entry.

```
rush list
```
