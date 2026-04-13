import subprocess
import json
import os
import re
import sys
import tempfile


from utils_api import (
    # normal
    read_json,
    write_json,
    read_file,
    write_file,
    delete_file,
    copy_file,
    create_directory,
    recreate_directory,
    delete_directory,
    copy_directory,
    create_backup_directory,
    restore_directory,
    tmp_backup_directory,
    run_script,
    #get_compile_commands,
    deduplicate_compile_commands,
    normalize_path,
    append_json,
    find_adjusted_start,
    get_current_code,
    read_specific_lines,

    # translation
    get_path_map,
    obtain_metadata,
    get_name_key,
)

# Call Rust tools to generate metadata
rust_parser_path = "/home/ubuntu/rust_parser/r_parser/target/release/r_parser"  # Adjust the path to suit your environment
user_name = "ubuntu"


"""
def r_analyze_dependencies(target_dir, dep_json_path):
    print("analyze_dependencies")


def r_analyze_function(target_dir, meta_dir):
    print("analyze_dependencies")



def r_extract_metadata(dep_json_path, build_path, target_dir, meta_dir):
    print("Getting metadata")

    # analyze dependencies
    r_analyze_dependencies(target_dir, dep_json_path)

    # analyze function dependencies
    r_analyze_function(target_dir, meta_dir)


def r_analyze_call_relationship(meta_dir, callee_path):
    print("Analyzing call relationship...")

"""
def match_rust_functions_to_c(c_meta_data, rust_metadata, rust_path):
    """
    Match Rust metadata with C metadata,
    and add rust_interface to C metadata
    """
    
    # Get Rust metadata of the target file
    target_file_metadata = None
    for file_path, file_meta in rust_metadata['files'].items():
        if rust_path in file_path or file_path.endswith(os.path.basename(rust_path)):
            target_file_metadata = file_meta
            break
    
    if not target_file_metadata:
        print(f"Warning: No Rust metadata found for {rust_path}")
        return
    
    # Convert Rust function interface information into a dictionary
    rust_interface_map = {}
    for func_info in target_file_metadata.get('function', []):
        rust_interface_map[func_info['name']] = func_info['signature']
    
    # Process each block in C metadata
    for c_item in c_meta_data:
        if c_item['block_type'] == 'function':
            # Find Rust interface whose function name matches
            func_name = c_item.get('name')
            if func_name and func_name in rust_interface_map:
                c_item['rust_interface'] = rust_interface_map[func_name]
        
        elif c_item['block_type'] == 'conventional':
            # Process functions inside conventional
            for func_category in ['function', 'macro_func']:
                if func_category in c_item:
                    for func in c_item[func_category]:
                        func_name = func.get('name')
                        if func_name and func_name in rust_interface_map:
                            c_item['rust_interface'] = rust_interface_map[func_name]
                            break  # Stop at the first match



def parse_files_rust(c_path, rust_path, raw_dir, meta_dir, flag_file):
    """
    Analyze Rust project and generate metadata
    Use rust-analyzer-based r_parser instead of ctags
    """
    
    print(f"************** start parse_files for rust at {rust_path} **************")

    # Identify the directory containing Cargo.toml of the project
    if os.path.isfile(rust_path):
        # If a file is specified, search for Cargo.toml in its directory
        project_dir = os.path.dirname(rust_path)
    else:
        # If a directory is specified
        project_dir = rust_path
    
    # Search parent directories until Cargo.toml is found
    while not os.path.exists(os.path.join(project_dir, "Cargo.toml")):
        parent = os.path.dirname(project_dir)
        if parent == project_dir:  # reached root
            raise FileNotFoundError(f"Cargo.toml not found for {rust_path}")
        project_dir = parent
    
    print(f"Analyzing Rust project at: {project_dir}")
    
    # Execute r_parser
    try:
        result = subprocess.run(
            [rust_parser_path, project_dir, "--metadata-only"],
            capture_output=True,
            text=True,
            check=True
        )
        print(result.stdout)
    except subprocess.CalledProcessError as e:
        print(f"Error running rust parser: {e}")
        print(f"stderr: {e.stderr}")
        raise
    
    # Load generated metadata
    metadata_file = os.path.join("metadata", "rust_metadata.json")
    if not os.path.exists(metadata_file):
        raise FileNotFoundError(f"Metadata file not found: {metadata_file}")
    
    with open(metadata_file, 'r') as f:
        rust_metadata = json.load(f)
    
    # Load C metadata
    c_meta_data, c_meta_path = obtain_metadata(c_path, meta_dir, False, None, "def")
    
    # Find corresponding functions from Rust metadata and add to C side
    match_rust_functions_to_c(c_meta_data, rust_metadata, rust_path)
    
    # Save updated C metadata
    write_json(c_meta_path, c_meta_data)
    
    print("************** end parse_files for rust **************")



def get_rust_interface(rust_source_file):
    tags_file = "rust_tags"
    delete_file(tags_file)

    #command = f"ctags --languages=Rust --rust-kinds=fn -x -f {tags_file} {rust_source_file}"
    command = f"ctags --languages=Rust -x -f {tags_file} {rust_source_file}"
    
    #command = f"ctags --fields=+n -x --c-kinds=+stfp -o {tags_file} {source_file}" # "--c-kinds=+st"
    #command = f"ctags --languages=Rust --rust-kinds=fn -x -o {tags_file} {rust_source_file}"
    #command = f"ctags --languages=Rust --rust-kinds=fn -f - --format=2 --excmd=number --fields=+n -o {tags_file} {rust_source_file}"
    subprocess.run(command, shell=True, capture_output=True, text=True)
    
    ctags_output = read_file(tags_file)
    functions = {}

    if ctags_output is None:
        return functions

    for line in ctags_output.split('\n'): 
        parts = line.split()  # Using split() without arguments to split by any whitespace
        if len(parts) >= 4:  # Ensure it's a function tag with sufficient parts
            func_name = parts[0]
            category = parts[1]
            if category == "function" or "method":
                line_number = parts[2]
                func_signature = ' '.join(parts[4:])  # Reconstruct the function signature
                func_signature = re.sub(r'\s*\{', '', func_signature)
                functions[func_name] = func_signature
                
            # #"""
            # # Extract the full signature
            # match = re.search(r'/\^(.*?)\$/', func_pattern)
            # if match:
            #     full_signature = match.group(1).strip()
            #     print(full_signature)
            # #"""
         
    # Print the functions list for debugging
    #print(json.dumps(functions, indent=2, ensure_ascii=False))
    
    delete_file(tags_file)

    return functions




