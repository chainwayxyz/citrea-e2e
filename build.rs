fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "clementine")]
    {
        use std::path::PathBuf;

        let proto_file = "proto/clementine.proto";
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

        tonic_build::configure()
            .build_server(false)
            .build_client(true)
            .out_dir(&out_dir)
            .compile_protos(&[proto_file], &["proto"])?;

        println!("cargo:rerun-if-changed={proto_file}");
    }

    Ok(())
}
