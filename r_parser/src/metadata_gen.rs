use anyhow::Result;
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::{ast, AstNode, SourceFile};
use ra_ap_syntax::ast::{HasModuleItem, HasName, HasVisibility, HasGenericParams};  // Added HasGenericParams
use ra_ap_vfs::Vfs;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct RustMetadata {
    pub files: HashMap<String, FileMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    pub function: Vec<FunctionInfo>,
    pub data_type: Vec<DataTypeInfo>,
    pub global_var: Vec<GlobalVarInfo>,
    pub macro_func: Vec<MacroInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub category: String,
    pub name: String,
    pub usr: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impl_name: Option<String>,
    pub signature: String,
    pub is_static: bool,
    pub arguments: Vec<Argument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argument {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataTypeInfo {
    pub category: String,
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalVarInfo {
    pub category: String,
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MacroInfo {
    pub category: String,
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
}

pub fn analyze_rust_metadata(
    _db: &RootDatabase,
    vfs: &Vfs,
) -> Result<RustMetadata> {
    let mut metadata = RustMetadata {
        files: HashMap::new(),
    };
    
    for (_file_id, path) in vfs.iter() {
        let path_str = format!("{}", path);
        
        // Process only Rust files
        if !path_str.ends_with(".rs") {
            continue;
        }
        
        // Exclude the standard library and dependencies
        if path_str.contains("/.cargo/") || path_str.contains("/.rustup/") {
            continue;
        }
        
        println!("Analyzing: {}", path_str);
        
        if let Some(std_path) = path.as_path() {
            if let Ok(content) = fs::read_to_string(std_path) {
                let parse = SourceFile::parse(&content, ra_ap_syntax::Edition::CURRENT);
                let root = parse.tree();
                
                let file_metadata = analyze_file(&root, &content, &path_str)?;
                metadata.files.insert(path_str, file_metadata);
            }
        }
    }
    
    Ok(metadata)
}

fn analyze_file(root: &SourceFile, content: &str, file_path: &str) -> Result<FileMetadata> {
    let mut file_metadata = FileMetadata {
        function: Vec::new(),
        data_type: Vec::new(),
        global_var: Vec::new(),
        macro_func: Vec::new(),
    };
    
    for item in root.items() {
        match item {
            ast::Item::Fn(func) => {
                if let Some(info) = extract_function_info(&func, None, content, file_path) {
                    file_metadata.function.push(info);
                }
            },
            
            ast::Item::Impl(impl_block) => {
                let impl_name = impl_block.self_ty()
                    .map(|ty| ty.syntax().text().to_string());
                
                if let Some(assoc_list) = impl_block.assoc_item_list() {
                    for assoc_item in assoc_list.assoc_items() {
                        if let ast::AssocItem::Fn(method) = assoc_item {
                            if let Some(info) = extract_function_info(&method, impl_name.clone(), content, file_path) {
                                file_metadata.function.push(info);
                            }
                        }
                    }
                }
            },
            
            ast::Item::Struct(struct_def) => {
                if let Some(name) = struct_def.name() {
                    let range = struct_def.syntax().text_range();
                    let (start_line, end_line) = calculate_line_numbers(content, range.start().into(), range.end().into());
                    file_metadata.data_type.push(DataTypeInfo {
                        category: "data_type".to_string(),
                        name: name.to_string(),
                        start_line,
                        end_line,
                        kind: "struct".to_string(),
                    });
                }
            },
            
            ast::Item::Enum(enum_def) => {
                if let Some(name) = enum_def.name() {
                    let range = enum_def.syntax().text_range();
                    let (start_line, end_line) = calculate_line_numbers(content, range.start().into(), range.end().into());
                    file_metadata.data_type.push(DataTypeInfo {
                        category: "data_type".to_string(),
                        name: name.to_string(),
                        start_line,
                        end_line,
                        kind: "enum".to_string(),
                    });
                }
            },
            
            ast::Item::TypeAlias(type_alias) => {
                if let Some(name) = type_alias.name() {
                    let range = type_alias.syntax().text_range();
                    let (start_line, end_line) = calculate_line_numbers(content, range.start().into(), range.end().into());
                    file_metadata.data_type.push(DataTypeInfo {
                        category: "data_type".to_string(),
                        name: name.to_string(),
                        start_line,
                        end_line,
                        kind: "type".to_string(),
                    });
                }
            },
            
            ast::Item::Static(static_def) => {
                if let Some(name) = static_def.name() {
                    let range = static_def.syntax().text_range();
                    let (start_line, end_line) = calculate_line_numbers(content, range.start().into(), range.end().into());
                    file_metadata.global_var.push(GlobalVarInfo {
                        category: "global_var".to_string(),
                        name: name.to_string(),
                        start_line,
                        end_line,
                        kind: "static".to_string(),
                    });
                }
            },
            
            ast::Item::Const(const_def) => {
                if let Some(name) = const_def.name() {
                    let range = const_def.syntax().text_range();
                    let (start_line, end_line) = calculate_line_numbers(content, range.start().into(), range.end().into());
                    file_metadata.global_var.push(GlobalVarInfo {
                        category: "global_var".to_string(),
                        name: name.to_string(),
                        start_line,
                        end_line,
                        kind: "const".to_string(),
                    });
                }
            },
            
            ast::Item::MacroRules(macro_def) => {
                if let Some(name) = macro_def.name() {
                    let range = macro_def.syntax().text_range();
                    let (start_line, end_line) = calculate_line_numbers(content, range.start().into(), range.end().into());
                    file_metadata.macro_func.push(MacroInfo {
                        category: "macro_func".to_string(),
                        name: name.to_string(),
                        start_line,
                        end_line,
                    });
                }
            },
            
            _ => {}
        }
    }
    
    Ok(file_metadata)
}

fn extract_function_info(
    func: &ast::Fn, 
    impl_name: Option<String>, 
    content: &str,
    file_path: &str
) -> Option<FunctionInfo> {
    let name = func.name()?;
    let range = func.syntax().text_range();
    
    let (start_line, end_line) = calculate_line_numbers(content, range.start().into(), range.end().into());
    
    // Extract arguments
    let arguments = extract_arguments(func);
    
    // Generate signature (with detailed arguments)
    let signature = extract_detailed_signature(func, &arguments);
    
    // Check visibility
    let is_static = func.visibility().is_none();
    
    // Generate USR
    let usr = generate_usr(file_path, &name.to_string(), start_line, impl_name.as_deref());
    
    Some(FunctionInfo {
        category: "function".to_string(),
        name: name.to_string(),
        usr,
        start_line,
        end_line,
        impl_name,
        signature,
        is_static,
        arguments,
    })
}

// Extract argument details
fn extract_arguments(func: &ast::Fn) -> Vec<Argument> {
    let mut args = Vec::new();
    
    if let Some(param_list) = func.param_list() {
        for param in param_list.params() {
            if let Some(pat) = param.pat() {
                let name = match pat {
                    ast::Pat::IdentPat(ident) => {
                        ident.name().map(|n| n.to_string()).unwrap_or_else(|| "_".to_string())
                    }
                    ast::Pat::RefPat(ref_pat) => {
                        if let Some(ast::Pat::IdentPat(ident)) = ref_pat.pat() {
                            ident.name().map(|n| n.to_string()).unwrap_or_else(|| "_".to_string())
                        } else {
                            "_".to_string()
                        }
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

// Generate a detailed signature
fn extract_detailed_signature(func: &ast::Fn, arguments: &[Argument]) -> String {
    let mut sig = String::new();
    
    // Visibility
    if let Some(visibility) = func.visibility() {
        sig.push_str(&format!("{} ", visibility.syntax().text()));
    }
    
    // Async
    if func.async_token().is_some() {
        sig.push_str("async ");
    }
    
    // const
    if func.const_token().is_some() {
        sig.push_str("const ");
    }
    
    // unsafe
    if func.unsafe_token().is_some() {
        sig.push_str("unsafe ");
    }
    
    sig.push_str("fn ");
    
    // Function name
    if let Some(name) = func.name() {
        sig.push_str(&name.to_string());
    }
    
    // Generics
    if let Some(type_params) = func.generic_param_list() {
        sig.push_str(&type_params.syntax().text().to_string());
    }
    
    // Arguments (detailed)
    sig.push('(');
    let args_str = arguments
        .iter()
        .map(|arg| format!("{}: {}", arg.name, arg.arg_type))
        .collect::<Vec<_>>()
        .join(", ");
    sig.push_str(&args_str);
    sig.push(')');
    
    // Return type
    if let Some(ret_type) = func.ret_type() {
        sig.push_str(&format!(" {}", ret_type.syntax().text()));
    }
    
    // Where clause
    if let Some(where_clause) = func.where_clause() {
        sig.push_str(&format!(" {}", where_clause.syntax().text()));
    }
    
    sig
}

// Generate USR (Unified Symbol Resolution)
fn generate_usr(file_path: &str, name: &str, line: u32, impl_name: Option<&str>) -> String {
    if let Some(impl_for) = impl_name {
        format!("rust:{}@{}::{}@{}", file_path, impl_for, name, line)
    } else {
        format!("rust:{}@{}@{}", file_path, name, line)
    }
}

fn calculate_line_numbers(content: &str, start_offset: usize, end_offset: usize) -> (u32, u32) {
    let mut current_offset = 0;
    let mut line_number = 1u32;
    let mut start_line = 1u32;
    let mut end_line = 1u32;
    
    for line in content.lines() {
        let line_len = line.len() + 1;
        
        if current_offset <= start_offset && start_offset < current_offset + line_len {
            start_line = line_number;
        }
        
        if current_offset <= end_offset && end_offset <= current_offset + line_len {
            end_line = line_number;
            break;
        }
        
        current_offset += line_len;
        line_number += 1;
    }
    
    (start_line, end_line)
}

pub fn print_rust_metadata(metadata: &RustMetadata) {
    println!("\n=== Rust Metadata Summary ===\n");
    
    let mut sorted_files: Vec<_> = metadata.files.keys().collect();
    sorted_files.sort();
    
    for file_path in sorted_files {
        let file_meta = &metadata.files[file_path];
        println!("File: {}", file_path);
        println!("  Functions: {}", file_meta.function.len());
        
        // Show function details
        for func in &file_meta.function {
            println!("    - {} (lines {}-{})", func.name, func.start_line, func.end_line);
            println!("      Signature: {}", func.signature);
            println!("      Static: {}", func.is_static);
            if !func.arguments.is_empty() {
                println!("      Arguments:");
                for arg in &func.arguments {
                    println!("        - {}: {}", arg.name, arg.arg_type);
                }
            }
        }
        
        println!("  Data types: {}", file_meta.data_type.len());
        println!("  Global vars: {}", file_meta.global_var.len());
        println!("  Macros: {}", file_meta.macro_func.len());
        println!();
    }
}

pub fn save_metadata_to_file(metadata: &RustMetadata, output_dir: &str) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    
    let output_path = format!("{}/rust_metadata.json", output_dir);
    let json = serde_json::to_string_pretty(metadata)?;
    fs::write(&output_path, json)?;
    
    println!("✓ Metadata saved to: {}", output_path);
    
    Ok(())
}