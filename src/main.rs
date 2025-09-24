use rayon::prelude::*;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    let result = args[1..].par_iter().map(|datas| -> ExitCode {
        let path = std::path::Path::new(datas);

        if path.extension() != Some(std::ffi::OsStr::new("unitypackage")) {
            eprintln!("Error: Invalid File Type.");
            return ExitCode::FAILURE;
        }

        println!(
            "Info: Inflating {}...",
            path.file_stem().unwrap().to_string_lossy()
        );

        let input_file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Error: Cannot Open The File ({})", e);
                return ExitCode::FAILURE;
            }
        };
        let input = std::io::BufReader::new(input_file);

        let decoder = flate2::bufread::GzDecoder::new(input);
        let mut archive = tar::Archive::new(decoder);

        let entries = match archive.entries() {
            Ok(iter) => iter,
            Err(e) => {
                eprintln!("Error: Cannot Open The Archive ({})", e);
                return ExitCode::FAILURE;
            }
        };

        let mut assets = std::collections::HashMap::new();

        for file in entries {
            match file {
                Ok(mut file) => {
                    let path = match file.path() {
                        Ok(path) => path,
                        Err(e) => {
                            eprintln!("Error: Cannot Get The Path. This data is skipped. ({})", e);
                            continue;
                        }
                    };

                    let uuid = match path.components().next() {
                        Some(uuid) => {
                            if let std::path::Component::Normal(uuid) = uuid {
                                match uuid.to_str() {
                                    Some(uuid) => uuid.to_string(),
                                    None => {
                                        eprintln!(
                                            "Error: Cannot Get The UUID. This data is skipped."
                                        );
                                        continue;
                                    }
                                }
                            } else {
                                continue;
                            }
                        }
                        None => {
                            eprintln!("Error: Cannot Get The UUID. This data is skipped.");
                            continue;
                        }
                    };

                    if !assets.contains_key(&uuid) {
                        let asset = Asset {
                            path: None,
                            data: None,
                            meta: None,
                            preview: None,
                        };

                        assets.insert(uuid.clone(), asset);
                    }

                    let data_name = match path.components().nth(1) {
                        Some(data_name) => {
                            if let std::path::Component::Normal(data_name) = data_name {
                                match data_name.to_str() {
                                    Some(uuid) => uuid.to_string(),
                                    None => {
                                        eprintln!(
                                            "Error: Cannot Get The Data Name. This data is skipped."
                                        );
                                        continue;
                                    }
                                }
                            } else {
                                continue;
                            }
                        }
                        None => {
                            // This is root dir. skip
                            continue;
                        }
                    };

                    let asset = assets.get_mut(&uuid).unwrap();

                    match data_name.as_str() {
                        "asset" => {
                            use std::io::Read;
                            let mut buf = std::vec::Vec::new();
                            match file.read_to_end(&mut buf) {
                                Ok(_) => {
                                    buf.shrink_to_fit();
                                    asset.data = Some(buf);
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Error: Cannot Read The Data. This data is skipped. ({})",
                                        e
                                    );
                                    continue;
                                }
                            }
                        }
                        "asset.meta" => {
                            use std::io::Read;
                            let buf = &mut Default::default();
                            match file.read_to_string(buf) {
                                Ok(_) => {
                                    buf.shrink_to_fit();
                                    asset.meta = Some(buf.to_string());
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Error: Cannot Read The Data. This data is skipped. ({})",
                                        e
                                    );
                                    continue;
                                }
                            }
                        }
                        "pathname" => {
                            use std::io::Read;
                            let buf = &mut Default::default();
                            match file.read_to_string(buf) {
                                Ok(_) => {
                                    buf.shrink_to_fit();
                                    asset.path = Some(buf.to_string());
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Error: Cannot Read The Data. This data is skipped. ({})",
                                        e
                                    );
                                    continue;
                                }
                            }
                        }
                        "preview.png" => {
                            use std::io::Read;
                            let mut buf = std::vec::Vec::new();
                            match file.read_to_end(&mut buf) {
                                Ok(_) => {
                                    buf.shrink_to_fit();
                                    asset.preview = Some(buf);
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Error: Cannot Read The Data. This data is skipped. ({})",
                                        e
                                    );
                                    continue;
                                }
                            }
                        }
                        _ => {
                            eprintln!("Error: Unknown Data Name. This data is skipped.");
                            continue;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: Cannot Get The Entry. This data is skipped. ({})", e);
                    continue;
                }
            }
        }

        let new_parent_path = path.with_extension("");
        for asset in assets {
            use std::io::Write;
            if asset.1.path.is_none() {
                continue;
            }
            if asset.1.data.is_some() {
                let next_file_path = new_parent_path.join(asset.1.path.clone().unwrap());
                match std::fs::create_dir_all(next_file_path.parent().unwrap()) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error: Cannot Create The Directory. ({})", e);
                        continue;
                    }
                }
                let file = match std::fs::File::create(next_file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        eprintln!("Error: Cannot Create The File. ({})", e);
                        continue;
                    }
                };
                let mut writer = std::io::BufWriter::new(file);
                match writer.write_all(&asset.1.data.unwrap()) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error: Cannot Write The File. ({})", e);
                        continue;
                    }
                }
            }
            if asset.1.meta.is_some() {
                let next_file_path = new_parent_path.join(asset.1.path.clone().unwrap() + ".meta");
                match std::fs::create_dir_all(next_file_path.parent().unwrap()) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error: Cannot Create The Directory. ({})", e);
                        continue;
                    }
                }
                let file = match std::fs::File::create(next_file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        eprintln!("Error: Cannot Create The File. ({})", e);
                        continue;
                    }
                };
                let mut writer = std::io::BufWriter::new(file);

                match writeln!(&mut writer, "{}", asset.1.meta.unwrap()) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error: Cannot Write The File. ({})", e);
                        continue;
                    }
                }
            }
            if asset.1.preview.is_some() {
                let original_path = asset.1.path.clone().unwrap();
                let base_path = new_parent_path.join(&original_path);
                let stem = base_path.file_stem().unwrap_or_default().to_string_lossy();
                let new_name = format!("{}_preview_image.png", stem);
                let next_file_path = base_path.with_file_name(new_name);
                match std::fs::create_dir_all(next_file_path.parent().unwrap()) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error: Cannot Create The Directory. ({})", e);
                        continue;
                    }
                }
                let file = match std::fs::File::create(next_file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        eprintln!("Error: Cannot Create The File. ({})", e);
                        continue;
                    }
                };
                let mut writer = std::io::BufWriter::new(file);
                match writer.write_all(&asset.1.preview.unwrap()) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error: Cannot Write The File. ({})", e);
                        continue;
                    }
                }
            }
        }
        
        ExitCode::SUCCESS
    }).collect::<Vec<_>>();

    if result.contains(&ExitCode::FAILURE) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[derive(Debug)]
struct Asset {
    path: Option<String>,
    data: Option<Vec<u8>>,
    meta: Option<String>,
    preview: Option<Vec<u8>>,
}
