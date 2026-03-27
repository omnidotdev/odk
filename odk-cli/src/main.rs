//! ODK CLI - Unified Schema Compilation Tool
//!
//! Compile schemas from various sources to TypeSpec targets:
//! - Input: TypeScript (.ts), WASM plugins (.wasm), YAML (.yaml), JSON (.json)
//! - Output: Any TypeSpec emitter

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use odk_schema::CompiledSchema;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod builder;
use odk_cli::config;
mod error_mapper;
mod extism_host;
mod source_map;
mod templates;
mod ts_executor;
mod ts_schema_adapter;
mod typespec_bridge;

use odk_cli::loaders;

#[derive(Parser, Debug)]
#[command(name = "odk", version, about = "ODK - Omni Development Kit CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Compile a schema and emit to targets
    Compile {
        /// Input source: .ts, .yaml, .json, or .wasm file (optional if in config)
        input: Option<Utf8PathBuf>,

        /// Config file path (default: auto-detect odk.config.*)
        #[arg(short, long)]
        config: Option<Utf8PathBuf>,

        /// Output directory for generated files
        #[arg(short, long)]
        out: Option<Utf8PathBuf>,

        /// TypeSpec emitters to use (comma-separated or multiple -e flags)
        /// Examples: @typespec/json-schema, @typespec/openapi3, @typespec/protobuf
        /// Default: @typespec/json-schema, @typespec/openapi3
        #[arg(short, long, value_delimiter = ',')]
        emit: Option<Vec<String>>,

        /// Namespace for generated TypeSpec (overrides schema name)
        #[arg(short, long)]
        namespace: Option<String>,

        /// Keep temporary TypeSpec files (for debugging)
        #[arg(long)]
        keep_temp: bool,

        /// GraphQL operation documents glob pattern (e.g. "src/**/*.graphql")
        #[arg(long)]
        operations: Option<String>,

        /// Introspect a GraphQL endpoint to get the schema
        #[arg(long)]
        introspect: Option<String>,
    },

    /// Initialize a new ODK project
    Init {
        /// Language/format: rust, python, yaml (prompts if not specified)
        #[arg(short, long)]
        lang: Option<String>,

        /// Project name (default: current directory name)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Build a schema plugin to WASM
    Build {
        /// Input: directory with Cargo.toml (default: current dir)
        #[arg(default_value = ".")]
        input: Utf8PathBuf,

        /// Output .wasm file (default: ./target/odk/<name>.wasm)
        #[arg(short, long)]
        out: Option<Utf8PathBuf>,

        /// Build in release mode
        #[arg(long, default_value = "true")]
        release: bool,
    },

    /// Check/validate a schema without emitting
    Check {
        /// Input source: .ts, .yaml, .json, .wasm, or .graphql file
        input: Utf8PathBuf,

        /// GraphQL operation documents glob pattern
        #[arg(long)]
        operations: Option<String>,
    },

    /// Generate client artifacts (hooks, mocks, SDK) from a schema
    Generate {
        /// Input source: .ts, .yaml, .json, .wasm, or .graphql file
        input: Option<Utf8PathBuf>,

        /// Config file path (default: auto-detect odk.config.*)
        #[arg(short, long)]
        config: Option<Utf8PathBuf>,

        /// Output directory for generated files
        #[arg(short, long, default_value = "./generated")]
        out: Utf8PathBuf,

        /// Generation targets (comma-separated)
        /// Available: sdk, react-query, msw, mcp
        /// Default: sdk,react-query,msw
        #[arg(short, long, default_value = "sdk,react-query,msw")]
        targets: String,

        /// GraphQL operation documents glob pattern (e.g. "src/**/*.graphql")
        #[arg(long)]
        operations: Option<String>,

        /// Introspect a GraphQL endpoint to get the schema
        #[arg(long)]
        introspect: Option<String>,
    },

    /// Generate TypeSpec source from a schema (without emitting)
    Typespec {
        /// Input source: .ts, .yaml, .json, or .wasm file
        input: Utf8PathBuf,

        /// Output .tsp file (stdout if not specified)
        #[arg(short, long)]
        out: Option<Utf8PathBuf>,

        /// Namespace for generated TypeSpec
        #[arg(short, long)]
        namespace: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile {
            input,
            config,
            out,
            emit,
            namespace,
            keep_temp,
            operations,
            introspect,
        } => cmd_compile(input, config, out, emit, namespace, keep_temp, operations, introspect),

        Commands::Init { lang, name } => cmd_init(lang, name),

        Commands::Build {
            input,
            out,
            release,
        } => builder::cmd_build(input, out, release),

        Commands::Check { input, operations } => cmd_check(input, operations),

        Commands::Generate {
            input,
            config,
            out,
            targets,
            operations,
            introspect,
        } => cmd_generate(input, config, out, targets, operations, introspect),

        Commands::Typespec {
            input,
            out,
            namespace,
        } => cmd_typespec(input, out, namespace),
    }
}