def rust_find_function_end(file_path, start_line):
    with open(file_path, 'r') as file:
        lines = file.readlines()
    
    brace_count = 0
    function_started = False
    
    for i, line in enumerate(lines[start_line-1:], start_line):
        stripped_line = line.strip()

        # If we haven't started counting braces and we encounter a '{', we start counting
        if '{' in stripped_line:
            function_started = True
        
        if function_started:
            brace_count += stripped_line.count('{')
            brace_count -= stripped_line.count('}')
        
            if brace_count == 0 and function_started:
                return i
        
    return start_line  # If no matching closing brace is found, return the start line



def find_rust_definitions(rust_path) -> list:
    # Adjust Rust element types to match the analysis code
    patterns = {
        'constant': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?const\s+([A-Z_][A-Z0-9_]*)\s*:',
        'function': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[(<]',
        'struct': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?struct\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'enum': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?enum\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'trait': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?trait\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'impl': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?impl\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'type_alias': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?type\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'static': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?static\s+([A-Z_][A-Z0-9_]*)',
        'module': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?mod\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'macro_func': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?macro_rules!\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'union': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?union\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'extern_crate': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?extern\s+crate\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        'use_statement': r'^\s*(?:#\[.*\])?\s*(?:pub\s+)?use\s+([a-zA-Z_][a-zA-Z0-9_]*)',
    }
    
    results = []
    
    func_signatures = get_rust_interface(rust_path)

    with open(rust_path, 'r') as f:
        for line_num, line in enumerate(f, 1):
            if line.strip().startswith('//'):
                continue
            
            for category, pattern in patterns.items():
                match = re.search(pattern, line)
                if match:
                    name = match.group(1)

                    element_name = name
                    start_line = line_num
                    end_line = line_num

                    rust_code = None
                    if category == "function" or category == 'method':
                        if element_name in func_signatures:
                            signature = func_signatures[element_name]
                            rust_code = signature


                    if category == 'macro_func': # <- 'macro_func' 
                        category = 'macro_func'
                        #category, end_line = categorize_macros(rust_path, start_line)
                        end_line = rust_find_function_end(rust_path, start_line)

                    elif category in ['static', 'constant']: # , 'variable'
                        if category == 'variable':
                            end_line = rust_find_variable_end(rust_path, start_line)
                        else:    
                            end_line = rust_find_global_var_end(rust_path, start_line)

                        category = 'global_var'

                    elif category in ['struct', 'enum', 'type_alias', 'union']:
                        end_line = rust_find_struct_end(rust_path, start_line)
                        category = 'data_type' # <- 'data_type' 

                    elif category == 'function': #in ['function', 'method']:
                        category = 'function' # <- 'function' 
                        end_line = rust_find_function_end(rust_path, start_line)

                    elif category == 'method': #in ['function', 'method']:
                        category = 'method'
                        impl_name = obtain_impl_name(tags_file, element_name, start_line)


                    definition = {
                        "name": name,
                        "file_path": rust_path,
                        "start_line": start_line,
                        "end_line": end_line,
                        "category": category,
                        "current_code" : None,
                        "rust_code" : rust_code,
                    }
                    results.append(definition)
    
    return results



def is_blank_line(line):
    return len(line.strip()) == 0



def find_other_intervals(rust_path, meta_dir): # c_path, 
    meta_data, meta_path = obtain_metadata(rust_path, meta_dir, True, None, "def")
    
    # Exclude items where end_line is None
    meta_data = [item for item in meta_data if item['end_line'] is not None]

    # Get ranges of existing blocks
    existing_blocks = [(item['start_line'], item['end_line']) for item in meta_data]
    existing_blocks.sort(key=lambda x: x[0])

    # Read file
    with open(rust_path, 'r') as file:
        lines = file.readlines()

    new_blocks = []
    current_line = 1
    
    # Scan entire file to detect new blocks
    for start, end in existing_blocks:
        if current_line < start:
            block_start = current_line
            while block_start < start and is_blank_line(lines[block_start - 1]):
                block_start += 1
            
            block_end = start - 1
            while block_end > block_start and is_blank_line(lines[block_end - 1]):
                block_end -= 1
            
            current_code = read_specific_lines(rust_path, block_start, block_end)

            if block_start < block_end:
                new_blocks.append({
                    "category": "others",
                    "name": "others",
                    "file_path": rust_path,
                    "start_line": block_start,
                    "end_line": block_end,
                    "current_code" : current_code,
                    "rust_code" : None, 
                })
        current_line = end + 1

    # If there are undetected blocks until the end of the file
    if current_line <= len(lines):
        block_start = current_line
        while block_start <= len(lines) and is_blank_line(lines[block_start - 1]):
            block_start += 1
        
        block_end = len(lines)
        while block_end >= block_start and is_blank_line(lines[block_end - 1]):
            block_end -= 1
        
        current_code = read_specific_lines(rust_path, block_start, block_end)

        if block_start <= block_end:
            new_blocks.append({
                "category": "others",
                "name": "others",
                "file_path": rust_path,
                "start_line": block_start,
                "end_line": block_end,
                "current_code" : current_code,
                "rust_code" : None, 
            })

    # Add new blocks to meta_data
    meta_data.extend(new_blocks)

    # Sort by line number
    meta_data.sort(key=lambda x: x['start_line'])

    write_json(meta_path, meta_data)

    return json.dumps(meta_data, indent=4)



def rust_create_defdata(rust_path, meta_dir): # c_path, 

    print(f"************** start parse_files for rust at {rust_path} **************")

    meta_path_rust = obtain_metadata(rust_path, meta_dir, True, True, "def")

    categories = find_rust_definitions(rust_path)

    # Get directory path of output file
    output_dir = os.path.dirname(meta_path_rust)
    if not os.path.exists(output_dir): # Create directory if it does not exist
        os.makedirs(output_dir)

    # Sort by start_line
    categories = sorted(categories, key=lambda x: x['start_line'])
    write_json(meta_path_rust, categories)

    get_current_code(meta_path_rust) #adjust_for_cfg(meta_path_rust)

    find_other_intervals(rust_path, meta_dir) # c_path, 



def get_rust_dependencies(file_path, lib_path, build_path, cargo_path, map_path):
    """
    Use rust-analyzer-based precise dependency analysis
    """
    target_file = os.path.abspath(file_path)  # absolute path
    dependencies = []
    
    # Check rust_parser path
    rust_parser_dir = f"/home/{user_name}/rust_parser/r_parser"
    if not os.path.exists(rust_parser_dir):
        print(f"Warning: rust_parser not found at {rust_parser_dir}")
        return [lib_path, build_path, cargo_path]
    
    # Find project root directory (where Cargo.toml exists)
    project_root = find_cargo_toml_dir(target_file)
    if not project_root:
        print(f"Warning: Could not find Cargo.toml for {target_file}")
        return [lib_path, build_path, cargo_path]
    
    # Path for temporary JSON file
    temp_json = f"/tmp/rust_deps_{os.getpid()}.json"
    
    try:
        # Execute rust-analyzer-based analysis
        cmd = [
            'cargo', 'run', '--',
            project_root,
            '--precise-only',
            '--precise-json', temp_json
        ]
        
        print(f"Running rust-analyzer based dependency analysis on {project_root}...")
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            check=True,
            cwd=rust_parser_dir,
            timeout=60  # timeout setting
        )
        
        # Load JSON result
        if os.path.exists(temp_json):
            with open(temp_json, 'r') as f:
                dep_data = json.load(f)
            
            # Extract dependencies related to the target file
            target_file_normalized = os.path.normpath(target_file)
            
            for dep in dep_data.get('dependencies', []):
                from_file = os.path.normpath(dep['from_file'])
                to_file = os.path.normpath(dep['to_file'])
                
                # If target file is the dependency source, add the dependency destination
                if from_file == target_file_normalized and to_file not in dependencies:
                    dependencies.append(to_file)
            
            print(f"\nFound {len(dependencies)} dependencies using rust-analyzer")
            
    except subprocess.TimeoutExpired:
        print("Warning: Dependency analysis timed out")
    except subprocess.CalledProcessError as e:
        print(f"Warning: Dependency analysis failed with exit status {e.returncode}")
        print(f"STDERR: {e.stderr}")
    except json.JSONDecodeError as e:
        print(f"Warning: Failed to parse JSON output: {e}")
    except Exception as e:
        print(f"Warning: Unexpected error during dependency analysis: {e}")
    finally:
        # Delete temporary file
        if os.path.exists(temp_json):
            os.remove(temp_json)
    
    # Convert dependencies to relative paths
    if dependencies:
        print("Found dependencies:")
        rel_dependencies = []
        for dep_path in dependencies:
            rel_path = absolute_to_relative(dep_path, f"/home/{user_name}/portable")
            print(f"  {rel_path}")
            rel_dependencies.append(rel_path)
        dependencies = rel_dependencies
    else:
        print("No dependencies found")
    
    include_files = dependencies.copy()
    print(f"include_files from rust_parser: {include_files}")
    
    # Also add C language dependencies (existing logic)
    c_include_files = []
    c_path = get_path_map(map_path, file_path, "c")
    if c_path:
        c_include_files = get_ref_files(c_path, dep_json_path)
        
        for inc_path in c_include_files:
            rust_inc_path = get_path_map(map_path, inc_path, "rust")
            if rust_inc_path and rust_inc_path not in include_files:
                include_files.append(rust_inc_path)
        
        if c_include_files:
            print(f"include_files from C references: {c_include_files}")
    
    print(f"Combined include_files: {include_files}")
    
    # Remove duplicates
    include_files = list(dict.fromkeys(include_files))
    
    # Add required files
    include_files.append(lib_path)
    include_files.append(build_path)
    include_files.append(cargo_path)
    
    return include_files


