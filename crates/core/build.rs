use zksync_error_codegen::arguments::Backend;
use zksync_error_codegen::arguments::GenerationArguments;

fn main() {
    let local_anvil_path = "../../etc/errors/anvil.json".to_owned();
    // If we have modified anvil errors, forces rerunning the build script and
    // regenerating the crate `zksync-error`.
    println!("cargo:rerun-if-changed={local_anvil_path}");

    // This is the root JSON file
    // It will contain the links to other JSON files in the `takeFrom`
    // fields, allowing to fetch errors defined in other projects.
    // One of these links leads to the `anvil.json` file in `anvil-zksync` repository.
    // However, when developing locally, we need to fetch errors from the local
    // copy of `anvil.json` file as well, because we may change it, adding new
    // errors.
    let root_link = "https://raw.githubusercontent.com/matter-labs/zksync-error/refs/heads/main/zksync-root.json".to_owned();

    let arguments = GenerationArguments {
        verbose: true,
        root_link,
        outputs: vec![
            // Overwrite the crate `zksync-error`, add the converter from
            // `anyhow` to a generic error of the appropriate domain.
            (
                "../zksync_error".into(),
                Backend::Rust,
                vec![("use_anyhow".to_owned(), "true".to_owned())],
            ),
        ],
        input_links: vec![local_anvil_path],
    };
    if let Err(e) = zksync_error_codegen::load_and_generate(arguments) {
        println!("cargo::error={e}");
    }
}