/// Load a schema from the given input source (auto-detected by extension).
fn load_schema(input: &Utf8PathBuf) -> Result<CompiledSchema> {
    let ext = input.extension().unwrap_or("");

    match ext {
        "ts" => {
            // TypeScript schema via bun + odk-ts
            eprintln!("  Compiling TypeScript schema via bun...");
            let ts_schema = ts_executor::compile_ts_schema(input)?;
            Ok(ts_schema.to_compiled_schema())
        }
        "wasm" => {
            // WASM plugin via Extism
            extism_host::load_schema_from_plugin(input)
        }
        "json" => {
            // JSON schema file - try ODK YAML format first, then raw CompiledSchema
            let path = std::path::Path::new(input.as_str());
            match loaders::load_yaml_schema(path) {
                Ok(schema) => Ok(schema),
                Err(_) => {
                    // Fall back to raw CompiledSchema JSON
                    let content = fs::read_to_string(input)
                        .with_context(|| format!("Failed to read {}", input))?;
                    CompiledSchema::from_json(&content)
                        .with_context(|| format!("Failed to parse JSON schema from {}", input))
                }
            }
        }
        "yaml" | "yml" => {
            // YAML schema file (ODK format)
            let path = std::path::Path::new(input.as_str());
            loaders::load_yaml_schema(path)
                .map_err(|e| anyhow::anyhow!("Failed to load YAML schema: {}", e))
        }
        "graphql" | "gql" => {
            // GraphQL SDL schema file
            let path = std::path::Path::new(input.as_str());
            loaders::load_graphql_schema(path)
                .map_err(|e| anyhow::anyhow!("Failed to load GraphQL schema: {}", e))
        }
        _ => bail!(
            "Unsupported input format: .{}. Use .ts, .wasm, .json, .yaml, or .graphql",
            ext
        ),
    }
}

/// Compile command: load schema and emit to targets.
fn cmd_compile(
    input: Option<Utf8PathBuf>,
    config_path: Option<Utf8PathBuf>,
    out: Option<Utf8PathBuf>,
    emit: Option<Vec<String>>,
    namespace: Option<String>,
    keep_temp: bool,
    operations: Option<String>,
    introspect_url: Option<String>,
) -> Result<()> {
    // Load config file if present
    let cfg = if let Some(ref path) = config_path {
        eprintln!("Loading config from {}...", path);
        Some(config::load_config(path)?)
    } else if let Some(found) = config::find_config_file() {
        eprintln!("Using config file: {}", found);
        Some(config::load_config(&found)?)
    } else {
        None
    };

    // Merge config with CLI args (CLI takes precedence)
    let (schema_path, output_dir, emitters, ns) = if let Some(cfg) = cfg {
        let merged = config::merge_with_cli(cfg, input.as_ref(), out.as_ref(), &emit, &namespace);
        (merged.schema, merged.output, merged.emit, merged.namespace)
    } else {
        (
            input,
            out.unwrap_or_else(|| Utf8PathBuf::from("./generated")),
            emit.unwrap_or_default(),
            namespace,
        )
    };

    // Ensure we have a schema path
    let schema_path = schema_path.ok_or_else(|| {
        anyhow::anyhow!(
            "No schema file specified. Use: odk compile <schema> or set 'schema' in config file."
        )
    })?;

    // If --introspect is set, fetch schema from endpoint
    let mut schema = if let Some(url) = &introspect_url {
        eprintln!("Introspecting {}...", url);
        loaders::introspect_schema(url)
            .map_err(|e| anyhow::anyhow!("Introspection failed: {}", e))?
    } else {
        eprintln!("Loading schema from {}...", schema_path);
        load_schema(&schema_path)?
    };

    // If --operations is set, load operation documents
    if let Some(pattern) = &operations {
        eprintln!("Loading operations from {}...", pattern);
        let (ops, frags) = loaders::load_graphql_operations(pattern)
            .map_err(|e| anyhow::anyhow!("Failed to load operations: {}", e))?;
        for op in ops {
            schema.add_operation(op);
        }
        for frag in frags {
            schema.add_fragment(frag);
        }
        eprintln!(
            "  Loaded {} operations, {} fragments",
            schema.operations.len(),
            schema.fragments.len()
        );
    }

    eprintln!(
        "  Loaded: {} ({} models, {} unions, {} operations)",
        schema.name,
        schema.models.len(),
        schema.unions.len(),
        schema.operations.len()
    );

    compile_with_typespec(&schema, &emitters, &output_dir, ns, keep_temp)
}

