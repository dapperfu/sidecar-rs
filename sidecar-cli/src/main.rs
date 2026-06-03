use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use sidecar::conventions::photo;
use sidecar::format::value::Value;
use sidecar::{sidecar_path_for_media, SidecarDocument, MEDIA_BASENAME_KEY, SIDECAR_EXTENSION};

#[derive(Parser)]
#[command(
    name = "sidecar",
    about = "Create and inspect SCAR binary sidecar files"
)]
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
    /// Inspect header and catalog summary
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
        } => cmd_set(SetArgs {
            file,
            f64,
            f32,
            u64,
            i64,
            bool_arg,
            str,
            bytes,
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
    } = args;
    let resolved = resolve_sidecar_path(&file);
    let file = resolved.as_path();
    if let Some(v) = f64 {
        let (key, val) = parse_pair(&v, "f64")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::F64(val.parse().context("invalid f64")?))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = f32 {
        let (key, val) = parse_pair(&v, "f32")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::F32(val.parse().context("invalid f32")?))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = u64 {
        let (key, val) = parse_pair(&v, "u64")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::U64(val.parse().context("invalid u64")?))
            .context("failed to set value")?;
        save_doc(&doc, file)?;
        return Ok(());
    }
    if let Some(v) = i64 {
        let (key, val) = parse_pair(&v, "i64")?;
        let mut doc = load_doc(file)?;
        doc.set(key, Value::I64(val.parse().context("invalid i64")?))
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
        doc.set(key, Value::String(val))
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

    bail!("specify one of --f64, --f32, --u64, --i64, --bool, --str, or --bytes")
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
        println!("{}\t{}", key, value_kind_label(value));
    }
    Ok(())
}

fn cmd_inspect(file: &Path) -> Result<()> {
    let resolved = resolve_sidecar_path(file);
    let doc = load_doc(&resolved)?;
    let header = doc.header_info();
    println!("SCAR v{}", header.version);
    println!("catalog_bytes: {}", header.catalog_len);
    println!("payload_bytes: {}", header.payload_len);
    println!("entries: {}", doc.entry_count());
    if let Some(basename) = doc.media_basename() {
        println!("media: {basename}");
    }
    println!("--- catalog ---");
    for (key, value) in doc.entries() {
        println!(
            "  {}: {} = {}",
            key,
            value_kind_label(value),
            format_value(value)
        );
    }
    let _ = photo::GPS_LATITUDE;
    Ok(())
}

/// Resolve the sidecar path from a user-supplied path.
///
/// If the path already has the `.scar` extension it is used verbatim; otherwise
/// it is treated as a media file (e.g. `photo.jpg`) and the sidecar path is
/// derived by swapping the extension to `.scar` (`photo.scar`).
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

fn value_kind_label(value: &Value) -> &'static str {
    match value {
        Value::Array { element_kind, .. } => {
            let _ = element_kind;
            "Array"
        }
        other => other.kind().name(),
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(v) => v.to_string(),
        Value::I64(v) => v.to_string(),
        Value::U64(v) => v.to_string(),
        Value::F32(v) => v.to_string(),
        Value::F64(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Bytes(v) => format!("<bytes len={}>", v.len()),
        Value::Array { elements, .. } => {
            let parts: Vec<String> = elements.iter().map(format_value).collect();
            format!("[{}]", parts.join(", "))
        }
    }
}
