use flatbuffers_build::BuilderOptions;

fn main() {
    BuilderOptions::new_with_files([
        "schema/basic_types.fbs",
        "schema/error.fbs",
        "schema/host.fbs",
        "schema/worker.fbs",
        "schema/client.fbs",
        "schema/scheduler_error.fbs",
        "schema/compile_error.fbs",
        "schema/rpc_schema.fbs",
    ])
    .compile()
    .expect("flatbuffer compilation failed")
}
