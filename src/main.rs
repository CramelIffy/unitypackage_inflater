use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Represents a single asset extracted from the .unitypackage
#[derive(Debug, Default)]
struct Asset {
    path: Option<String>,
    data: Option<Vec<u8>>,
    meta: Option<String>,
    preview: Option<Vec<u8>>,
}

/// Main entry point. Parses command-line arguments and processes files in parallel.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <file1.unitypackage> [file2.unitypackage] ...", args.get(0).map_or("program", |s| s.as_str()));
        return ExitCode::FAILURE;
    }

    let results: Vec<ExitCode> = args[1..]
        .par_iter()
        .map(|path_str| {
            let path = Path::new(path_str);
            println!(
                "Info: Inflating {}...",
                path.file_stem().unwrap_or_default().to_string_lossy()
            );
            match inflate_package(path) {
                Ok(_) => {
                    println!("Info: Successfully inflated {}.", path.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error: Failed to inflate {} ({})", path.display(), e);
                    ExitCode::FAILURE
                }
            }
        })
        .collect();

    if results.contains(&ExitCode::FAILURE) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Inflates a single .unitypackage file.
fn inflate_package(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.extension() != Some(std::ffi::OsStr::new("unitypackage")) {
        return Err("Invalid file type. Expected .unitypackage".into());
    }

    let assets = collect_assets(path)?;
    write_assets(assets, path)?;

    Ok(())
}

/// Collects all assets from the tar archive inside the .unitypackage.
fn collect_assets(path: &Path) -> Result<HashMap<String, Asset>, Box<dyn Error>> {
    let input_file = File::open(path)?;
    let input = BufReader::new(input_file);
    let decoder = flate2::bufread::GzDecoder::new(input);
    let mut archive = tar::Archive::new(decoder);

    let mut assets: HashMap<String, Asset> = HashMap::new();

    for entry_result in archive.entries()? {
        let mut file = entry_result?;
        let entry_path = file.path()?.into_owned();

        let mut components = entry_path.components();
        let uuid = match components.next() {
            Some(std::path::Component::Normal(s)) => s.to_string_lossy().into_owned(),
            _ => {
                eprintln!("Warning: Skipping entry with invalid UUID path: {}", entry_path.display());
                continue;
            }
        };

        let data_name = match components.next() {
            Some(std::path::Component::Normal(s)) => s.to_string_lossy().into_owned(),
            _ => continue, // This is a root directory entry for the asset, skip.
        };

        let asset = assets.entry(uuid).or_default();

        match data_name.as_str() {
            "asset" => asset.data = Some(read_entry_to_vec(&mut file)?),
            "asset.meta" => asset.meta = Some(read_entry_to_string(&mut file)?),
            "pathname" => asset.path = Some(read_entry_to_string(&mut file)?),
            "preview.png" => asset.preview = Some(read_entry_to_vec(&mut file)?),
            _ => eprintln!("Warning: Unknown data type '{}' found. Skipping.", data_name),
        }
    }

    Ok(assets)
}

/// Writes the collected assets to the filesystem.
fn write_assets(assets: HashMap<String, Asset>, package_path: &Path) -> Result<(), Box<dyn Error>> {
    let output_dir = package_path.with_extension("");

    for (_, asset) in assets {
        let Some(relative_path_str) = asset.path else {
            continue;
        };
        let relative_path = PathBuf::from(relative_path_str);

        if let Some(data) = asset.data {
            let dest_path = output_dir.join(&relative_path);
            write_file_with_parents(&dest_path, &data)?;
        }

        if let Some(meta) = asset.meta {
            let dest_path = output_dir.join(relative_path.with_extension(
                format!(
                    "{}.meta",
                    relative_path.extension().unwrap_or_default().to_string_lossy()
                )
            ));
            write_file_with_parents(&dest_path, meta.as_bytes())?;
        }

        if let Some(preview_data) = asset.preview {
            let stem = relative_path.file_stem().unwrap_or_default().to_string_lossy();
            let new_name = format!("{}_preview_image.png", stem);
            let dest_path = output_dir.join(relative_path.with_file_name(new_name));
            write_file_with_parents(&dest_path, &preview_data)?;
        }
    }

    Ok(())
}

// --- Helper Functions ---

/// Reads a tar entry into a String.
fn read_entry_to_string(entry: &mut tar::Entry<impl Read>) -> Result<String, std::io::Error> {
    let mut s = String::new();
    entry.read_to_string(&mut s)?;
    Ok(s)
}

/// Reads a tar entry into a Vec<u8>.
fn read_entry_to_vec(entry: &mut tar::Entry<impl Read>) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Writes content to a file, creating parent directories if they don't exist.
fn write_file_with_parents(path: &Path, content: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(content)?;
    Ok(())
}