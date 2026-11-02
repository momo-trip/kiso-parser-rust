use anyhow::Result;
use ra_ap_syntax::ast::{self, HasModuleItem, HasName};
use ra_ap_syntax::Edition;
use ra_ap_syntax::{AstNode, SourceFile};
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    pub module_name: String,
    pub file_path: String,
    pub imports: Vec<ImportInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportInfo {
    pub use_path: String,      // Full path of the use statement (e.g., std::collections::HashMap)
    pub imported_items: Vec<String>, // Specific items imported
    pub is_glob: bool,          // Whether this is a glob import such as use foo::*
}

pub fn analyze_module_dependencies(
    vfs: &ra_ap_vfs::Vfs,
) -> Result<HashMap<String, ModuleDependency>> {
    let mut module_deps: HashMap<String, ModuleDependency> = HashMap::new();
    
    for (file_id, path) in vfs.iter() {
        let path_display = format!("{}", path);
        
        // Process only Rust files
        if !path_display.ends_with(".rs") {
            continue;
        }
        
        // Exclude the standard library and dependencies
        if path_display.contains("/.cargo/") || path_display.contains("/.rustup/") {
            continue;
        }
        
        println!("Analyzing module: {}", path_display);
        
        // Get file contents
        let file_path = vfs.file_path(file_id);
        
        if let Some(std_path) = file_path.as_path() {
            if let Ok(contents) = std::fs::read_to_string(std_path) {
                if let Some(dep) = analyze_single_module(&contents, &path_display) {
                    module_deps.insert(dep.module_name.clone(), dep);
                }
            }
        }
    }
    
    Ok(module_deps)
}

fn analyze_single_module(content: &str, file_path: &str) -> Option<ModuleDependency> {
    let parse = SourceFile::parse(content, Edition::CURRENT);
    //use ra_ap_syntax::Edition;
    let parse = SourceFile::parse(content, Edition::CURRENT);
    let root = parse.tree();
    
    // Infer module name (from file path)
    let module_name = extract_module_name(file_path);
    
    let mut imports = Vec::new();
    
    // Find all use statements
    for item in root.items() {
        if let Some(use_item) = ast::Use::cast(item.syntax().clone()) {
            if let Some(use_tree) = use_item.use_tree() {
                extract_imports_from_use_tree(&use_tree, String::new(), &mut imports);
            }
        }
    }
    
    Some(ModuleDependency {
        module_name,
        file_path: file_path.to_string(),
        imports,
    })
}

fn extract_imports_from_use_tree(
    use_tree: &ast::UseTree,
    prefix: String,
    imports: &mut Vec<ImportInfo>,
) {
    if let Some(path) = use_tree.path() {
        let path_str = path.to_string();
        let current_path = if prefix.is_empty() {
            path_str.clone()
        } else {
            format!("{}::{}", prefix, path_str)
        };
        
        // If there is a use_tree_list (e.g., use foo::{bar, baz})
        if let Some(use_tree_list) = use_tree.use_tree_list() {
            for child_tree in use_tree_list.use_trees() {
                extract_imports_from_use_tree(&child_tree, current_path.clone(), imports);
            }
        } else if use_tree.star_token().is_some() {
            // Glob import (use foo::*)
            imports.push(ImportInfo {
                use_path: current_path,
                imported_items: vec![],
                is_glob: true,
            });
        } else {
            // Single import (use foo::bar)
            let imported_item = if let Some(rename) = use_tree.rename() {
                if let Some(alias) = rename.name() {
                    alias.to_string()
                } else {
                    path_str.split("::").last().unwrap_or(&path_str).to_string()
                }
            } else {
                path_str.split("::").last().unwrap_or(&path_str).to_string()
            };
            
            imports.push(ImportInfo {
                use_path: current_path,
                imported_items: vec![imported_item],
                is_glob: false,
            });
        }
    }
}

fn extract_module_name(file_path: &str) -> String {
    // Extract module name from path
    // e.g., /path/to/src/foo/bar.rs -> foo::bar
    
    if let Some(src_idx) = file_path.rfind("/src/") {
        let relative = &file_path[src_idx + 5..];
        let without_ext = relative.trim_end_matches(".rs");
        
        // main.rs and lib.rs are special cases
        if without_ext == "main" || without_ext == "lib" {
            return "crate".to_string();
        }
        
        // mod.rs also uses the parent directory name
        if without_ext.ends_with("/mod") {
            let parent = without_ext.trim_end_matches("/mod");
            return parent.replace('/', "::");
        }
        
        without_ext.replace('/', "::")
    } else {
        // Fallback: file name only
        file_path
            .split('/')
            .last()
            .unwrap_or(file_path)
            .trim_end_matches(".rs")
            .to_string()
    }
}

