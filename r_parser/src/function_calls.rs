use anyhow::Result;
use ra_ap_syntax::ast::{self, HasModuleItem, HasName};
use ra_ap_syntax::Edition;
use ra_ap_syntax::{AstNode, SourceFile};
use std::collections::HashMap;

pub fn analyze_function_calls(
    vfs: &ra_ap_vfs::Vfs,
) -> Result<HashMap<String, Vec<String>>> {
    let mut call_graph: HashMap<String, Vec<String>> = HashMap::new();
    
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
        
        println!("Analyzing calls in: {}", path_display);
        
        // Get file contents
        let file_path = vfs.file_path(file_id);
        
        if let Some(std_path) = file_path.as_path() {
            if let Ok(contents) = std::fs::read_to_string(std_path) {
                analyze_file(&contents, &mut call_graph);
            }
        }
    }
    
    Ok(call_graph)
}

fn analyze_file(content: &str, call_graph: &mut HashMap<String, Vec<String>>) {
    // Parse the source file
    //let parse = SourceFile::parse(content, Edition::CURRENT);
    let parse = SourceFile::parse(content, Edition::CURRENT);
    let root = parse.tree();
    
    // Visit all functions
    for item in root.items() {
        if let Some(func) = ast::Fn::cast(item.syntax().clone()) {
            if let Some(name) = func.name() {
                let func_name = name.to_string();
                let mut callees = Vec::new();
                
                // Get the function body
                if let Some(body) = func.body() {
                    // Find function calls
                    for node in body.syntax().descendants() {
                        // Regular function call (foo())
                        if let Some(call_expr) = ast::CallExpr::cast(node.clone()) {
                            if let Some(expr) = call_expr.expr() {
                                if let Some(path_expr) = ast::PathExpr::cast(expr.syntax().clone()) {
                                    if let Some(path) = path_expr.path() {
                                        // Get the full path (e.g., foo::bar::baz)
                                        let full_path = path.to_string();
                                        callees.push(full_path);
                                    }
                                }
                            }
                        }
                        
                        // Method call (x.foo())
                        if let Some(method_call) = ast::MethodCallExpr::cast(node.clone()) {
                            if let Some(name_ref) = method_call.name_ref() {
                                callees.push(format!(".{}", name_ref.to_string()));
                            }
                        }
                    }
                }
                
                if !callees.is_empty() {
                    call_graph.insert(func_name, callees);
                }
            }
        }
    }
}

pub fn print_call_graph(call_graph: &HashMap<String, Vec<String>>) {
    println!("\n=== Function Call Graph ===\n");
    
    let mut sorted_keys: Vec<_> = call_graph.keys().collect();
    sorted_keys.sort();
    
    for caller in sorted_keys {
        let callees = &call_graph[caller];
        if !callees.is_empty() {
            println!("{}:", caller);
            for callee in callees {
                println!("  -> {}", callee);
            }
            println!();
        }
    }
}