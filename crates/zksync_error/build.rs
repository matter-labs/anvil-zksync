use std::process::ExitCode;

use zksync_error_codegen::arguments::Backend;
use zksync_error_codegen::arguments::GenerationArguments;

const REPOSITORY_ROOT: &str = "../..";
const ROOT_ERROR_DEFINITIONS_FROM_ZKSYNC_ERROR: &str = "zksync-error://zksync-root.json";

fn main() -> ExitCode {
    let local_anvil_path = format!("{REPOSITORY_ROOT}/etc/errors/anvil.json");
    let local_core_path = format!("{REPOSITORY_ROOT}/etc/errors/core.json");
    // If we have modified anvil errors, forces rerunning the build script and
    // regenerating the crate `zksync-error`.
    println!("cargo::rerun-if-changed={local_anvil_path}");
    println!("cargo::rerun-if-changed={local_core_path}");

    // This is the root JSON file
    // It will contain the links to other JSON files in the `takeFrom`
    // fields, allowing to fetch errors defined in other projects.
    // One of these links leads to the `anvil.json` file in `anvil-zksync` repository.
    // However, when developing locally, we need to fetch errors from the local
    // copy of `anvil.json` file as well, because we may change it, adding new
    // errors.
    // Useful link types:
    // - `zksync-error://zksync-root.json` -- file provided by zksync-error crate, matching its version;
    // - `file://<path>` or simply a path` -- local file path;
    // - URL -- allows fetching file from network.
    let root_link = ROOT_ERROR_DEFINITIONS_FROM_ZKSYNC_ERROR;

    let arguments = GenerationArguments {
        verbose: true,
        input_links: vec![root_link.into(),
                          local_core_path],
        override_links: vec![
            ( "https://raw.githubusercontent.com/matter-labs/anvil-zksync/refs/heads/main/etc/errors/anvil.json".to_owned(),
               local_anvil_path)
        ],
        outputs: vec![
            // Overwrite the crate `zksync-error`, add the converter from
            // `anyhow` to a generic error of the appropriate domain.
            zksync_error_codegen::arguments::BackendOutput {
                output_path: format!("{REPOSITORY_ROOT}/crates/zksync_error").into(),
                backend: Backend::Rust,
                arguments: vec![
                    ("use_anyhow".to_owned(), "true".to_owned()),
                    ("generate_cargo_toml".to_owned(), "false".to_owned()),
                ],
            },
            ],
    };
    if let Err(e) = zksync_error_codegen::load_and_generate(arguments) {
        println!("{e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