pub fn print_module_dependencies(module_deps: &HashMap<String, ModuleDependency>) {
    println!("\n=== Module Dependencies ===\n");
    
    let mut sorted_modules: Vec<_> = module_deps.keys().collect();
    sorted_modules.sort();
    
    for module_name in sorted_modules {
        let dep = &module_deps[module_name];
        
        if dep.imports.is_empty() {
            continue;
        }
        
        println!("Module: {}", dep.module_name);
        println!("  File: {}", dep.file_path);
        println!("  Imports:");
        
        for import in &dep.imports {
            if import.is_glob {
                println!("    - {}::* (glob)", import.use_path);
            } else {
                println!("    - {} -> {:?}", import.use_path, import.imported_items);
            }
        }
        println!();
    }
}

pub fn build_dependency_graph(
    module_deps: &HashMap<String, ModuleDependency>,
) -> HashMap<String, HashSet<String>> {
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
    
    for (module_name, dep) in module_deps {
        let mut dependencies = HashSet::new();
        
        for import in &dep.imports {
            // Exclude crate, super, and self to extract actual module dependencies
            let target_module = extract_target_module(&import.use_path);
            if !target_module.is_empty() 
                && target_module != "std" 
                && target_module != "core" 
                && &target_module != module_name {
                dependencies.insert(target_module);
            }
        }
        
        if !dependencies.is_empty() {
            graph.insert(module_name.clone(), dependencies);
        }
    }
    
    graph
}

fn extract_target_module(use_path: &str) -> String {
    // Extract the first module name from the use path
    // e.g., crate::foo::bar -> foo
    //       super::baz -> (empty string; excluded as a relative path)
    
    let parts: Vec<&str> = use_path.split("::").collect();
    
    if parts.is_empty() {
        return String::new();
    }
    
    match parts[0] {
        "crate" if parts.len() > 1 => parts[1].to_string(),
        "super" | "self" => String::new(),
        other => other.to_string(),
    }
}

pub fn print_dependency_graph(graph: &HashMap<String, HashSet<String>>) {
    println!("\n=== Module Dependency Graph ===\n");
    
    let mut sorted_modules: Vec<_> = graph.keys().collect();
    sorted_modules.sort();
    
    for module in sorted_modules {
        let deps = &graph[module];
        if !deps.is_empty() {
            println!("{}:", module);
            let mut sorted_deps: Vec<_> = deps.iter().collect();
            sorted_deps.sort();
            for dep in sorted_deps {
                println!("  -> {}", dep);
            }
            println!();
        }
    }
}

// Format similar to the C dependency JSON
#[derive(Debug, Serialize, Deserialize)]
pub struct RustModuleDependencyJson {
    pub source: String,
    pub include: Vec<String>,           // Modules this module directly depends on
    pub indirect_include: Vec<String>,  // Modules this module indirectly depends on (currently empty)
    pub included: Vec<String>,          // Modules that directly depend on this module
    pub indirect_included: Vec<String>, // Modules that indirectly depend on this module (currently empty)
}

pub fn export_to_json(
    module_deps: &HashMap<String, ModuleDependency>,
) -> Result<Vec<RustModuleDependencyJson>> {
    let dep_graph = build_dependency_graph(module_deps);
    
    // Build reverse dependencies (which modules depend on this one)
    let mut reverse_deps: HashMap<String, HashSet<String>> = HashMap::new();
    for (module, deps) in &dep_graph {
        for dep in deps {
            reverse_deps
                .entry(dep.clone())
                .or_insert_with(HashSet::new)
                .insert(module.clone());
        }
    }
    
    let mut result = Vec::new();
    
    // Output all modules in JSON format
    for (module_name, dep_info) in module_deps {
        let include: Vec<String> = dep_graph
            .get(module_name)
            .map(|deps| deps.iter().cloned().collect())
            .unwrap_or_default();
        
        let included: Vec<String> = reverse_deps
            .get(module_name)
            .map(|deps| deps.iter().cloned().collect())
            .unwrap_or_default();
        
        result.push(RustModuleDependencyJson {
            source: dep_info.file_path.clone(),
            include,
            indirect_include: vec![], // Not implemented yet
            included,
            indirect_included: vec![], // Not implemented yet
        });
    }
    
    // Sort by source path
    result.sort_by(|a, b| a.source.cmp(&b.source));
    
    Ok(result)
}

pub fn save_json_to_file(
    json_data: &[RustModuleDependencyJson],
    output_path: &str,
) -> Result<()> {
    let json_string = serde_json::to_string_pretty(json_data)?;
    std::fs::write(output_path, json_string)?;
    println!("✓ Saved dependency graph to: {}", output_path);
    Ok(())
}