/// Compile using TypeSpec emitters.
fn compile_with_typespec(
    schema: &CompiledSchema,
    emit: &[String],
    out: &Utf8PathBuf,
    namespace: Option<String>,
    keep_temp: bool,
) -> Result<()> {
    // Check that tsp is available
    if which::which("tsp").is_err() {
        bail!(
            "TypeSpec compiler (tsp) not found. Install it with:\n\
             npm install -g @typespec/compiler"
        );
    }

    // Default emitters if none specified
    let emitters: Vec<String> = if emit.is_empty() {
        vec![
            "@typespec/json-schema".to_string(),
            "@typespec/openapi3".to_string(),
        ]
    } else {
        emit.to_vec()
    };

    eprintln!("Using TypeSpec emitters: {}", emitters.join(", "));

    // Generate TypeSpec source
    let ns = namespace.as_deref().unwrap_or(&schema.name);
    let tsp_source = typespec_bridge::compiled_to_typespec(schema, ns);

    // Create temp directory for TypeSpec project
    let temp_dir = tempfile::tempdir()?;
    let temp_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let tsp_file = temp_path.join("main.tsp");

    // Write TypeSpec source
    fs::write(&tsp_file, &tsp_source)?;

    // Create package.json with all requested emitters
    let mut deps = serde_json::Map::new();
    deps.insert("@typespec/compiler".into(), serde_json::json!("*"));
    for emitter in &emitters {
        deps.insert(emitter.clone(), serde_json::json!("*"));
    }
    let package_json = serde_json::json!({
        "name": "odk-temp-project",
        "type": "module",
        "dependencies": deps
    });
    fs::write(temp_path.join("package.json"), package_json.to_string())?;

    // Find and symlink node_modules
    let search_start = Utf8PathBuf::try_from(std::env::current_dir()?)?;
    if let Some(node_modules) = find_node_modules(&search_start) {
        let temp_node_modules = temp_path.join("node_modules");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&node_modules, &temp_node_modules)
            .with_context(|| format!("Failed to symlink node_modules"))?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&node_modules, &temp_node_modules)
            .with_context(|| format!("Failed to symlink node_modules"))?;
        eprintln!("  Using node_modules from {}", node_modules.display());
    } else {
        eprintln!("  Installing TypeSpec dependencies...");
        let npm_output = Command::new("npm")
            .args(["install", "--silent"])
            .current_dir(&temp_path)
            .output()
            .context("Failed to run npm install")?;
        if !npm_output.status.success() {
            bail!(
                "npm install failed: {}",
                String::from_utf8_lossy(&npm_output.stderr)
            );
        }
    }

    // Create tspconfig.yaml
    let absolute_out = if out.is_absolute() {
        out.clone()
    } else {
        Utf8PathBuf::try_from(std::env::current_dir()?)?.join(out)
    };

    let tsp_config = format!(
        "output-dir: \"{}\"\nemit:\n{}\noptions:\n{}",
        absolute_out,
        emitters
            .iter()
            .map(|e| format!("  - \"{}\"", e))
            .collect::<Vec<_>>()
            .join("\n"),
        emitters
            .iter()
            .map(|e| emitter_options(e))
            .collect::<Vec<_>>()
            .join("\n")
    );
    fs::write(temp_path.join("tspconfig.yaml"), &tsp_config)?;

    // Ensure output directory exists
    fs::create_dir_all(&absolute_out)?;

    // Run TypeSpec compiler
    eprintln!("Running TypeSpec compiler...");
    let output = Command::new("tsp")
        .args(["compile", tsp_file.as_str()])
        .current_dir(&temp_path)
        .output()
        .context("Failed to run tsp compiler")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("TypeSpec stdout:\n{}", stdout);
        eprintln!("TypeSpec stderr:\n{}", stderr);
        bail!("TypeSpec compilation failed");
    }

    eprintln!("Generated output to {}", absolute_out);

    // Keep temp files if requested
    if keep_temp {
        let kept_path = temp_dir.keep();
        eprintln!("Kept temp files at: {}", kept_path.display());
    }

    Ok(())
}

