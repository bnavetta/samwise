fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("src/samwise.proto")?;

    tonic_build::configure()
        .format(false)
        .compile(&["src/samwise.proto"], &["src"])?;

    Ok(())
}
