use std::process::Command;

use crate::common::{uv_snapshot, TestContext};
use assert_fs::prelude::PathChild;

use uv_static::EnvVars;

#[test]
fn python_upgrade() {
    let context: TestContext = TestContext::new_with_versions(&[])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs();

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.8 in [TIME]
     + cpython-3.10.8-[PLATFORM]
    ");

    // Don't accept patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10.8"), @r"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    error: `uv python upgrade` only accepts minor versions
    ");

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.17 in [TIME]
     + cpython-3.10.17-[PLATFORM]
    ");

    // Should be a no-op when already upgraded
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    ");
}

#[test]
fn python_upgrade_without_version() {
    let context: TestContext = TestContext::new_with_versions(&[])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs()
        .with_filtered_python_names();

    // Should be a no-op when no versions have been installed
    uv_snapshot!(context.filters(), context.python_upgrade(), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    ");

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8").arg("3.11.8").arg("3.12.8"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed 3 versions in [TIME]
     + cpython-3.10.8-[PLATFORM]
     + cpython-3.11.8-[PLATFORM]
     + cpython-3.12.8-[PLATFORM]
    ");

    // Upgrade patch versions
    uv_snapshot!(context.filters(), context.python_upgrade(), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed 3 versions in [TIME]
     + cpython-3.10.17-[PLATFORM]
     + cpython-3.11.12-[PLATFORM]
     + cpython-3.12.10-[PLATFORM]
    ");

    // Should be a no-op when already upgraded
    uv_snapshot!(context.filters(), context.python_upgrade(), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    All requested versions already on latest patch
    ");
}

#[test]
fn python_upgrade_preview() {
    let context: TestContext = TestContext::new_with_versions(&[])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs();

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8").arg("--preview"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.8 in [TIME]
     + cpython-3.10.8-[PLATFORM] (python3.10)
    ");

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.17 in [TIME]
     + cpython-3.10.17-[PLATFORM]
    ");

    // Should be a no-op when already upgraded
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    ");
}

#[test]
fn python_upgrade_transparent_from_venv() {
    let context: TestContext = TestContext::new_with_versions(&["3.13"])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs();

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.8 in [TIME]
     + cpython-3.10.8-[PLATFORM]
    ");

    // Create a virtual environment
    uv_snapshot!(context.filters(), context.venv().arg("-p").arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.10.8
    Creating virtual environment at: .venv
    Activate with: source .venv/[BIN]/activate
    ");

    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.8

    ----- stderr -----
    "
    );

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.17 in [TIME]
     + cpython-3.10.17-[PLATFORM]
    ");

    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.17

    ----- stderr -----
    "
    );
}

// TODO(john): Add upgrade support for preview bin Python. After upgrade,
// the bin Python version should be the latest patch.
#[test]
fn python_transparent_upgrade_with_preview_installation() {
    let context: TestContext = TestContext::new_with_versions(&["3.13"])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs();

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8").arg("--preview"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.8 in [TIME]
     + cpython-3.10.8-[PLATFORM] (python3.10)
    ");

    let bin_python = context
        .bin_dir
        .child(format!("python3.10{}", std::env::consts::EXE_SUFFIX));

    uv_snapshot!(context.filters(), Command::new(bin_python.as_os_str())
        .arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.8

    ----- stderr -----
    "
    );

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.17 in [TIME]
     + cpython-3.10.17-[PLATFORM]
    ");

    // TODO(john): Upgrades are not currently reflected for --preview bin Python,
    // so we see the outdated patch version.
    uv_snapshot!(context.filters(), Command::new(bin_python.as_os_str())
        .arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.8

    ----- stderr -----
    "
    );
}

#[test]
fn python_upgrade_transparent_from_venv_preview() {
    let context: TestContext = TestContext::new_with_versions(&["3.13"])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs();

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8").arg("--preview"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.8 in [TIME]
     + cpython-3.10.8-[PLATFORM] (python3.10)
    ");

    // Create a virtual environment
    uv_snapshot!(context.filters(), context.venv().arg("-p").arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.10.8
    Creating virtual environment at: .venv
    Activate with: source .venv/[BIN]/activate
    ");

    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.8

    ----- stderr -----
    "
    );

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.17 in [TIME]
     + cpython-3.10.17-[PLATFORM]
    ");

    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.17

    ----- stderr -----
    "
    );
}

#[test]
fn python_upgrade_ignored_with_python_pin() {
    let context: TestContext = TestContext::new_with_versions(&["3.13"])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs();

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.8 in [TIME]
     + cpython-3.10.8-[PLATFORM]
    ");

    // Create a virtual environment
    uv_snapshot!(context.filters(), context.venv(), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.10.8
    Creating virtual environment at: .venv
    Activate with: source .venv/[BIN]/activate
    ");

    // Pin to older patch version
    uv_snapshot!(context.filters(), context.python_pin().arg("3.10.8"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Pinned `.python-version` to `3.10.8`

    ----- stderr -----
    ");

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.17 in [TIME]
     + cpython-3.10.17-[PLATFORM]
    ");

    // Should respect pinned patch version
    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.8

    ----- stderr -----
    "
    );
}

#[test]
fn python_transparent_upgrade_despite_venv_patch_specification() {
    let context: TestContext = TestContext::new_with_versions(&["3.13"])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs();

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.8 in [TIME]
     + cpython-3.10.8-[PLATFORM]
    ");

    // Create a virtual environment with a patch version
    uv_snapshot!(context.filters(), context.venv().arg("-p").arg("3.10.8"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    warning: Virtual environments only record Python minor versions. You could use `uv python pin python3.10.8` to pin the full version
    Using CPython 3.10.8
    Creating virtual environment at: .venv
    Activate with: source .venv/[BIN]/activate
    ");

    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.8

    ----- stderr -----
    "
    );

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.17 in [TIME]
     + cpython-3.10.17-[PLATFORM]
    ");

    // The virtual environment Python version is transparently upgraded.
    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.17

    ----- stderr -----
    "
    );
}

#[test]
fn python_transparent_upgrade_venv_venv() {
    let context: TestContext = TestContext::new_with_versions(&[])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs();

    // Install an earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.10.8"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.8 in [TIME]
     + cpython-3.10.8-[PLATFORM]
    ");

    uv_snapshot!(context.filters(), context.venv().arg("-p").arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.10.8
    Creating virtual environment at: .venv
    Activate with: source .venv/[BIN]/activate
    ");

    // Init a new project from within a virtual environment
    uv_snapshot!(context.filters(), context.init().arg("proj"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Initialized project `proj` at `[TEMP_DIR]/proj`
    ");

    // Create a new virtual environment from within a virtual environment
    uv_snapshot!(context.filters(), context.venv()
        .arg("--directory").arg("proj"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.10.8
    Creating virtual environment at: .venv
    Activate with: source .venv/[BIN]/activate
    ");

    uv_snapshot!(context.filters(), context.run()
        .env(EnvVars::VIRTUAL_ENV, ".venv")
        .arg("--directory").arg("proj")
        .arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.8

    ----- stderr -----
    Resolved 1 package in [TIME]
    Audited in [TIME]
    "
    );

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.10.17 in [TIME]
     + cpython-3.10.17-[PLATFORM]
    ");

    // Should have transparently upgraded in second-order virtual environment
    uv_snapshot!(context.filters(), context.run()
        .env(EnvVars::VIRTUAL_ENV, ".venv")
        .arg("--directory").arg("proj")
        .arg("python").arg("--version")
        .env_remove(EnvVars::VIRTUAL_ENV), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.10.17

    ----- stderr -----
    Resolved 1 package in [TIME]
    Audited in [TIME]
    "
    );
}

#[test]
fn python_upgrade_transparent_from_venv_module() {
    let context = TestContext::new_with_versions(&["3.13"])
        .with_filtered_python_keys()
        .with_filtered_exe_suffix()
        .with_managed_python_dirs()
        .with_filtered_python_names()
        .with_filtered_python_install_bin();

    let bin_dir = context.temp_dir.child("bin");

    // Install earlier patch version
    uv_snapshot!(context.filters(), context.python_install().arg("3.12.9"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.12.9 in [TIME]
     + cpython-3.12.9-[PLATFORM]
    ");

    // Set up a virtual environment using venv module
    uv_snapshot!(context.filters(), context.run().arg("python").arg("-m").arg("venv").arg(context.venv.as_os_str()).arg("--without-pip")
        .env(EnvVars::PATH, bin_dir.as_os_str()), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    ");

    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.12.9

    ----- stderr -----
    "
    );

    // Upgrade patch version
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.12"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Installed Python 3.12.10 in [TIME]
     + cpython-3.12.10-[PLATFORM]
    "
    );

    uv_snapshot!(context.filters(), context.run().arg("python").arg("--version"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Python 3.12.10

    ----- stderr -----
    "
    );
}
