use std::fs::File;
use std::io::{self, Read, Write};
use flate2::Compression;
use flate2::write::GzEncoder;
use tar::Builder;
use std::mem::drop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create("archive.tar.gz")?;
    let mut encoder = GzEncoder::new(file, Compression::default());

    let mut builder = Builder::new(&mut encoder);
    builder.append_dir_all("archive", "src/beaver_config")?;
    drop(builder);

    encoder.finish()?;

    Ok(())
}