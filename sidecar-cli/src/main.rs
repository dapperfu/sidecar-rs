use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use indexmap::IndexMap;
use sidecar::{
    sidecar_path_for_media, SidecarDocument, Value, MEDIA_BASENAME_KEY, SIDECAR_EXTENSION,
};

#[derive(Parser)]
#[command(name = "sidecar", about = "Create and inspect CBOR sidecar files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create an empty sidecar file for a media file
    Create {
        /// Path to the media file
        media: PathBuf,
        /// Output sidecar path (default: same basename with .scar extension)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Set a typed field in a sidecar file
    Set {
        /// Path to the media file or its sidecar (.scar resolved automatically)
        file: PathBuf,
        /// Set an f64 field (KEY=VALUE)
        #[arg(long)]
        f64: Option<String>,
        /// Set an f32 field (KEY=VALUE)
        #[arg(long)]
        f32: Option<String>,
        /// Set a u64 field (KEY=VALUE)
        #[arg(long)]
        u64: Option<String>,
        /// Set an i64 field (KEY=VALUE)
        #[arg(long)]
        i64: Option<String>,
        /// Set a bool field (KEY=true|false)
        #[arg(long)]
        bool: Option<String>,
        /// Set a string field (KEY=VALUE)
        #[arg(long)]
        str: Option<String>,
        /// Set a bytes field from file (KEY=@path)
        #[arg(long)]
        bytes: Option<String>,
        /// Set a value from JSON (KEY=<json>, supports nested maps and arrays)
        #[arg(long)]
        json: Option<String>,
    },
    /// Get a field value from a sidecar file
    Get {
        /// Path to the media file or its sidecar (.scar resolved automatically)
        file: PathBuf,
        key: String,
    },
    /// List all keys in a sidecar file
    List {
        /// Path to the media file or its sidecar (.scar resolved automatically)
        file: PathBuf,
    },
    /// Inspect document summary and catalog
    Inspect {
        /// Path to the media file or its sidecar (.scar resolved automatically)
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Create { media, output } => cmd_create(&media, output.as_deref()),
        Commands::Set {
            file,
            f64,
            f32,
            u64,
            i64,
            bool: bool_arg,
            str,
            bytes,
            json,
        } => cmd_set(SetArgs {
            file,
            f64,
            f32,
            u64,
            i64,
            bool_arg,
            str,
            bytes,
            json,
        }),
        Commands::Get { file, key } => cmd_get(&file, &key),
        Commands::List { file } => cmd_list(&file),
        Commands::Inspect { file } => cmd_inspect(&file),
    }
}

fn cmd_create(media: &Path, output: Option<&Path>) -> Result<()> {
    let out_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| sidecar_path_for_media(media));

    let mut doc = SidecarDocument::new();
    if let Some(name) = media.file_name().and_then(|n| n.to_str()) {
        doc.set_media_basename(name)
            .context("failed to set media basename")?;
    }

    doc.to_path(&out_path)
        .with_context(|| format!("failed to write sidecar to {}", out_path.display()))?;

    println!("Created {}", out_path.display());
    Ok(())
}

struct SetArgs {
    file: PathBuf,
    f64: Option<String>,
    f32: Option<String>,
    u64: Option<String>,
    i64: Option<String>,
    bool_arg: Option<String>,
    str: Option<String>,
    bytes: Option<String>,
    json: Option<String>,
}

fn cmd_set(args: SetArgs) -> Result<()> {
    let SetArgs {
        file,
        f64,
        f32,
        u64,
        i64,
        bool_arg,
        str: str_arg,
        bytes,
        json,
    } = args;
    let resolved = resolve_sidecar_path(&file);
    let file = resolved.as_path();
    if let Some(v) = f64 {
        let (key, val) = parse_pair(&v, "f64")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::Float(val.parse().context("invalid f64")?))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = f32 {
        let (key, val) = parse_pair(&v, "f32")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::Float(val.parse().context("invalid f32")?))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = u64 {
        let (key, val) = parse_pair(&v, "u64")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::Integer(val.parse().context("invalid u64")?))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = i64 {
        let (key, val) = parse_pair(&v, "i64")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::Integer(val.parse().context("invalid i64")?))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = bool_arg {
        let (key, val) = parse_pair(&v, "bool")?;
        let parsed = match val.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" => false,
            _ => bail!("invalid bool value: {val}"),
        };
        let mut doc = load_doc(file)?;
        doc.set(key, Value::Bool(parsed))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = str_arg {
        let (key, val) = parse_pair(&v, "str")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::Text(val))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = bytes {
        let (key, path) = parse_bytes_pair(&v)?;
        let data = fs::read(&path)
            .with_context(|| format!("failed to read bytes from {}", path.display()))?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::Bytes(data))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = json {
        let (key, val) = parse_pair(&v, "json")?;
        let parsed: serde_json::Value = serde_json::from_str(&val).context("invalid JSON value")?;
        let mut doc = load_doc(file)?;
        doc.set(key, json_to_value(&parsed)?)
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }

    bail!("specify one of --f64, --f32, --u64, --i64, --bool, --str, --bytes, or --json")
}

