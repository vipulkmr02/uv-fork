//! Create a virtual environment.

use std::env::consts::EXE_SUFFIX;
use std::io;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use fs_err as fs;
use fs_err::File;
use itertools::Itertools;
use tracing::debug;

use uv_fs::{cachedir, Simplified, CWD};
use uv_pypi_types::Scheme;
use uv_python::managed::create_bin_link;
use uv_python::{Interpreter, VirtualEnvironment};
use uv_shell::escape_posix_for_single_quotes;
use uv_version::version;

use crate::{Error, Prompt};

/// Activation scripts for the environment, with dependent paths templated out.
const ACTIVATE_TEMPLATES: &[(&str, &str)] = &[
    ("activate", include_str!("activator/activate")),
    ("activate.csh", include_str!("activator/activate.csh")),
    ("activate.fish", include_str!("activator/activate.fish")),
    ("activate.nu", include_str!("activator/activate.nu")),
    ("activate.ps1", include_str!("activator/activate.ps1")),
    ("activate.bat", include_str!("activator/activate.bat")),
    ("deactivate.bat", include_str!("activator/deactivate.bat")),
    ("pydoc.bat", include_str!("activator/pydoc.bat")),
    (
        "activate_this.py",
        include_str!("activator/activate_this.py"),
    ),
];
const VIRTUALENV_PATCH: &str = include_str!("_virtualenv.py");

/// Very basic `.cfg` file format writer.
fn write_cfg(f: &mut impl Write, data: &[(String, String)]) -> io::Result<()> {
    for (key, value) in data {
        writeln!(f, "{key} = {value}")?;
    }
    Ok(())
}