def find_cargo_toml_dir(file_path):
    """
    Traverse upward from the specified file to find Cargo.toml
    """
    current_dir = os.path.dirname(os.path.abspath(file_path))
    
    while current_dir != '/':
        cargo_toml = os.path.join(current_dir, 'Cargo.toml')
        if os.path.exists(cargo_toml):
            return current_dir
        current_dir = os.path.dirname(current_dir)
    
    return None


# Definitions in the same file are not referenced, so maybe they don't need to be removed
def remove_def_dups(target_path, meta_dir, rust_flag):
    meta_data = obtain_metadata(target_path, meta_dir, rust_flag, False, "def")
    use_meta_data, use_path = obtain_metadata(target_path, meta_dir, rust_flag, None, "use")
    
    if meta_data is None or use_meta_data is None or not meta_data or not use_meta_data:
        return
    
    element_key = 'name' # 'name'
    # Convert metadata to hash map
    meta_map = {}
    for item in meta_data:
        key = (item[element_key], item['start_line'], item.get('end_line'))
        meta_map[key] = item

        if 'components' in item:
            for com in item['components']:
                com_key = (com[element_key], com['start_line'], com.get('end_line'))
                meta_map[com_key] = com

    # List to store filtered use_meta_data
    filtered_use_meta_data = []

    for use_item in use_meta_data:
        element = use_item[element_key]
        should_keep = True

        if 'line_number' in use_item:
            line = use_item['line_number']
            for (e, start, end) in meta_map:
                if e == element and end is not None and start <= line <= end:
                    should_keep = False
                    break
        elif 'start_line' in use_item:
            start = use_item['start_line']
            if any((e, s) == (element, start) for (e, s, _) in meta_map if e == element):
                should_keep = False

        if should_keep:
            filtered_use_meta_data.append(use_item)

    # Write result
    write_json(use_path, filtered_use_meta_data)



