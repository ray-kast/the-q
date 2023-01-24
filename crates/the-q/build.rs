fn main() {
    prost_build::compile_protos(
        &glob::glob("src/proto/*.proto")
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        &["src/proto"],
    )
    .unwrap();
}
