use ra_ap_syntax::{ast, AstNode, SyntaxKind, Edition};
use ra_ap_syntax::ast::{HasName, HasVisibility};
use ra_ap_vfs::Vfs;
use serde::{Serialize, Deserialize};
use anyhow::Result;
use std::collections::HashMap;

// Import utils
use crate::utils::{get_metadata, MetadataResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub category: String,
    pub name: String,
    pub usr: String,
    pub def_file_path: String,
    pub def_start_line: u32,
    pub def_end_line: u32,
    pub signature: String,
    pub is_static: bool,
    pub event: String,
    pub arguments: Vec<Argument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argument {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: String,
}

// Analyze function definitions from the VFS
pub fn analyze_function_definitions(vfs: &Vfs) -> Result<Vec<FunctionDef>> {
    let mut definitions = Vec::new();
    
    // VFS::iter() returns an iterator of (FileId, &VfsPath)
    for (file_id, vfs_path) in vfs.iter() {
        let path_str = vfs_path.as_path()
            .map(|p| p.to_string())
            .unwrap_or_else(|| format!("{:?}", vfs_path));
        
        // Process only .rs files
        if !path_str.ends_with(".rs") {
            continue;
        }
        
        // Read directly from the file system
        let content = match std::fs::read_to_string(&path_str) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("⚠ Failed to read {}: {}", path_str, e);
                continue;
            }
        };
        
        let parse = ra_ap_syntax::SourceFile::parse(&content, Edition::CURRENT);
        let root = parse.syntax_node();
        
        // Find function definitions
        for node in root.descendants() {
            if node.kind() == SyntaxKind::FN {
                if let Some(func) = ast::Fn::cast(node.clone()) {
                    if let Some(def) = extract_function_def(&func, &path_str, &content) {
                        definitions.push(def);
                    }
                }
            }
        }
    }
    
    Ok(definitions)
}

// Extract information from a function definition
fn extract_function_def(func: &ast::Fn, file_path: &str, source: &str) -> Option<FunctionDef> {
    let name = func.name()?.to_string();
    let syntax = func.syntax();
    
    // Compute line numbers
    let start_line = source[..syntax.text_range().start().into()]
        .lines()
        .count() as u32 + 1;
    let end_line = source[..syntax.text_range().end().into()]
        .lines()
        .count() as u32 + 1;
    
    // Check visibility (treated as static unless pub/pub(crate))
    let is_static = func.visibility().is_none();
    
   // Extract arguments
    let arguments = extract_arguments(func);
    
    // Generate signature
    let signature = generate_signature(&name, &arguments, func);
    
    // Generate USR (simplified)
    let usr = format!("rust:{}@{}@{}", file_path, name, start_line);
    
    Some(FunctionDef {
        category: "function".to_string(),
        name,
        usr,
        def_file_path: file_path.to_string(),
        def_start_line: start_line,
        def_end_line: end_line,
        signature,
        is_static,
        event: "definition".to_string(),
        arguments,
    })
}

// Extract function arguments
fn extract_arguments(func: &ast::Fn) -> Vec<Argument> {
    let mut args = Vec::new();
    
    if let Some(param_list) = func.param_list() {
        for param in param_list.params() {
            if let Some(pat) = param.pat() {
                let name = match pat {
                    ast::Pat::IdentPat(ident) => {
                        ident.name().map(|n| n.to_string()).unwrap_or_else(|| "_".to_string())
                    }
                    _ => "_".to_string(),
                };
                
                let arg_type = param.ty()
                    .map(|ty| ty.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                
                args.push(Argument {
                    name,
                    arg_type,
                });
            }
        }
    }
    
    args
}

// Generate function signature
fn generate_signature(name: &str, arguments: &[Argument], func: &ast::Fn) -> String {
    let args_str = arguments
        .iter()
        .map(|arg| format!("{}:{}", arg.name, arg.arg_type))
        .collect::<Vec<_>>()
        .join(",");
    
    let ret_type = func.ret_type()
        .and_then(|rt| rt.ty())
        .map(|ty| format!(" -> {}", ty))
        .unwrap_or_default();
    
    format!("{}({}){}", name, args_str, ret_type)
}

// Print function definitions
pub fn print_function_definitions(definitions: &[FunctionDef]) {
    println!("Found {} function definitions\n", definitions.len());
    
    for def in definitions {
        println!("Function: {}", def.name);
        println!("  File: {}", def.def_file_path);
        println!("  Lines: {}-{}", def.def_start_line, def.def_end_line);
        println!("  Signature: {}", def.signature);
        println!("  Static: {}", def.is_static);
        println!("  Arguments:");
        for arg in &def.arguments {
            println!("    - {}: {}", arg.name, arg.arg_type);
        }
        println!();
    }
}

// Save function definitions to the metadata directory (using utils)
pub fn save_definitions_to_metadata(
    definitions: &[FunctionDef],
    meta_dir: &str,
) -> Result<()> {
    // Group by file
    let mut by_file: HashMap<String, Vec<&FunctionDef>> = HashMap::new();
    
    for def in definitions {
        by_file.entry(def.def_file_path.clone())
            .or_insert_with(Vec::new)
            .push(def);
    }
    
    // Save metadata for each file
    for (file_path, defs) in by_file {
        // Generate path with utils::get_metadata
        match get_metadata(&file_path, meta_dir, Some(true)) {
            Ok(MetadataResult::Path(meta_path)) => {
                // Create directory
                if let Some(parent) = std::path::Path::new(&meta_path).parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                // Save JSON
                let json = serde_json::to_string_pretty(&defs)?;
                std::fs::write(&meta_path, json)?;
                println!("✓ Saved {} definitions to: {}", defs.len(), meta_path);
            }
            Err(e) => eprintln!("⚠ Failed to generate path for {}: {}", file_path, e),
            _ => {}
        }
    }
    
    Ok(())
}

// Load function definitions from metadata
pub fn load_definitions_from_metadata(
    source_file: &str,
    meta_dir: &str,
) -> Result<Vec<FunctionDef>> {
    match get_metadata(source_file, meta_dir, None) {
        Ok(MetadataResult::Data(data)) => {
            let defs: Vec<FunctionDef> = serde_json::from_value(data)?;
            Ok(defs)
        }
        Err(e) => Err(anyhow::anyhow!("Failed to load from {}: {}", source_file, e)),
        _ => unreachable!(),
    }
}

// Save to a JSON file (single-file version)
pub fn save_definitions_to_json(
    definitions: &[FunctionDef],
    output_path: &str,
) -> Result<()> {
    let json = serde_json::to_string_pretty(definitions)?;
    std::fs::write(output_path, json)?;
    println!("✓ Function definitions saved to: {}", output_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_def_serialization() {
        let def = FunctionDef {
            category: "function".to_string(),
            name: "test_func".to_string(),
            usr: "rust:test.rs@test_func@10".to_string(),
            def_file_path: "/path/to/test.rs".to_string(),
            def_start_line: 10,
            def_end_line: 20,
            signature: "test_func(x:i32) -> bool".to_string(),
            is_static: false,
            event: "definition".to_string(),
            arguments: vec![
                Argument {
                    name: "x".to_string(),
                    arg_type: "i32".to_string(),
                }
            ],
        };
        
        let json = serde_json::to_string_pretty(&def).unwrap();
        println!("{}", json);
        assert!(json.contains("test_func"));
        assert!(json.contains("\"type\": \"i32\""));
    }
}