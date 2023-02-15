fn main() {
    capnpc::CompilerCommand::new()
        .output_path("src/")
        .src_prefix("schemas/")
        .file("schemas/minimal_pkg.capnp")
        .run()
        .expect("schema compiler command");
}