fn cmd_get(file: &Path, key: &str) -> Result<()> {
    let resolved = resolve_sidecar_path(file);
    let doc = load_doc(&resolved)?;
    match doc.get(key) {
        Some(value) => println!("{}", format_value(value)),
        None => bail!("key not found: {key}"),
    }
    Ok(())
}

fn cmd_list(file: &Path) -> Result<()> {
    let resolved = resolve_sidecar_path(file);
    let doc = load_doc(&resolved)?;
    for (key, value) in doc.entries() {
        if key == MEDIA_BASENAME_KEY {
            continue;
        }
        println!("{}\t{}", key, value.type_name());
    }
    Ok(())
}

fn cmd_inspect(file: &Path) -> Result<()> {
    let resolved = resolve_sidecar_path(file);
    let doc = load_doc(&resolved)?;
    println!("format: CBOR");
    println!("entries: {}", doc.entry_count());
    println!(
        "byte_len: {}",
        doc.byte_len().context("failed to measure size")?
    );
    if let Some(basename) = doc.media_basename() {
        println!("media: {basename}");
    }
    println!("--- catalog ---");
    for (key, value) in doc.entries() {
        println!("  {}: {} = {}", key, value.type_name(), format_value(value));
    }
    Ok(())
}

/// Resolve the sidecar path from a user-supplied path.
fn resolve_sidecar_path(path: &Path) -> PathBuf {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case(SIDECAR_EXTENSION) => path.to_path_buf(),
        _ => sidecar_path_for_media(path),
    }
}

fn load_doc(path: &Path) -> Result<SidecarDocument> {
    SidecarDocument::from_path(path)
        .with_context(|| format!("failed to read sidecar {}", path.display()))
}

fn save_doc(doc: &SidecarDocument, path: &Path) -> Result<()> {
    doc.to_path(path)
        .with_context(|| format!("failed to write sidecar {}", path.display()))
}

fn parse_pair(input: &str, kind: &str) -> Result<(String, String)> {
    let Some((key, val)) = input.split_once('=') else {
        bail!("{kind} argument must be KEY=VALUE, got: {input}");
    };
    if key.is_empty() {
        bail!("key must not be empty");
    }
    Ok((key.to_string(), val.to_string()))
}

fn parse_bytes_pair(input: &str) -> Result<(String, PathBuf)> {
    let Some((key, path)) = input.split_once('=') else {
        bail!("bytes argument must be KEY=@path, got: {input}");
    };
    let path = path.strip_prefix('@').unwrap_or(path);
    Ok((key.to_string(), PathBuf::from(path)))
}

fn json_to_value(json: &serde_json::Value) -> Result<Value> {
    match json {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(v) => Ok(Value::Bool(*v)),
        serde_json::Value::Number(n) => {
            if let Some(v) = n.as_i64() {
                Ok(Value::Integer(v as i128))
            } else if let Some(v) = n.as_u64() {
                Ok(Value::Integer(v as i128))
            } else if let Some(v) = n.as_f64() {
                Ok(Value::Float(v))
            } else {
                bail!("unsupported JSON number");
            }
        }
        serde_json::Value::String(v) => Ok(Value::Text(v.clone())),
        serde_json::Value::Array(items) => {
            let elements = items
                .iter()
                .map(json_to_value)
                .collect::<Result<Vec<_>>>()?;
            Ok(Value::Array(elements))
        }
        serde_json::Value::Object(map) => {
            let mut entries = IndexMap::with_capacity(map.len());
            for (key, val) in map {
                if key.is_empty() {
                    bail!("JSON object key must not be empty");
                }
                entries.insert(key.clone(), json_to_value(val)?);
            }
            Ok(Value::Map(entries))
        }
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(v) => v.to_string(),
        Value::Integer(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::Text(v) => v.clone(),
        Value::Bytes(v) => format!("<bytes len={}>", v.len()),
        Value::Array(elements) => {
            let parts: Vec<String> = elements.iter().map(format_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Map(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{k}: {}", format_value(v)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}
