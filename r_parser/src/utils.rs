use std::path::{Path, PathBuf};
use serde_json::Value;

/// Result of retrieving metadata
pub enum MetadataResult {
    Data(Value),
    Path(String),
    Both(Value, String),
}

/// Get metadata for a Rust or shell script file
/// 
/// # Arguments
/// * `rust_path` - Path to the source file
/// * `meta_dir` - Metadata directory
/// * `path_flag` - None: metadata only, Some(true): path only, Some(false): both

pub fn get_metadata(
    rust_path: &str,
    meta_dir: &str,
    path_flag: Option<bool>,
) -> Result<MetadataResult, Box<dyn std::error::Error>> {
    let path = Path::new(rust_path);
    
    // Convert absolute path to relative path
    let relative_path = if path.is_absolute() {
        let current_dir = std::env::current_dir()?;
        path.strip_prefix(current_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    } else {
        rust_path.to_string()
    };

    // Determine suffix based on file extension
    let suffix = if relative_path.ends_with(".rs") {
        "_rs"
    } else if relative_path.ends_with(".sh") {
        "_sh"
    } else {
        ""
    };

    // Build metadata file path (removing the last extension)
    let base_path = relative_path
        .strip_suffix(".rs")
        .or_else(|| relative_path.strip_suffix(".sh"))
        .unwrap_or(&relative_path);
    
    let meta_path = format!("{}/{}{}.json", meta_dir, base_path, suffix);
    
    println!("Getting metadata from {}", meta_path);

    match path_flag {
        Some(true) => {
            // Return path only
            Ok(MetadataResult::Path(meta_path))
        }
        Some(false) | None => {
            // Load metadata
            let meta_data = read_json(&meta_path)?;
            
            if path_flag.is_none() {
                Ok(MetadataResult::Data(meta_data))
            } else {
                Ok(MetadataResult::Both(meta_data, meta_path))
            }
        }
    }
}

/// Read a JSON file
fn read_json(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let json: Value = serde_json::from_str(&content)?;
    Ok(json)
}

// Usage examples
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_metadata() {
        // Get metadata only
        match get_metadata("src/main.rs", "metadata", None) {
            Ok(MetadataResult::Data(data)) => println!("Data: {:?}", data),
            Err(e) => eprintln!("Error: {}", e),
            _ => unreachable!(),
        }

        // Get path only
        match get_metadata("src/main.rs", "metadata", Some(true)) {
            Ok(MetadataResult::Path(path)) => println!("Path: {}", path),
            Err(e) => eprintln!("Error: {}", e),
            _ => unreachable!(),
        }

        // Get both
        match get_metadata("src/main.rs", "metadata", Some(false)) {
            Ok(MetadataResult::Both(data, path)) => {
                println!("Data: {:?}", data);
                println!("Path: {}", path);
            }
            Err(e) => eprintln!("Error: {}", e),
            _ => unreachable!(),
        }
    }
}