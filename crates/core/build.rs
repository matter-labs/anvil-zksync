use zksync_error_codegen::arguments::Backend;
use zksync_error_codegen::arguments::GenerationArguments;

fn main() {
    println!("cargo:rerun-if-changed=../../etc/resources/anvil.json");
    if let Err(e) = zksync_error_codegen::load_and_generate(GenerationArguments {
        verbose: true,
        root_link: "../../zksync-root.json".into(),
        outputs: vec![
            ("../zksync_error".into(), Backend::Rust, vec![("use_anyhow".to_owned(), "true".to_owned())]),
        ],
        input_links: vec!["../../etc/resources/anvil.json".into()],
    }) {
        eprintln!("{e:?}");
    }
}