def rust_add_macro_usage(meta_dir, rust_path, given_macro_path): # c_path, 
    use_meta_data, use_meta_path = obtain_metadata(rust_path, meta_dir, True, None, "use")
    
    occurrences = []
    macro_data = read_json(given_macro_path)

    if macro_data is None:
        return
    for macro, entries in macro_data.items():
        for entry in entries:
            #print(entry)
            if entry['directive'] == "#define": # Assumes macros exist in C path, but maybe it should fully shift to Rust side?
                #if entry['category'] == 'macro_func':
                #    macro_det = None
                #    occurrences = rust_find_symbol(macro, entry['category'], rust_path, entry['file_path'])
                #
                if entry['category'] == 'macro_var':
                    macro_det = "tmp" #entry['macro_det']
                    occurrences = rust_find_symbol(macro, entry['category'], rust_path, macro_det)
                
                use_meta_data.extend(occurrences)
                
    use_meta_data.extend(occurrences)

    write_json(use_meta_path, use_meta_data)




# This is completely newly replaced
def rust_create_usedata(rust_path, meta_dir, map_path, lib_path, build_path, cargo_path, macro_path, all_macro_path): #, dep_json_path

    print(f"=========== rust_create_usedata start for {rust_path} ===========")
    # Create "use" data here # This corresponds to Rust use
    use_meta_path = obtain_metadata(rust_path, meta_dir, True, True, "use")
    use_meta_data = []
    print(f"use_meta_path is {use_meta_path}")

    include_files = []
    include_files = get_rust_dependencies(rust_path, lib_path, build_path, cargo_path, map_path)

    for include_file in include_files:
        # For all items in include_file
        meta_data, meta_path = obtain_metadata(include_file, meta_dir, True, None, "def")
        if meta_data is None: # 'raw/ed/carg_parser.h'? # It would be strange if this happens
            continue

        for item in meta_data:
            #occurrences = []
            occurrences = rust_find_symbol(item['name'], item['category'], rust_path, include_file) #, c_path, include_file)
            use_meta_data.extend(occurrences)

            if 'components' in item:
                for com in item['components']:
                    occurrences = rust_find_symbol(com['name'], com['category'], rust_path, include_file) #, c_path, include_file)
                    use_meta_data.extend(occurrences)

    #print(f"created usedata for {c_path}, {use_meta_path}")
    write_json(use_meta_path, use_meta_data)

    # Exclude itself
    remove_def_dups(rust_path, meta_dir, True)  # c_path, 

    # Also find usage locations for macro variables  # Is this necessary?
    rust_add_macro_usage(meta_dir, rust_path, macro_path) # c_path, # What's the difference from separate_macro_usage? I'm starting to forget myself
    rust_add_macro_usage(meta_dir, rust_path, all_macro_path)  # c_path, 

    print("=========== rust_create_usedata end ===========")





# When child metadata is updated, merge into parent data
def merge_parent_def(rust_path, meta_dir, map_path): #c_path, meta_dir, dep_json_path):

    print("Printing parent data")
    parent_path = get_path_map(map_path, rust_path, "parent") #parent_path = get_parent_path(c_path, dep_json_path)

    if parent_path is None: # That means it is a parent file, so return
        return
    
    # Update and write "def" file
    child_meta_data, child_meta_path = obtain_metadata(rust_path, meta_dir, True, None, "def") # c_path,
    par_meta_data, par_meta_path = obtain_metadata(parent_path, meta_dir, True, None, "def")

    #print(f"par_meta_data is {par_meta_data} for def data")
    if par_meta_data is None:
        par_meta_data = []

    if child_meta_data is None: # added, but we need?
        child_meta_data = []

    par_meta_data.extend(child_meta_data)
    write_json(par_meta_path, par_meta_data)

    # Update and write "use" file
    child_meta_data, child_meta_path = obtain_metadata(rust_path, meta_dir, True, None, "use") # c_path,
    par_meta_data, par_meta_path = obtain_metadata(parent_path, meta_dir, True, None, "use")

    #print(f"par_meta_data is {par_meta_data} for use data")
    if par_meta_data is None:
        par_meta_data = []
    
    if child_meta_data is None: # added, but we need?
        child_meta_data = []
    par_meta_data.extend(child_meta_data)
    write_json(par_meta_path, par_meta_data)


def run_metadata_gen(project_dir, metadata_file):
    print("Running metadata gen...")
    # metadata-only
    print(f"************** start parse_files for rust at {project_dir} **************")

    # Identify directory containing Cargo.toml of the project
    """
    if os.path.isfile(rust_path):
        # If a file is specified, search for Cargo.toml in its directory
        project_dir = os.path.dirname(rust_path)
    else:
        # If a directory is specified
        project_dir = rust_path
    """
    
    # Search parent directories until Cargo.toml is found
    while not os.path.exists(os.path.join(project_dir, "Cargo.toml")):
        parent = os.path.dirname(project_dir)
        if parent == project_dir:  # reached root
            raise FileNotFoundError(f"Cargo.toml not found for {rust_path}")
        project_dir = parent
    
    print(f"Analyzing Rust project at: {project_dir}")
    
    # Execute r_parser
    try:
        result = subprocess.run(
            [rust_parser_path, project_dir, "--metadata-only"],
            capture_output=True,
            text=True,
            check=True
        )
        print(result.stdout)
    except subprocess.CalledProcessError as e:
        print(f"Error running rust parser: {e}")
        print(f"stderr: {e.stderr}")
        raise
    
    # Load generated metadata
    #metadata_file = os.path.join("metadata", "rust_metadata.json")
    if not os.path.exists(metadata_file):
        raise FileNotFoundError(f"Metadata file not found: {metadata_file}")
    
    with open(metadata_file, 'r') as f:
        rust_metadata = json.load(f)

    print("************** end parse_files for rust **************")

    return rust_metadata


def insert_rust_signature(rust_metadata, meta_dir):
    print("Updating metadata")

    summary = {}
    for file_path, item in rust_metadata.items():
        if file_path not in summary:
            summary[file_path] = []
        summary[file_path].append(item)

    for file_path, data in summary.items():
        meta_path = get_meta_path(file_path)
        write_json(meta_path, data)


def update_c_rust_map(rust_c_map, rust_metadata):

    for item in rust_metadata:
        name_key = get_name_key(item)
    
    # Was the intention to check here?


