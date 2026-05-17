use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() {
    // `crate::version::APP_VERSION` uses this environment variable.
    println!(
        "cargo:rustc-env=APP_VERSION={}",
        cargo_workspace_version().unwrap()
    );
}

/// Finds and parses the root workspace's `Cargo.toml` file to find the value of
/// `workspace.package.version`.
fn cargo_workspace_version() -> Result<String, Box<dyn Error>> {
    let mut workspace_cargo_toml_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_cargo_toml_path.pop();
    workspace_cargo_toml_path.pop();
    workspace_cargo_toml_path.push("Cargo.toml");

    if !workspace_cargo_toml_path.exists() {
        return Err(format!(
            "Root workspace Cargo.toml file isn't at `{}`",
            workspace_cargo_toml_path.display()
        )
        .into());
    }

    println!(
        "cargo:rerun-if-changed={}",
        workspace_cargo_toml_path.display()
    );

    let workspace_cargo_toml =
        fs::read_to_string(&workspace_cargo_toml_path)?.parse::<toml::Table>()?;

    let workspace_version = workspace_cargo_toml
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())
        .ok_or_else(|| {
            format!(
                "No `workspace.package.version` string in `{}`",
                workspace_cargo_toml_path.display()
            )
        })?;

    Ok(workspace_version.into())
}