/// Create a [`VirtualEnvironment`] at the given location.
#[allow(clippy::fn_params_excessive_bools)]
pub(crate) fn create(
    location: &Path,
    interpreter: &Interpreter,
    prompt: Prompt,
    system_site_packages: bool,
    allow_existing: bool,
    relocatable: bool,
    seed: bool,
) -> Result<VirtualEnvironment, Error> {
    // Determine the base Python executable; that is, the Python executable that should be
    // considered the "base" for the virtual environment.
    //
    // For consistency with the standard library, rely on `sys._base_executable`, _unless_ we're
    // using a uv-managed Python (in which case, we can do better for symlinked executables).
    let base_python = if cfg!(unix) && interpreter.is_standalone() {
        interpreter.find_base_python()?
    } else {
        interpreter.to_base_python()?
    };

    debug!(
        "Using base executable for virtual environment: {}",
        base_python.display()
    );

    // Validate the existing location.
    match location.metadata() {
        Ok(metadata) => {
            if metadata.is_file() {
                return Err(Error::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("File exists at `{}`", location.user_display()),
                )));
            } else if metadata.is_dir() {
                if allow_existing {
                    debug!("Allowing existing directory");
                } else if location.join("pyvenv.cfg").is_file() {
                    debug!("Removing existing directory");

                    // On Windows, if the current executable is in the directory, guard against
                    // self-deletion.
                    #[cfg(windows)]
                    if let Ok(itself) = std::env::current_exe() {
                        let target = std::path::absolute(location)?;
                        if itself.starts_with(&target) {
                            debug!("Detected self-delete of executable: {}", itself.display());
                            self_replace::self_delete_outside_path(location)?;
                        }
                    }

                    fs::remove_dir_all(location)?;
                    fs::create_dir_all(location)?;
                } else if location
                    .read_dir()
                    .is_ok_and(|mut dir| dir.next().is_none())
                {
                    debug!("Ignoring empty directory");
                } else {
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!(
                            "The directory `{}` exists, but it's not a virtual environment",
                            location.user_display()
                        ),
                    )));
                }
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(location)?;
        }
        Err(err) => return Err(Error::Io(err)),
    }

    let location = std::path::absolute(location)?;

    let bin_name = if cfg!(unix) {
        "bin"
    } else if cfg!(windows) {
        "Scripts"
    } else {
        unimplemented!("Only Windows and Unix are supported")
    };
    let scripts = location.join(&interpreter.virtualenv().scripts);
    let prompt = match prompt {
        Prompt::CurrentDirectoryName => CWD
            .file_name()
            .map(|name| name.to_string_lossy().to_string()),
        Prompt::Static(value) => Some(value),
        Prompt::None => None,
    };

    // Add the CACHEDIR.TAG.
    cachedir::ensure_tag(&location)?;

    // Create a `.gitignore` file to ignore all files in the venv.
    fs::write(location.join(".gitignore"), "*")?;

    let executable_target = if interpreter.is_managed() {
        interpreter.symlink_path_from_base_python(base_python.clone())?
    } else {
        base_python.clone()
    };

    // Per PEP 405, the Python `home` is the parent directory of the interpreter.
    // For managed interpreters, this `home` value will include a symlink directory
    // on Unix or junction on Windows to enable transparent Python patch upgrades.
    let python_home = executable_target
        .parent()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "The Python interpreter needs to have a parent directory",
            )
        })?
        .to_path_buf();
    let python_home = python_home.as_path();

    // Different names for the python interpreter
    fs::create_dir_all(&scripts)?;
    let executable = scripts.join(format!("python{EXE_SUFFIX}"));

    #[cfg(unix)]
    {
        uv_fs::replace_symlink(&executable_target, &executable)?;
        uv_fs::replace_symlink(
            "python",
            scripts.join(format!("python{}", interpreter.python_major())),
        )?;
        uv_fs::replace_symlink(
            "python",
            scripts.join(format!(
                "python{}.{}",
                interpreter.python_major(),
                interpreter.python_minor(),
            )),
        )?;

        if interpreter.markers().implementation_name() == "pypy" {
            uv_fs::replace_symlink(
                "python",
                scripts.join(format!("pypy{}", interpreter.python_major())),
            )?;
            uv_fs::replace_symlink("python", scripts.join("pypy"))?;
        }

        if interpreter.markers().implementation_name() == "graalpy" {
            uv_fs::replace_symlink("python", scripts.join("graalpy"))?;
        }
    }

    // On Windows, we use trampolines that point to our junction-containing executable link.
    // TODO(john): I think we can do this directly with junctions.
    if cfg!(windows) {
        create_venv_trampoline_windows(
            &executable_target,
            &[WindowsExecutable::Python],
            interpreter,
            &scripts,
        )?;

        if interpreter.markers().implementation_name() == "graalpy" {
            create_venv_trampoline_windows(
                &executable_target,
                &[
                    WindowsExecutable::GraalPy,
                    WindowsExecutable::PythonMajor,
                    WindowsExecutable::Pythonw,
                ],
                interpreter,
                &scripts,
            )?;
        }

        if interpreter.markers().implementation_name() == "pypy" {
            create_venv_trampoline_windows(
                &executable_target,
                &[
                    WindowsExecutable::PythonMajor,
                    WindowsExecutable::PythonMajorMinor,
                    WindowsExecutable::PyPy,
                    WindowsExecutable::PyPyMajor,
                    WindowsExecutable::PyPyMajorMinor,
                    WindowsExecutable::PyPyw,
                    WindowsExecutable::PyPyMajorMinorw,
                ],
                interpreter,
                &scripts,
            )?;
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        compile_error!("Only Windows and Unix are supported")
    }

    // Add all the activate scripts for different shells
    for (name, template) in ACTIVATE_TEMPLATES {
        let path_sep = if cfg!(windows) { ";" } else { ":" };

        let relative_site_packages = [
            interpreter.virtualenv().purelib.as_path(),
            interpreter.virtualenv().platlib.as_path(),
        ]
        .iter()
        .dedup()
        .map(|path| {
            pathdiff::diff_paths(path, &interpreter.virtualenv().scripts)
                .expect("Failed to calculate relative path to site-packages")
        })
        .map(|path| path.simplified().to_str().unwrap().replace('\\', "\\\\"))
        .join(path_sep);

        let virtual_env_dir = match (relocatable, name.to_owned()) {
            (true, "activate") => {
                r#"'"$(dirname -- "$(dirname -- "$(realpath -- "$SCRIPT_PATH")")")"'"#.to_string()
            }
            (true, "activate.bat") => r"%~dp0..".to_string(),
            (true, "activate.fish") => {
                r#"'"$(dirname -- "$(cd "$(dirname -- "$(status -f)")"; and pwd)")"'"#.to_string()
            }
            // Note:
            // * relocatable activate scripts appear not to be possible in csh and nu shell
            // * `activate.ps1` is already relocatable by default.
            _ => escape_posix_for_single_quotes(location.simplified().to_str().unwrap()),
        };

        let activator = template
            .replace("{{ VIRTUAL_ENV_DIR }}", &virtual_env_dir)
            .replace("{{ BIN_NAME }}", bin_name)
            .replace(
                "{{ VIRTUAL_PROMPT }}",
                prompt.as_deref().unwrap_or_default(),
            )
            .replace("{{ PATH_SEP }}", path_sep)
            .replace("{{ RELATIVE_SITE_PACKAGES }}", &relative_site_packages);
        fs::write(scripts.join(name), activator)?;
    }

    let mut pyvenv_cfg_data: Vec<(String, String)> = vec![
        (
            "home".to_string(),
            python_home.simplified_display().to_string(),
        ),
        (
            "implementation".to_string(),
            interpreter
                .markers()
                .platform_python_implementation()
                .to_string(),
        ),
        ("uv".to_string(), version().to_string()),
        (
            "version_info".to_string(),
            interpreter.markers().python_version().string.clone(),
        ),
        (
            "include-system-site-packages".to_string(),
            if system_site_packages {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
    ];

    if relocatable {
        pyvenv_cfg_data.push(("relocatable".to_string(), "true".to_string()));
    }

    if seed {
        pyvenv_cfg_data.push(("seed".to_string(), "true".to_string()));
    }

    if let Some(prompt) = prompt {
        pyvenv_cfg_data.push(("prompt".to_string(), prompt));
    }

    if cfg!(windows) && interpreter.markers().implementation_name() == "graalpy" {
        pyvenv_cfg_data.push((
            "venvlauncher_command".to_string(),
            python_home
                .join("graalpy.exe")
                .simplified_display()
                .to_string(),
        ));
    }

    let mut pyvenv_cfg = BufWriter::new(File::create(location.join("pyvenv.cfg"))?);
    write_cfg(&mut pyvenv_cfg, &pyvenv_cfg_data)?;
    drop(pyvenv_cfg);

    // Construct the path to the `site-packages` directory.
    let site_packages = location.join(&interpreter.virtualenv().purelib);
    fs::create_dir_all(&site_packages)?;

    // If necessary, create a symlink from `lib64` to `lib`.
    // See: https://github.com/python/cpython/blob/b228655c227b2ca298a8ffac44d14ce3d22f6faa/Lib/venv/__init__.py#L135C11-L135C16
    #[cfg(unix)]
    if interpreter.pointer_size().is_64()
        && interpreter.markers().os_name() == "posix"
        && interpreter.markers().sys_platform() != "darwin"
    {
        match std::os::unix::fs::symlink("lib", location.join("lib64")) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    // Populate `site-packages` with a `_virtualenv.py` file.
    fs::write(site_packages.join("_virtualenv.py"), VIRTUALENV_PATCH)?;
    fs::write(site_packages.join("_virtualenv.pth"), "import _virtualenv")?;

    Ok(VirtualEnvironment {
        scheme: Scheme {
            purelib: location.join(&interpreter.virtualenv().purelib),
            platlib: location.join(&interpreter.virtualenv().platlib),
            scripts: location.join(&interpreter.virtualenv().scripts),
            data: location.join(&interpreter.virtualenv().data),
            include: location.join(&interpreter.virtualenv().include),
        },
        root: location,
        executable,
        base_executable: base_python,
    })
}

#[derive(Debug, Copy, Clone)]
enum WindowsExecutable {
    /// The `python.exe` executable (or `venvlauncher.exe` launcher shim).
    Python,
    /// The `python3.exe` executable (or `venvlauncher.exe` launcher shim).
    PythonMajor,
    /// The `python3.<minor>.exe` executable (or `venvlauncher.exe` launcher shim).
    PythonMajorMinor,
    /// The `pythonw.exe` executable (or `venvwlauncher.exe` launcher shim).
    Pythonw,
    /// The `pypy.exe` executable.
    PyPy,
    /// The `pypy3.exe` executable.
    PyPyMajor,
    /// The `pypy3.<minor>.exe` executable.
    PyPyMajorMinor,
    /// The `pypyw.exe` executable.
    PyPyw,
    /// The `pypy3.<minor>w.exe` executable.
    PyPyMajorMinorw,
    // The `graalpy.exe` executable
    GraalPy,
}

impl WindowsExecutable {
    /// The name of the Python executable.
    fn exe(self, interpreter: &Interpreter) -> String {
        match self {
            WindowsExecutable::Python => String::from("python.exe"),
            WindowsExecutable::PythonMajor => {
                format!("python{}.exe", interpreter.python_major())
            }
            WindowsExecutable::PythonMajorMinor => {
                format!(
                    "python{}.{}.exe",
                    interpreter.python_major(),
                    interpreter.python_minor()
                )
            }
            WindowsExecutable::Pythonw => String::from("pythonw.exe"),
            WindowsExecutable::PyPy => String::from("pypy.exe"),
            WindowsExecutable::PyPyMajor => {
                format!("pypy{}.exe", interpreter.python_major())
            }
            WindowsExecutable::PyPyMajorMinor => {
                format!(
                    "pypy{}.{}.exe",
                    interpreter.python_major(),
                    interpreter.python_minor()
                )
            }
            WindowsExecutable::PyPyw => String::from("pypyw.exe"),
            WindowsExecutable::PyPyMajorMinorw => {
                format!(
                    "pypy{}.{}w.exe",
                    interpreter.python_major(),
                    interpreter.python_minor()
                )
            }
            WindowsExecutable::GraalPy => String::from("graalpy.exe"),
        }
    }
}

fn create_venv_trampoline_windows(
    executable: &Path,
    executable_kinds: &[WindowsExecutable],
    interpreter: &Interpreter,
    scripts: &Path,
) -> Result<(), Error> {
    for kind in executable_kinds {
        let target = scripts.join(kind.exe(interpreter));
        create_bin_link(target.as_path(), PathBuf::from(executable)).map_err(Error::Python)?;
    }
    Ok(())
}
