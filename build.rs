use std::fs::File;
use std::io::{self, Read, Write};
use flate2::Compression;
use flate2::write::GzEncoder;
use tar::Builder;
use std::mem::drop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let file = File::create("archive.tar")?;
    // let mut builder = Builder::new(file);
    // builder.append_dir_all("archive", "src/beaver_config")?;

    Ok(())
}