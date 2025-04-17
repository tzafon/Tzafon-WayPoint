use std::{path::PathBuf, process::Command};

fn compile_protos(folder: PathBuf) -> Result<(), anyhow::Error> {
    if !folder.exists() || !folder.is_dir() {
        return Err(anyhow::anyhow!(
            "Folder {} does not exist",
            folder.display()
        ));
    }
    // run command from parent

    let hash = Command::new("sh")
        .arg("-c")
        .arg("cd ..;./proto-definition/get-hash.sh")
        .output()?
        .stdout;
    let hash = String::from_utf8(hash)?;
    let on_file_version = String::from_utf8(std::fs::read(folder.join("proto_version"))?)?;
    anyhow::ensure!(
        hash == on_file_version,
        // multiline
        "Version mismatch new version:
        --------
        {}
        --------
        Expected:
        --------
        {}
        --------
        Run `./scripts/build.sh` to update the version
        ",
        on_file_version,
        hash
    );

    for file in glob::glob(&format!("{}/*.proto", folder.display()))?.flatten() {
        tonic_build::compile_protos(&file)?;
    }
    #[allow(clippy::unwrap_used)]
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let hashfile = PathBuf::from(out_dir).join("proto_version");
    std::fs::write(hashfile, &hash)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    compile_protos(PathBuf::from("../proto-definition"))?;
    Ok(())
}
