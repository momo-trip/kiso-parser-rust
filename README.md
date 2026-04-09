# kiso-parser-rust

API for Rust project analysis, C-to-Rust metadata management, and call flow tracing.

## Dependencies

- Python 3.10+
- `ctags` (Universal Ctags)
- `addr2line`
- `perf` (Linux perf tools)
- [rustfilt](https://github.com/luser/rustfilt) — Rust symbol demangler (`cargo install rustfilt`)

## Main Functions

### Rust Metadata

- `rust_create_defdata(rust_path, meta_dir)` — Parse a Rust source file and generate definition metadata.
- `rust_create_usedata(rust_path, meta_dir, ...)` — Analyze symbol usage and dependency relationships.
- `parse_files_rust(c_path, rust_path, ...)` — Analyze a Rust project and match functions to their C counterparts.
- `check_rust_block(project_dir, meta_dir, ...)` — Run the Rust parser and update function signatures in metadata.

### C ↔ Rust Mapping

- `match_rust_functions_to_c(c_meta, rust_meta, rust_path)` — Match Rust functions to C metadata by name.
- `merge_c_rust_metadata(c_path, tmp_rust_path)` — Merge translated Rust code back into C metadata.
- `update_c_rust_map(c_rust_path, update_data)` — Update the C→Rust mapping file.
- `update_rust_c_map(rust_c_path, update_data)` — Update the Rust→C mapping file.

### Call Flow & Tracing

- `run_call_flow(workspace, build_script, test_script, output)` — Build with frame pointers enabled, run tests under `perf`, and capture call flow.
- `filter_perf_script(perf_output, workspace)` — Filter `perf script` output to user binaries only.
- `build_call_tree(filtered_script)` — Aggregate perf samples into a single call tree.
- `setup_rust_trace(rust_lib_path)` — Configure a Rust project for trace instrumentation (`-Z instrument-mcount`).

### Utilities

- `find_rust_definitions(rust_path)` — Find all definitions (functions, structs, enums, etc.) in a Rust file using regex.
- `get_rust_interface(rust_source_file)` — Extract function signatures using `ctags`.
- `rust_find_function_end(file_path, start_line)` — Find the closing brace of a function by brace counting.

## Quick Start

```python
from rust_api import run_call_flow, filter_perf_script, build_call_tree

# Capture call flow
run_call_flow(
    workspace="/path/to/project",
    build_script="/path/to/build.sh",
    test_script="/path/to/test.sh",
    output="trace.txt",
)

# Filter and build call tree
with open("trace.txt") as f:
    perf_output = f.read()

filtered = filter_perf_script(perf_output, workspace="/path/to/project")
call_tree = build_call_tree(filtered)
print(call_tree)
```

## Build Flags

For C projects, compile with:

```
-finstrument-functions -g -gdwarf-4
```

For Rust projects, `setup_rust_trace()` configures nightly + `-Z instrument-mcount` automatically.
