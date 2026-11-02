use anyhow::Result;
use ra_ap_hir::{Crate, Module, ModuleDef, Semantics, Adt};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::Edition;
use ra_ap_vfs::{FileId, Vfs};
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreciseDependency {
    pub from_file: String,
    pub from_module: String,
    pub to_file: String,
    pub to_module: String,
    pub item_type: DependencyType,
    pub item_name: String,
    pub is_public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    Function,
    Struct,
    Enum,
    Trait,
    Type,
    Const,
    Static,
    Module,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreciseDependencyGraph {
    pub dependencies: Vec<PreciseDependency>,
    pub module_files: HashMap<String, String>,
    pub statistics: DependencyStatistics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DependencyStatistics {
    pub total_dependencies: usize,
    pub function_deps: usize,
    pub type_deps: usize,
    pub module_deps: usize,
    pub unique_source_files: usize,
    pub unique_target_files: usize,
}

pub fn analyze_precise_dependencies(
    db: &RootDatabase,
    vfs: &Vfs,
) -> Result<PreciseDependencyGraph> {
    let sema = Semantics::new(db);
    let mut dependencies = Vec::new();
    let mut module_files = HashMap::new();
    
    let crates = Crate::all(db);
    
    for krate in crates {
        let root_module = krate.root_module(db);
        analyze_module_recursive(
            &sema,
            vfs,
            root_module,
            &mut dependencies,
            &mut module_files,
            &mut HashSet::new(),
        )?;
    }
    
    let statistics = calculate_statistics(&dependencies);
    
    Ok(PreciseDependencyGraph {
        dependencies,
        module_files,
        statistics,
    })
}

fn analyze_module_recursive(
    sema: &Semantics<RootDatabase>,
    vfs: &Vfs,
    module: Module,
    dependencies: &mut Vec<PreciseDependency>,
    module_files: &mut HashMap<String, String>,
    visited: &mut HashSet<Module>,
) -> Result<()> {
    if visited.contains(&module) {
        return Ok(());
    }
    visited.insert(module);
    
    let module_name = module.name(sema.db)
        .map(|n| n.display(sema.db, Edition::CURRENT).to_string())
        .unwrap_or_else(|| "crate".to_string());
    
    if let Some(file_id) = get_module_file_id(sema, module) {
        let file_path = get_file_path(vfs, file_id);
        module_files.insert(module_name.clone(), file_path);
    }
    
    let declarations = module.declarations(sema.db);
    for decl in declarations {
        analyze_declaration(sema, vfs, module, decl, dependencies)?;
    }
    
    for child_module in module.children(sema.db) {
        analyze_module_recursive(sema, vfs, child_module, dependencies, module_files, visited)?;
    }
    
    Ok(())
}

fn get_module_file_id(sema: &Semantics<RootDatabase>, module: Module) -> Option<FileId> {
    let source = module.definition_source(sema.db);
    let editioned_file_id = source.file_id.original_file(sema.db);
    // file_id() requires db
    Some(editioned_file_id.file_id(sema.db))
}

fn analyze_declaration(
    sema: &Semantics<RootDatabase>,
    vfs: &Vfs,
    current_module: Module,
    decl: ModuleDef,
    dependencies: &mut Vec<PreciseDependency>,
) -> Result<()> {
    if let ModuleDef::Adt(adt) = decl {
        analyze_adt_dependencies(sema, vfs, current_module, adt, dependencies)?;
    }
    Ok(())
}

fn analyze_adt_dependencies(
    sema: &Semantics<RootDatabase>,
    vfs: &Vfs,
    current_module: Module,
    adt: Adt,
    dependencies: &mut Vec<PreciseDependency>,
) -> Result<()> {
    let from_file = get_module_file_path(sema, vfs, current_module);
    let from_module = get_module_name(sema, current_module);
    
    match adt {
        Adt::Struct(s) => {
            for field in s.fields(sema.db) {
                // Pass an empty iterator to ty_with_args
                let field_ty = field.ty_with_args(sema.db, std::iter::empty());
                analyze_type_dependencies(
                    sema,
                    vfs,
                    &from_file,
                    &from_module,
                    &field_ty,
                    dependencies,
                )?;
            }
        }
        Adt::Enum(e) => {
            for variant in e.variants(sema.db) {
                for field in variant.fields(sema.db) {
                    let field_ty = field.ty_with_args(sema.db, std::iter::empty());
                    analyze_type_dependencies(
                        sema,
                        vfs,
                        &from_file,
                        &from_module,
                        &field_ty,
                        dependencies,
                    )?;
                }
            }
        }
        Adt::Union(_) => {}
    }
    
    Ok(())
}

fn analyze_type_dependencies(
    sema: &Semantics<RootDatabase>,
    vfs: &Vfs,
    from_file: &str,
    from_module: &str,
    ty: &ra_ap_hir::Type,
    dependencies: &mut Vec<PreciseDependency>,
) -> Result<()> {
    if let Some(adt) = ty.as_adt() {
        let module = adt.module(sema.db);
        let to_file = get_module_file_path(sema, vfs, module);
        let to_module = get_module_name(sema, module);
        
        let (item_type, item_name) = match adt {
            Adt::Struct(s) => {
                (DependencyType::Struct, s.name(sema.db).display(sema.db, Edition::CURRENT).to_string())
            }
            Adt::Enum(e) => {
                (DependencyType::Enum, e.name(sema.db).display(sema.db, Edition::CURRENT).to_string())
            }
            Adt::Union(u) => {
                (DependencyType::Type, u.name(sema.db).display(sema.db, Edition::CURRENT).to_string())
            }
        };
        
        dependencies.push(PreciseDependency {
            from_file: from_file.to_string(),
            from_module: from_module.to_string(),
            to_file,
            to_module,
            item_type,
            item_name,
            is_public: true,
        });
    }
    
    Ok(())
}

fn get_module_file_path(
    sema: &Semantics<RootDatabase>,
    vfs: &Vfs,
    module: Module,
) -> String {
    if let Some(file_id) = get_module_file_id(sema, module) {
        get_file_path(vfs, file_id)
    } else {
        "unknown".to_string()
    }
}

fn get_module_name(sema: &Semantics<RootDatabase>, module: Module) -> String {
    module.name(sema.db)
        .map(|n| n.display(sema.db, Edition::CURRENT).to_string())
        .unwrap_or_else(|| "crate".to_string())
}

fn get_file_path(vfs: &Vfs, file_id: FileId) -> String {
    vfs.file_path(file_id).as_path()
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("file_{:?}", file_id))
}

fn calculate_statistics(dependencies: &[PreciseDependency]) -> DependencyStatistics {
    let mut source_files = HashSet::new();
    let mut target_files = HashSet::new();
    let mut function_count = 0;
    let mut type_count = 0;
    let mut module_count = 0;
    
    for dep in dependencies {
        source_files.insert(dep.from_file.clone());
        target_files.insert(dep.to_file.clone());
        
        match dep.item_type {
            DependencyType::Function => function_count += 1,
            DependencyType::Struct | DependencyType::Enum | DependencyType::Type => type_count += 1,
            DependencyType::Module => module_count += 1,
            _ => {}
        }
    }
    
    DependencyStatistics {
        total_dependencies: dependencies.len(),
        function_deps: function_count,
        type_deps: type_count,
        module_deps: module_count,
        unique_source_files: source_files.len(),
        unique_target_files: target_files.len(),
    }
}

pub fn print_precise_dependencies(graph: &PreciseDependencyGraph) {
    println!("=== Precise Dependency Analysis (rust-analyzer) ===\n");
    
    println!("Statistics:");
    println!("  Total dependencies: {}", graph.statistics.total_dependencies);
    println!("  Function dependencies: {}", graph.statistics.function_deps);
    println!("  Type dependencies: {}", graph.statistics.type_deps);
    println!("  Module dependencies: {}", graph.statistics.module_deps);
    println!("  Source files: {}", graph.statistics.unique_source_files);
    println!("  Target files: {}", graph.statistics.unique_target_files);
    println!();
    
    if graph.dependencies.is_empty() {
        println!("No dependencies found.");
        return;
    }
    
    let mut by_module: HashMap<String, Vec<&PreciseDependency>> = HashMap::new();
    for dep in &graph.dependencies {
        by_module.entry(dep.from_module.clone())
            .or_insert_with(Vec::new)
            .push(dep);
    }
    
    for (module, deps) in by_module {
        if !deps.is_empty() {
            println!("Module: {}", module);
            for dep in deps {
                println!("  -> {:?} {} (in {})", dep.item_type, dep.item_name, dep.to_module);
            }
            println!();
        }
    }
}

pub fn save_precise_dependencies_to_json(
    graph: &PreciseDependencyGraph,
    output_path: &str,
) -> Result<()> {
    let json = serde_json::to_string_pretty(graph)?;
    std::fs::write(output_path, json)?;
    println!("✓ Precise dependencies saved to: {}", output_path);
    Ok(())
}