# Latest: heavily rewritten
# Run rust parser & obtain rust function signature.
def check_rust_block(project_dir, meta_dir, database_dir, rust_c_path, c_rust_path): #, map_path):
    
    rust_c_map = read_json(rust_c_path)
    c_rust_map = read_json(c_rust_path)

    # run metadata_gen.rs
    #output_path = os.path.join("metadata", "rust_metadata.json")
    output_path = os.path.join(database_dir, "rust_metadata.json")
    rust_metadata = run_metadata_gen(project_dir, output_path)

    # Better to insert into each metadata
    insert_rust_signature(rust_metadata, meta_dir)

    update_rust_c_map(rust_c_map, rust_metadata)

    update_c_rust_map(c_rust_map, rust_c_map)

    

def update_c_rust_map(c_rust_path, update_data):
    """
    Integrate update data from sum_json_path into mapping of c_rust_path
    
    Args:
        c_rust_path: Path to C->Rust mapping file
        sum_json_path: Path to JSON file containing update data
    """
    print(f"Update {c_rust_path} ...")
    
    # Load existing c_rust_map (empty dict if not exists)
    if os.path.exists(c_rust_path):
        with open(c_rust_path, 'r', encoding='utf-8') as f:
            c_rust_map = json.load(f)
    else:
        c_rust_map = {}
    
    # # Load update data
    # with open(sum_json_path, 'r', encoding='utf-8') as f:
    #     update_data = json.load(f)
    
    # Integrate update data
    for item in update_data:
        # Build C key
        c_name = item.get('name')
        c_file_path = item.get('file_path')
        c_start_line = item.get('start_line')
        c_end_line = item.get('end_line')
        c_key = f"{c_name}:{c_file_path}:{c_start_line}:{c_end_line}"
        
        # Build Rust key
        rust_code = item.get('rust_code', {})
        rust_name = rust_code.get('name')
        rust_file_path = rust_code.get('file_path')
        rust_start_line = rust_code.get('start_line')
        rust_end_line = rust_code.get('end_line')
        rust_key = f"{rust_name}:{rust_file_path}:{rust_start_line}:{rust_end_line}"
        
        # Add to mapping
        if c_key and rust_key:
            c_rust_map[c_key] = rust_key
            print(f"  Updated: {c_key} -> {rust_key}")
    
    # Save updated mapping
    with open(c_rust_path, 'w', encoding='utf-8') as f:
        json.dump(c_rust_map, f, indent=4, ensure_ascii=False)
    
    print(f"Saved updated c_rust_map to {c_rust_path} ({len(c_rust_map)} entries)")
    return c_rust_map



def update_rust_c_map(rust_c_path, update_data):
    """
    Integrate update data from sum_json_path into the mapping of rust_c_path
    
    Args:
        rust_c_path: Path to Rust->C mapping file
        sum_json_path: Path to JSON file containing update data
    """
    print(f"Update {rust_c_path} ...")
    
    # Load existing rust_c_map (empty dict if not exists)
    if os.path.exists(rust_c_path):
        with open(rust_c_path, 'r', encoding='utf-8') as f:
            rust_c_map = json.load(f)
    else:
        rust_c_map = {}
    
    # # Load update data
    # with open(sum_json_path, 'r', encoding='utf-8') as f:
    #     update_data = json.load(f)
    
    # Integrate update data (reverse key and value)
    for item in update_data:
        # Build C key
        c_name = item.get('name')
        c_file_path = item.get('file_path')
        c_start_line = item.get('start_line')
        c_end_line = item.get('end_line')
        c_key = f"{c_name}:{c_file_path}:{c_start_line}:{c_end_line}"
        
        # Build Rust key
        rust_code = item.get('rust_code', {})
        rust_name = rust_code.get('name')
        rust_file_path = rust_code.get('file_path')
        rust_start_line = rust_code.get('start_line')
        rust_end_line = rust_code.get('end_line')
        rust_key = f"{rust_name}:{rust_file_path}:{rust_start_line}:{rust_end_line}"
        
        # Add to mapping (Rust->C)
        if c_key and rust_key:
            rust_c_map[rust_key] = c_key
            print(f"  Updated: {rust_key} -> {c_key}")
    
    # Save updated mapping
    with open(rust_c_path, 'w', encoding='utf-8') as f:
        json.dump(rust_c_map, f, indent=4, ensure_ascii=False)
    
    print(f"Saved updated rust_c_map to {rust_c_path} ({len(rust_c_map)} entries)")
    return rust_c_map