/// Generate emitter-specific options for tspconfig.yaml.
fn emitter_options(emitter: &str) -> String {
    match emitter {
        "@typespec/json-schema" => format!(
            "  \"{}\":\n    emitter-output-dir: \"{{output-dir}}\"\n    emitAllModels: true\n    emitAllRefs: true",
            emitter
        ),
        "@typespec/openapi3" => format!(
            "  \"{}\":\n    emitter-output-dir: \"{{output-dir}}\"",
            emitter
        ),
        _ => format!(
            "  \"{}\":\n    emitter-output-dir: \"{{output-dir}}\"",
            emitter
        ),
    }
}

/// Find node_modules directory by searching upward.
fn find_node_modules(start: &Utf8PathBuf) -> Option<PathBuf> {
    let mut current = start.as_std_path().to_path_buf();
    loop {
        let node_modules = current.join("node_modules");
        if node_modules.is_dir() && node_modules.join("@typespec").is_dir() {
            return Some(node_modules);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Init command: create a new ODK project.
fn cmd_init(lang: Option<String>, name: Option<String>) -> Result<()> {
    use templates::ProjectLang;

    // Determine project name
    let project_name = match name {
        Some(n) => n,
        None => {
            // Use current directory name
            std::env::current_dir()?
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "my-schema".to_string())
        }
    };

    // Determine language (interactive if not specified)
    let project_lang = match lang {
        Some(l) => ProjectLang::from_str(&l).ok_or_else(|| {
            anyhow::anyhow!("Unknown language: '{}'. Use: rust, python, or yaml", l)
        })?,
        None => prompt_for_language()?,
    };

    eprintln!(
        "Initializing {} project '{}'...",
        project_lang.display_name(),
        project_name
    );

    // Create project based on language
    let is_directory = match project_lang {
        ProjectLang::Rust => {
            // Create directory structure
            let dir = std::path::Path::new(&project_name);
            fs::create_dir_all(dir.join("src"))?;

            // Write Cargo.toml
            let cargo_toml = templates::render(templates::RUST_CARGO_TOML, &project_name);
            fs::write(dir.join("Cargo.toml"), cargo_toml)?;

            // Write src/lib.rs
            let lib_rs = templates::render(templates::RUST_LIB_RS, &project_name);
            fs::write(dir.join("src/lib.rs"), lib_rs)?;

            true
        }
        ProjectLang::Python => {
            // Write schema.py in current directory
            let schema_py = templates::render(templates::PYTHON_SCHEMA, &project_name);
            fs::write("schema.py", schema_py)?;

            false
        }
        ProjectLang::Yaml => {
            // Write schema.yaml in current directory
            let schema_yaml = templates::render(templates::YAML_SCHEMA, &project_name);
            fs::write("schema.yaml", schema_yaml)?;

            false
        }
    };

    // Print success message with next steps
    eprintln!(
        "{}",
        templates::post_init_message(project_lang, &project_name, is_directory)
    );

    Ok(())
}

/// Prompt user to select a language interactively
fn prompt_for_language() -> Result<templates::ProjectLang> {
    use std::io::{self, Write};

    eprintln!("Select a project type:");
    for (i, lang) in templates::ProjectLang::all().iter().enumerate() {
        eprintln!("  {}. {}", i + 1, lang.display_name());
    }
    eprint!("Enter choice [1-3]: ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let choice: usize = input
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid choice. Please enter 1, 2, or 3."))?;

    templates::ProjectLang::all()
        .get(choice.saturating_sub(1))
        .copied()
        .ok_or_else(|| anyhow::anyhow!("Invalid choice. Please enter 1, 2, or 3."))
}

/// Check command: validate a schema without emitting.
fn cmd_check(input: Utf8PathBuf, operations: Option<String>) -> Result<()> {
    eprintln!("Checking {}...", input);

    let mut schema = load_schema(&input)?;

    if let Some(pattern) = &operations {
        let (ops, frags) = loaders::load_graphql_operations(pattern)
            .map_err(|e| anyhow::anyhow!("Failed to load operations: {}", e))?;
        for op in ops {
            schema.add_operation(op);
        }
        for frag in frags {
            schema.add_fragment(frag);
        }
    }

    println!("Schema: {}", schema.name);
    println!("Models: {}", schema.models.len());
    for model in &schema.models {
        println!("  - {} ({} fields)", model.name, model.fields.len());
    }
    println!("Unions: {}", schema.unions.len());
    for union in &schema.unions {
        println!("  - {} ({} variants)", union.name, union.variants.len());
    }
    println!("Operations: {}", schema.operations.len());
    for op in &schema.operations {
        println!("  - {} ({:?})", op.name, op.kind);
    }
    println!("Fragments: {}", schema.fragments.len());
    for frag in &schema.fragments {
        println!("  - {} on {}", frag.name, frag.type_condition);
    }

    if let Some(warnings) = &schema.warnings {
        if !warnings.is_empty() {
            println!("\nWarnings:");
            for w in warnings {
                println!("  - {}", w);
            }
        }
    }

    println!("\nSchema is valid!");
    Ok(())
}

/// Generate command: produce client artifacts from schema + operations.
fn cmd_generate(
    input: Option<Utf8PathBuf>,
    config_path: Option<Utf8PathBuf>,
    out: Utf8PathBuf,
    targets_str: String,
    operations: Option<String>,
    introspect_url: Option<String>,
) -> Result<()> {
    use odk_cli::generate::{self, Target};

    // Load config file if present
    let cfg = if let Some(ref path) = config_path {
        Some(config::load_config(path)?)
    } else if let Some(found) = config::find_config_file() {
        eprintln!("Using config file: {}", found);
        Some(config::load_config(&found)?)
    } else {
        None
    };
    let gen_cfg = cfg.as_ref().and_then(|c| c.generate.as_ref());

    // Merge: CLI flags take precedence over config
    let schema_input = input.or_else(|| cfg.as_ref().and_then(|c| c.schema.clone()));
    let final_targets_str = if targets_str == "sdk,react-query,msw" {
        // Only use config targets if CLI is still at default
        gen_cfg
            .filter(|g| !g.targets.is_empty())
            .map(|g| g.targets.join(","))
            .unwrap_or(targets_str)
    } else {
        targets_str
    };
    let final_operations = operations.or_else(|| {
        gen_cfg.and_then(|g| g.operations.clone())
    });
    let final_out = if out == Utf8PathBuf::from("./generated") {
        gen_cfg
            .and_then(|g| g.output.clone())
            .unwrap_or(out)
    } else {
        out
    };

    // Parse targets
    let targets = Target::parse_list(&final_targets_str)
        .map_err(|e| anyhow::anyhow!(e))?;

    if targets.is_empty() {
        bail!("No targets specified. Available: sdk, react-query, msw, mcp");
    }

    // Load schema
    let mut schema = if let Some(url) = &introspect_url {
        eprintln!("Introspecting {}...", url);
        loaders::introspect_schema(url)
            .map_err(|e| anyhow::anyhow!("Introspection failed: {}", e))?
    } else if let Some(ref input_path) = schema_input {
        eprintln!("Loading schema from {}...", input_path);
        load_schema(input_path)?
    } else {
        bail!("No schema source. Provide an input file or --introspect URL.");
    };

    // Load operations
    if let Some(pattern) = &final_operations {
        eprintln!("Loading operations from {}...", pattern);
        let (ops, frags) = loaders::load_graphql_operations(pattern)
            .map_err(|e| anyhow::anyhow!("Failed to load operations: {}", e))?;
        for op in ops {
            schema.add_operation(op);
        }
        for frag in frags {
            schema.add_fragment(frag);
        }
    }

    if schema.operations.is_empty() {
        bail!(
            "No operations found. Provide operation documents via --operations \
             or include them in the schema file."
        );
    }

    eprintln!(
        "  {} operations, {} fragments",
        schema.operations.len(),
        schema.fragments.len()
    );

    // Generate
    let target_names: Vec<String> = targets.iter().map(|t| t.to_string()).collect();
    eprintln!("Generating targets: {}", target_names.join(", "));

    // Extract MCP config if present
    let mcp_config = cfg
        .as_ref()
        .and_then(|c| c.generate.as_ref())
        .and_then(|g| g.mcp.as_ref());

    let output = generate::generate(&schema, &targets, mcp_config);

    // Write output files
    fs::create_dir_all(&final_out)
        .with_context(|| format!("Failed to create output directory: {}", final_out))?;

    for (filename, content) in &output {
        let path = final_out.join(filename);
        fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path))?;
        eprintln!("  Wrote {}", path);
    }

    eprintln!("Done! {} files generated.", output.len());
    Ok(())
}

/// Typespec command: generate TypeSpec source only.
fn cmd_typespec(
    input: Utf8PathBuf,
    out: Option<Utf8PathBuf>,
    namespace: Option<String>,
) -> Result<()> {
    let schema = load_schema(&input)?;
    let ns = namespace.as_deref().unwrap_or(&schema.name);
    let tsp_source = typespec_bridge::compiled_to_typespec(&schema, ns);

    match out {
        Some(path) => {
            fs::write(&path, &tsp_source)?;
            eprintln!("Wrote TypeSpec to {}", path);
        }
        None => {
            println!("{}", tsp_source);
        }
    }

    Ok(())
}
