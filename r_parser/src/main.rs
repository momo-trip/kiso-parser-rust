use anyhow::Result;
use ra_ap_paths::AbsPathBuf;
use ra_ap_project_model::CargoConfig;
use ra_ap_load_cargo::{load_workspace_at, LoadCargoConfig, ProcMacroServerChoice};
use camino::Utf8PathBuf;
use std::env;

mod module_deps;
mod function_calls;
mod utils;
mod function_defs;
mod metadata_gen;
mod analyze_deps;  // new module

use module_deps::{analyze_module_dependencies, print_module_dependencies, 
                   build_dependency_graph, print_dependency_graph,
                   export_to_json, save_json_to_file};
use function_calls::{analyze_function_calls, print_call_graph};
use function_defs::{analyze_function_definitions, print_function_definitions,
                     save_definitions_to_metadata};
use metadata_gen::{analyze_rust_metadata, print_rust_metadata, save_metadata_to_file};
use analyze_deps::{analyze_precise_dependencies, print_precise_dependencies,
                    save_precise_dependencies_to_json};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-rust-project> [OPTIONS]", args[0]);
        eprintln!("Example: {} ../programs/sample_project", args[0]);
        eprintln!("\nOptions:");
        eprintln!("  --modules-only       Only analyze module dependencies");
        eprintln!("  --functions-only     Only analyze function call graph");
        eprintln!("  --function-defs-only Only analyze function definitions");
        eprintln!("  --metadata-only      Only generate metadata");
        eprintln!("  --precise-only       Only analyze precise dependencies (rust-analyzer)");
        eprintln!("  --json <file>        Export module dependencies to JSON file");
        eprintln!("  --precise-json <file> Export precise dependencies to JSON file");
        eprintln!("  (no option)          Analyze all");
        std::process::exit(1);
    }
    
    let analyze_modules = !args.contains(&"--functions-only".to_string()) 
                       && !args.contains(&"--function-defs-only".to_string())
                       && !args.contains(&"--metadata-only".to_string())
                       && !args.contains(&"--precise-only".to_string());
    let analyze_functions = !args.contains(&"--modules-only".to_string())
                         && !args.contains(&"--function-defs-only".to_string())
                         && !args.contains(&"--metadata-only".to_string())
                         && !args.contains(&"--precise-only".to_string());
    let analyze_function_defs = !args.contains(&"--modules-only".to_string())
                             && !args.contains(&"--functions-only".to_string())
                             && !args.contains(&"--metadata-only".to_string())
                             && !args.contains(&"--precise-only".to_string());
    let analyze_metadata = !args.contains(&"--modules-only".to_string())
                        && !args.contains(&"--functions-only".to_string())
                        && !args.contains(&"--function-defs-only".to_string())
                        && !args.contains(&"--precise-only".to_string());
    let analyze_precise = args.contains(&"--precise-only".to_string())
                       || (!args.contains(&"--modules-only".to_string())
                       && !args.contains(&"--functions-only".to_string())
                       && !args.contains(&"--function-defs-only".to_string())
                       && !args.contains(&"--metadata-only".to_string()));
    
    // Check JSON output options
    let json_output = args.iter()
        .position(|arg| arg == "--json")
        .and_then(|pos| args.get(pos + 1))
        .map(|s| s.as_str());
    
    let precise_json_output = args.iter()
        .position(|arg| arg == "--precise-json")
        .and_then(|pos| args.get(pos + 1))
        .map(|s| s.as_str());
    
    // Convert relative path to absolute path
    let project_path = Utf8PathBuf::from(&args[1]);
    let project_path = env::current_dir()?.join(&project_path);
    let project_path = project_path.canonicalize()?;
    
    let manifest_path = project_path.join("Cargo.toml");
    
    if !manifest_path.exists() {
        eprintln!("Error: Cargo.toml not found at {:?}", manifest_path);
        std::process::exit(1);
    }
    
    println!("Loading project from: {:?}", manifest_path);
    
    let manifest_utf8 = Utf8PathBuf::from_path_buf(manifest_path.clone())
        .map_err(|_| anyhow::anyhow!("Path contains invalid UTF-8"))?;
    
    let abs_path = AbsPathBuf::assert(manifest_utf8);
    
    // Build the database using load_workspace_at
    let load_config = LoadCargoConfig {
        load_out_dirs_from_check: true,
        with_proc_macro_server: ProcMacroServerChoice::Sysroot,
        prefill_caches: false,
    };
    
    println!("Building database...");
    let (host, vfs, _) = load_workspace_at(
        abs_path.as_ref(),
        &CargoConfig::default(),
        &load_config,
        &|msg| println!("  {}", msg),
    )?;
    
    println!("✓ Database built successfully!\n");
    
    // Analyze module dependencies
    if analyze_modules {
        println!("=== Analyzing Module Dependencies ===\n");
        
        let module_deps = analyze_module_dependencies(&vfs)?;
        print_module_dependencies(&module_deps);
        
        let dep_graph = build_dependency_graph(&module_deps);
        print_dependency_graph(&dep_graph);
        
        // JSON output
        if let Some(output_path) = json_output {
            println!("\n=== Exporting to JSON ===\n");
            let json_data = export_to_json(&module_deps)?;
            save_json_to_file(&json_data, output_path)?;
        }
    }
    
    // Analyze function call graph
    if analyze_functions {
        println!("=== Analyzing Function Call Graph ===\n");
        
        let call_graph = analyze_function_calls(&vfs)?;
        print_call_graph(&call_graph);
    }
    
    // Analyze function definitions
    if analyze_function_defs {
        println!("=== Analyzing Function Definitions ===\n");
        
        let func_defs = analyze_function_definitions(&vfs)?;
        print_function_definitions(&func_defs);
        
        // Save to metadata directory
        save_definitions_to_metadata(&func_defs, "metadata")?;
    }

    // Generate metadata
    if analyze_metadata {
        println!("=== Analyzing Rust Metadata ===\n");
        
        let db = &host;
        let metadata = analyze_rust_metadata(db, &vfs)?;
        print_rust_metadata(&metadata);
        
        // Save to metadata directory
        save_metadata_to_file(&metadata, "metadata")?;
    }
    
    // Precise dependency analysis (using rust-analyzer)
    if analyze_precise {
        println!("=== Analyzing Precise Dependencies (rust-analyzer) ===\n");
        
        let precise_graph = analyze_precise_dependencies(&host, &vfs)?;
        print_precise_dependencies(&precise_graph);
        
        // JSON output
        if let Some(output_path) = precise_json_output {
            save_precise_dependencies_to_json(&precise_graph, output_path)?;
        }
    }
    
    Ok(())
}