# test
def merge_c_rust_metadata(c_path, tmp_rust_path): # div_start_line

    tmp_data = read_json(tmp_rust_path)
    
    # Adjust line numbers
    """
    # Temporarily remove
    if div_start_line is not None:
        for tmp_item in tmp_data:
            tmp_line = tmp_item['start_line']
            tmp_item['start_line'] = tmp_line + div_start_line - 1
    """
    c_meta_data, c_meta_path = obtain_metadata(c_path, meta_dir, False, None, "def")
    print(f"Merging to {c_meta_path}")

    for tmp_item in tmp_data:
        if 'name' not in tmp_item and 'c_name' in tmp_item:
            tmp_item['name'] = tmp_item['c_name']
        if 'start_line' not in tmp_item and 'c_start_line' in tmp_item:
            tmp_item['start_line'] = tmp_item['c_start_line']
        if 'end_line' not in tmp_item and 'c_end_line' in tmp_item:
            tmp_item['end_line'] = tmp_item['c_end_line']

    # if c_mata_data is None: # It would be strange if an error occurs here, right?
    for c_item in c_meta_data:
        for tmp_item in tmp_data:
            #print(f"now tmp_item is {tmp_item}")
            #tmp_item['start_line'] = get_start_from_persable_id(tmp_item)

            if tmp_item['name'] == c_item['name'] and tmp_item['start_line'] == c_item['start_line']: # It seems not included because start_line is different
                c_item['rust_code'] = tmp_item['rust_code']

                #print(c_item)
                tmp_item['element_id'] = c_item['element_id'] # Assign element_id correspondence here
                #c_item['rust_start_line'] = tmp_item['rust_start_line']
                if 'rust_function_name' in tmp_item:
                    c_item['rust_function_name'] = tmp_item['rust_function_name']
    
            if 'components' in c_item:    
                for com in c_item['components']:
                    if tmp_item['name'] == com['name'] and tmp_item['start_line'] == com['start_line']:
                        com['rust_code'] = tmp_item['rust_code']
                        tmp_item['element_id'] = c_item['element_id'] # Assign element_id correspondence here
                        #com['rust_start_line'] = tmp_item['rust_start_line']
                        if 'rust_function_name' in tmp_item:
                            com['rust_function_name'] = tmp_item['rust_function_name']

    
    undetected = []
    for tmp_item in tmp_data:
        if 'element_id' not in tmp_item:
            print("Did not find element_id")
            undetected.append(tmp_item)

    write_json("tmp_data.json", undetected)

    write_json(c_meta_path, c_meta_data)

    # This function might be taking a somewhat subtle stance. It might be fine to consider that all files are only modified at child files, and parent files are not initial modification targets
    # This needs to also apply to parent c_path / so that it can also handle the child case
    families = []
    child_paths = None
    # Child files are considered the primary modification targets.
    parent_path = get_path_map(map_path, c_path, "parent")
    #child_paths = get_path_map(map_path, c_path, "child")

    if parent_path is not None:
        families.append(parent_path)
    if child_paths is not None:
        families.extend(child_paths)

    for found_family_path in families:
        c_meta_data, c_meta_path = obtain_metadata(found_family_path, meta_dir, False, None, "def")

        print(f"found_family_path is {found_family_path}; merging to {c_meta_path}")
        for c_item in c_meta_data:
            for tmp_item in tmp_data:
                #print(f"tmp_item is {tmp_item}, c_item is {c_item}") # An error occurred saying element_id does not exist <- why?
                #if tmp_item['name'] == c_item['name'] and tmp_item['element_id'] == c_item['element_id']:
                #if tmp_item['name'] == c_item['name'] and tmp_item['start_line'] == c_item['start_line']:
                if 'element_id' in tmp_item and tmp_item['element_id'] == c_item['element_id']:
                    c_item['rust_code'] = tmp_item['rust_code']

                    if 'rust_function_name' in tmp_item:
                        c_item['rust_function_name'] = tmp_item['rust_function_name']


                if 'components' in c_item:    
                    for com in c_item['components']:
                        #if tmp_item['name'] == com['name'] and tmp_item['element_id'] == com['element_id']:
                        #if tmp_item['name'] == com['name'] and tmp_item['start_line'] == com['start_line']:
                        if 'element_id' in tmp_item and tmp_item['element_id'] == com['element_id']:
                            com['rust_code'] = tmp_item['rust_code']

                            if 'rust_function_name' in tmp_item:
                                com['rust_function_name'] = tmp_item['rust_function_name']

        
        write_json(c_meta_path, c_meta_data)


def update_metadata_with_rust(sum_answer_data, div_meta_dir, database_dir):

    mod_by_file = {}
    for item in sum_answer_data:
        file_path = item['file_path']
        if file_path not in mod_by_file:
            mod_by_file[file_path] = []
        mod_by_file[file_path].append(item)

    for file_path, file_list in mod_by_file.items():
        meta_data, meta_path = obtain_metadata(file_path, div_meta_dir, False, None, "def")
        for item in file_list:
            name = item['name']
            start_line = int(item['start_line'])
            def_key = f"{name}:{file_path}:{start_line}"
            if def_key not in meta_data: # It may sometimes interpret internal IFDEF blocks on its own, so error handling here might be necessary.
                continue
            if 'rust_code' not in meta_data[def_key]:
                meta_data[def_key]['rust_code'] = {
                    'file_path' : None,
                    'start_line' : None,
                    'content' : None
                }
            meta_data[def_key]['rust_code']['content'] = item['rust_code']

        write_json(meta_path, meta_data)



def update_c_rust_metadata(rust_output_dir, meta_dir, database_dir, sum_data, c_rust_path, rust_c_path):
    print("update_c_rust_metadata...")

    # sum_data = read_json(sum_json_path)

    for item in sum_data: #mod_c_path in mod_files:
        # merge the answer_data
        mod_c_path = item['file_path']
        print(f"Merging with {mod_c_path}")
        merge_c_rust_metadata(mod_c_path, answer_path) # , label, div_start_line
        
        # Updating parent_c_path is already done inside merge_c_rust_metadata, so doing it below would cause element_id conflicts
        """
        parent_c_path = get_parent_path(mod_c_path, map_path)
        if parent_c_path is not None:
            merge_c_rust_metadata(parent_c_path, answer_path) # , label, div_start_line
        """

        if DEBUG_LLM:
            tmp_json_data = read_json(answer_path)
            write_json(answer_path, tmp_json_data)
            merge_c_rust_metadata(mod_c_path, answer_path)

    """
    # I'm starting to wonder whether this is really necessary
    update_c_rust_map(c_rust_path, sum_data)

    update_rust_c_map(rust_c_path, sum_data)
    """

    # I think this is necessary for correctness. # If you want to check whether the LLM made mistakes in line numbers, this is probably needed, but currently pending.
    # Run rust parser & obtain rust function signatures.
    # check_rust_block(rust_output_dir, meta_dir, database_dir, rust_c_path, c_rust_path)
    

def run_call_flow(
    workspace: str,
    build_script: str,
    test_script: str,
    output: str | None = None,
):
    """
    Build and then execute tests with perf to obtain function call flow.
    Build with frame pointer enabled and obtain full stack using fp mode.

    Args:
        workspace:    Workspace directory path
        build_script: Path to build script
        test_script:  Path to test execution script
        output:       Output file path (stdout if None)
    """
    workspace = os.path.abspath(workspace)
    build_script = os.path.abspath(build_script)
    test_script = os.path.abspath(test_script)

    # Build with frame pointer enabled
    env = os.environ.copy()
    env["RUSTFLAGS"] = "-C force-frame-pointers=yes"
    env["CFLAGS"] = "-fno-omit-frame-pointer -g"
    env["CXXFLAGS"] = "-fno-omit-frame-pointer -g"

    print(f"[*] Building (frame pointer enabled): {build_script}")
    #r = subprocess.run(["bash", build_script], cwd=workspace, env=env)
    r = subprocess.run(["bash", build_script], cwd=workspace)
    if r.returncode != 0:
        print("[!] Build failed")
        sys.exit(1)

    # perf record (fp mode)
    with tempfile.NamedTemporaryFile(suffix=".data", delete=False) as f:
        perf_data = f.name

    test_dir = os.path.dirname(test_script)

    try:
        print(f"[*] perf record (fp): {test_script}")
        subprocess.run(
            # ["perf", "record", "-g", "--call-graph", "fp",
            #  "-o", perf_data, "--", "bash", test_script],
            # cwd=test_dir,
            ["perf", "record", "-g", "--call-graph", "dwarf,65528",
            "-o", perf_data, "--", "bash", test_script],
            cwd=test_dir,
        )

        # perf script
        print("[*] perf script ...")
        result = subprocess.run(
            ["perf", "script", "-i", perf_data],
            capture_output=True, text=True,
        )

        if output:
            with open(output, "w") as f:
                f.write(result.stdout)
            print(f"[*] Saved: {output}")
        else:
            print(result.stdout)

        return result.stdout

    finally:
        os.unlink(perf_data)


