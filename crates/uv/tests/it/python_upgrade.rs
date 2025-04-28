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
    uv_snapshot!(context.filters(), context.python_upgrade().arg("3.10"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    "###);
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
    uv_snapshot!(context.filters(), context.venv().arg("-p").arg("3.10.8"), @r"
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
fn python_upgrade_ignored_with_venv_patch_specification() {
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
    Using CPython 3.10.8
    Creating virtual environment at: .venv
    Activate with: source .venv/[BIN]/activate
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

    // Should respect patch version venv was created with
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