def detect_user_binaries_script(perf_output: str, workspace: str) -> set:
    """
    For perf script: search for ELF binaries inside the workspace,
    and match them with process names in the first line of perf script.
    """
    local_binaries = set()
    for root, dirs, files in os.walk(workspace):
        for f in files:
            path = os.path.join(root, f)
            try:
                with open(path, "rb") as fh:
                    if fh.read(4) == b"\x7fELF":
                        local_binaries.add(f)
            except (PermissionError, IsADirectoryError, OSError):
                continue

    found = set()
    for line in perf_output.splitlines():
        if not line.startswith(" ") and not line.startswith("\t") and line.strip():
            process_name = line.split()[0]
            if process_name in local_binaries:
                found.add(process_name)

    if found:
        print(f"[*] Detected binaries: {sorted(found)}")
    else:
        print("[!] No binaries found in the workspace")
    return found



def filter_perf_script(perf_output: str, workspace: str) -> str:
    """
    Filter the output of perf script.
    1. Extract only processes of ELF binaries inside the workspace
    2. Remove [unknown] lines
    3. Remove addresses and paths, leaving only function names
    """
    binary_names = detect_user_binaries_script(perf_output, workspace)

    blocks = perf_output.strip().split("\n\n")
    filtered_blocks = []

    for block in blocks:
        lines = block.strip().splitlines()
        if not lines:
            continue

        header = lines[0]
        process_name = header.split()[0] if header.split() else ""

        if process_name not in binary_names:
            continue

        cleaned_lines = [header]
        for line in lines[1:]:
            stripped = line.strip()
            if '[unknown]' in stripped:
                continue
            if 'ffffffffffffffff' in stripped:
                continue
            parts = stripped.split(None, 1)
            if len(parts) >= 2:
                func_part = parts[1]
                func_name = func_part.split('+')[0].split('(')[0].strip()
                cleaned_lines.append("    " + func_name)

        if len(cleaned_lines) > 1:
            filtered_blocks.append("\n".join(cleaned_lines))

    return "\n\n".join(filtered_blocks)


def detect_user_binaries(perf_output: str, workspace: str) -> list[str]:
    """
    Search for ELF binaries inside the workspace,
    and return those that appear in the perf output.
    """
    local_binaries = set()
    for root, dirs, files in os.walk(workspace):
        for f in files:
            path = os.path.join(root, f)
            try:
                with open(path, "rb") as fh:
                    if fh.read(4) == b"\x7fELF":
                        local_binaries.add(f)
            except (PermissionError, IsADirectoryError, OSError):
                continue

    # Match with the Command column in perf output
    found = set()
    for line in perf_output.splitlines():
        stripped = line.lstrip()
        if not stripped or not stripped[0].isdigit():
            continue
        parts = stripped.split()
        if len(parts) >= 3:
            command = parts[2]
            if command in local_binaries:
                found.add(command)

    result = sorted(found)
    if result:
        print(f"[*] Detected binaries: {result}")
    else:
        print("[!] No binaries found in the workspace")
    return result


def filter_perf_output(perf_output: str, workspace: str = None, binary_names: list[str] = None, crate_name: str = None) -> str:
    """
    Filter the output of perf report.

    Args:
        perf_output:  Output text of perf report --stdio
        workspace:    Workspace path (used for auto-detection when binary_names is not specified)
        binary_names: List of binary names (auto-detected from workspace if None)
        crate_name:   Filter by crate name (e.g., "trans_rust") → Rust functions only
    """
    if binary_names is None and workspace:
        binary_names = detect_user_binaries(perf_output, workspace)

    if not binary_names and crate_name is None:
        return perf_output

    lines = perf_output.splitlines()
    filtered = []
    in_target_block = False

    for line in lines:
        if line.startswith("#") or line.strip() == "":
            continue

        stripped = line.lstrip()
        if stripped and stripped[0].isdigit():
            if binary_names:
                in_target_block = any(name in line for name in binary_names)
            else:
                in_target_block = True

        if in_target_block:
            if crate_name:
                if crate_name + "::" in line or (stripped and stripped[0].isdigit()):
                    filtered.append(line)
            else:
                filtered.append(line)

    return "\n".join(filtered)


def clean_perf_output(text: str) -> str:
    """
    Remove lines unnecessary for LLM from filtered perf output.
    - Unresolved address lines (e.g., 0xffffffffa...)
    - Kernel/user-space symbol lines with [unknown]
    """
    lines = text.splitlines()
    cleaned = []
    for line in lines:
        stripped = line.strip()
        content = re.sub(r'^[\|\s\-]+', '', stripped)
        if re.match(r'^0x[0-9a-f]+$', content):
            continue
        if '[unknown]' in line:
            continue
        cleaned.append(line)
    return "\n".join(cleaned)


def filter_rust_functions(perf_output: str, crate_name: str = "trans_rust") -> str:
    """
    Extract only lines of Rust functions (specified crate) from perf report output.

    Args:
        perf_output: Output text of perf report --stdio
        crate_name:  Target crate name for filtering (default: trans_rust)
    """
    lines = perf_output.splitlines()
    filtered = []

    for line in lines:
        # Keep header lines and comment lines
        if line.startswith("#") or line.strip() == "":
            continue
        # Extract only lines containing the crate name
        if crate_name + "::" in line:
            filtered.append(line)

    return "\n".join(filtered)



def build_call_tree(filtered_script: str) -> str:
    """
    Aggregate stacks from all samples from the output of filter_perf_script
    and build a single call tree.
    """
    # Extract call chains (bottom → top order) from each sample
    blocks = filtered_script.strip().split("\n\n")
    chains = []

    for block in blocks:
        lines = block.strip().splitlines()
        if len(lines) <= 1:
            continue
        # Get stack frame lines and reverse order (caller → callee)
        frames = []
        for line in lines[1:]:
            func = line.strip()
            if func:
                frames.append(func)
        if frames:
            frames.reverse()
            chains.append(frames)

    # Build the tree
    tree = {}
    for chain in chains:
        node = tree
        for func in chain:
            if func not in node:
                node[func] = {"_count": 0, "_children": {}}
            node[func]["_count"] += 1
            node = node[func]["_children"]

    # Output the tree as text
    total = len(chains)
    lines = []

    def render(node, indent=0):
        # Sort by count (descending)
        items = sorted(node.items(), key=lambda x: x[1]["_count"], reverse=True)
        for func, data in items:
            count = data["_count"]
            pct = count * 100.0 / total if total > 0 else 0
            prefix = "  " * indent
            lines.append(f"{prefix}{func}  ({pct:.1f}%, {count}/{total})")
            render(data["_children"], indent + 1)

    render(tree)
    return "\n".join(lines)


import os

def get_rust_lib_path(work_dir, rust_lib_name):
    """
    Find a Rust library directory by name under work_dir.
    
    Args:
        work_dir: Base directory to search from
        rust_lib_name: Name of the Rust library (e.g. "trans_rust")
    
    Returns:
        Path to the Rust library directory containing Cargo.toml
    
    Raises:
        FileNotFoundError: If no matching Rust library is found
    """
    for root, dirs, files in os.walk(work_dir):
        if "Cargo.toml" in files:
            cargo_path = os.path.join(root, "Cargo.toml")
            with open(cargo_path, "r") as f:
                content = f.read()
            if f'name = "{rust_lib_name}"' in content:
                return root

    raise FileNotFoundError(
        f"Rust library '{rust_lib_name}' not found under '{work_dir}'"
    )


def setup_rust_trace(work_dir: str):
    """
    Build Rust library for tracing
    
    Args:
        rust_lib_path: Path to Rust project containing Cargo.toml
                       e.g.: "/home/ubuntu/c_parser/sample/rust_lib"
    """

    rust_lib_path = get_rust_lib_path(work_dir, "trans_rust")
    print(rust_lib_path)
    cargo_toml_path = os.path.join(rust_lib_path, "Cargo.toml")
    
    if not os.path.exists(cargo_toml_path):
        print(f"[!] Cargo.toml not found: {cargo_toml_path}")
        return False

    # Add trace configuration to Cargo.toml
    with open(cargo_toml_path, "r") as f:
        content = f.read()

    modified = False

    if "[profile.release]" not in content:
        content += "\n[profile.release]\ndebug = true\nopt-level = 0\n"
        modified = True
    else:
        if "debug = true" not in content:
            content = content.replace("[profile.release]", "[profile.release]\ndebug = true")
            modified = True
        if "opt-level = 0" not in content:
            content = content.replace("[profile.release]", "[profile.release]\nopt-level = 0")
            modified = True

    if modified:
        with open(cargo_toml_path, "w") as f:
            f.write(content)
        print(f"[*] Added trace configuration to Cargo.toml: {cargo_toml_path}")



    # Insert nightly + instrument-mcount    
    rust_build_path = os.path.join(rust_lib_path, "rust_build.sh")
    
    if not os.path.exists(rust_build_path):
        print(f"[!] ust_build.sh not found: {rust_build_path}")
        return False

    # Add trace configuration to ust_build.sh
    with open(rust_build_path, "r") as f:
        content = f.read()

    modified = False

    old_txt = "RUSTFLAGS=\"-Awarnings\" cargo build --release --manifest-path=Cargo.toml"
    new_txt = "RUSTFLAGS=\"-Z instrument-mcount\" rustup run nightly cargo build --release --manifest-path=Cargo.toml"
    if old_txt in content:
        content = content.replace(old_txt, new_txt)
        modified = True

    if modified:
        with open(rust_build_path, "w") as f:
            f.write(content)
        print(f"[*] Added trace configuration to rust_build.sh: {rust_build_path}")


    return False


if __name__ == "__main__":

    workspace_dir = "/home/ubuntu/allrust/workspace_0000_zopfli"
    
    run_call_flow(
        workspace=workspace_dir,
        build_script="/home/ubuntu/allrust/workspace_0000_zopfli/run_all.sh",
        test_script="/home/ubuntu/allrust/workspace_0000_zopfli/zopfli/run_test.sh",
        output="trace.txt",
    )

    perf_output = read_file("trace.txt")  # or open("trace.txt").read()

    filtered = filter_perf_script(perf_output, workspace=workspace_dir)
    write_file("filtered_script.txt", filtered)

    call_tree = build_call_tree(filtered) #, min_pct=0.5, max_depth=20)
    write_file("call_tree.txt", call_tree)

    # For large programs (omit below 1%, max depth 10)
    compact_tree = build_call_tree(filtered, min_pct=1.0, max_depth=10)

    sys.exit(0)

    filtered = filter_perf_output(perf_output, workspace=workspace_dir)
    print("\n" + "=" * 60)
    print("# Call flow of user binaries")
    print("=" * 60)
    print(filtered)
    write_file("filtered.txt", filtered)

    cleaned = clean_perf_output(filtered)
    write_file("cleaned.txt", cleaned)
    
    filtered = filter_rust_functions(perf_output, crate_name="trans_rust")
    print(filtered)
    write_file("filtered.txt", filtered)


"""
cargo build
cargo run /home/ubuntu/rust_parser/programs/sample_project


# Analyze both
cargo run -- ../programs/sample_project

# Module dependent only
cargo run -- ../programs/sample_project --modules-only --json output.json

# Function calls only
cargo run -- ../programs/sample_project --functions-only

cargo run -- ../programs/sample_project --funcrions-defs-only
"""


"""
[profile.release]
force-frame-pointers = true

"-fno-omit-frame-pointer -g"
"""