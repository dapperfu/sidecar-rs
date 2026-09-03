//! Benchmark lock/unlock of every `.scar` found under a vault directory.
//!
//! Does **not** rewrite sidecar contents. Each cycle only creates `{path}.lock`,
//! takes an exclusive flock, unlinks, and closes.
//!
//! ```text
//! cargo run --release -p sidecar --example bench_dir_locks -- /tun/pictures
//! cargo run --release -p sidecar --example bench_dir_locks -- /tun/pictures --rmw-copies 200
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sidecar::{
    lock_unlock_sidecar_phases, update_path_with_timeout, SidecarDocument, DEFAULT_LOCK_TIMEOUT,
};

const DEFAULT_ROOT: &str = "/tun/pictures";
const DEFAULT_SIZES: &[usize] = &[1_000, 10_000, 100_000];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut rmw_copies: usize = 0;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rmw-copies" => {
                i += 1;
                rmw_copies = args.get(i).ok_or("--rmw-copies needs a count")?.parse()?;
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: bench_dir_locks [DIR] [--rmw-copies N]\n\
                     Default DIR={DEFAULT_ROOT}\n\
                     Lock/unlock the first 1e3, 1e4, 1e5 (or however many .scar files exist).\n\
                     --rmw-copies N  also time identity update_path on N copies in a temp dir"
                );
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown flag {other}").into());
            }
            other => root = PathBuf::from(other),
        }
        i += 1;
    }

    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()).into());
    }

    eprintln!("Walking {} for .scar files ...", root.display());
    let walk_start = Instant::now();
    let max_n = *DEFAULT_SIZES.iter().max().unwrap_or(&100_000);
    let mut paths = Vec::with_capacity(max_n.min(65_536));
    collect_scar_paths(&root, &mut paths, max_n)?;
    let walk_elapsed = walk_start.elapsed();
    let found = paths.len();
    eprintln!(
        "Found {found} sidecar files in {:.3}s ({:.0} files/s)",
        walk_elapsed.as_secs_f64(),
        found as f64 / walk_elapsed.as_secs_f64().max(1e-9)
    );

    println!();
    println!(
        "{:>8} {:>12} {:>12} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "n", "lock_s", "per_file_us", "files_s", "open_s", "flock_s", "ident_s", "rel_s", "other_s"
    );

    for &size in DEFAULT_SIZES {
        let n = size.min(found);
        if n == 0 {
            break;
        }
        run_lock_size(&paths[..n])?;
        if n == found && size > found {
            eprintln!("(stopped at {found} files on disk; requested {size})");
            break;
        }
    }

    if rmw_copies > 0 {
        let n = rmw_copies.min(found);
        eprintln!();
        eprintln!("Identity update_path on {n} copies (does not touch the vault) ...");
        rmw_on_copies(&paths[..n])?;
    }

    Ok(())
}

fn run_lock_size(paths: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    let n = paths.len();
    let timeout = DEFAULT_LOCK_TIMEOUT;

    let mut open = Duration::ZERO;
    let mut flock = Duration::ZERO;
    let mut identity = Duration::ZERO;
    let mut release = Duration::ZERO;
    let start = Instant::now();
    for path in paths {
        let phases = lock_unlock_sidecar_phases(path, timeout)?;
        open += phases.open;
        flock += phases.flock;
        identity += phases.identity;
        release += phases.release;
    }
    let lock_elapsed = start.elapsed();
    let accounted = open + flock + identity + release;
    let other = lock_elapsed.saturating_sub(accounted);

    let secs = lock_elapsed.as_secs_f64();
    let per_us = if n == 0 { 0.0 } else { secs * 1e6 / n as f64 };
    let rate = n as f64 / secs.max(1e-9);
    println!(
        "{n:>8} {secs:>12.3} {per_us:>12.1} {rate:>10.0} {:>10.3} {:>10.3} {:>10.3} {:>10.3} {:>10.3}",
        open.as_secs_f64(),
        flock.as_secs_f64(),
        identity.as_secs_f64(),
        release.as_secs_f64(),
        other.as_secs_f64(),
    );

    let phases = [
        ("open+create lock file", open.as_secs_f64()),
        ("flock wait/acquire", flock.as_secs_f64()),
        ("inode identity check", identity.as_secs_f64()),
        ("unlink+close (release)", release.as_secs_f64()),
        ("other (path/setup)", other.as_secs_f64()),
    ];
    let mut ranked = phases;
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    eprintln!(
        "  n={n} pinch: {} ({:.1}%)",
        ranked[0].0,
        100.0 * ranked[0].1 / secs.max(1e-9)
    );
    Ok(())
}

fn rmw_on_copies(paths: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    let tmp = tempfile::Builder::new()
        .prefix("sidecar-lock-bench-")
        .tempdir()?;
    let mut copies = Vec::with_capacity(paths.len());
    for (i, src) in paths.iter().enumerate() {
        let dest = tmp.path().join(format!("{i}.scar"));
        std::fs::copy(src, &dest)?;
        copies.push(dest);
    }

    let timeout = DEFAULT_LOCK_TIMEOUT;
    let start = Instant::now();
    for path in &copies {
        update_path_with_timeout(path, timeout, |_doc: &mut SidecarDocument| Ok(()))?;
    }
    let elapsed = start.elapsed();
    let n = copies.len();
    eprintln!(
        "  rmw n={n}: {:.3}s ({:.1} us/file, {:.0} files/s)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e6 / n as f64,
        n as f64 / elapsed.as_secs_f64().max(1e-9)
    );
    Ok(())
}

fn collect_scar_paths(root: &Path, out: &mut Vec<PathBuf>, limit: usize) -> std::io::Result<()> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => continue,
            Err(err) => return Err(err),
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_dir() {
                stack.push(path);
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("scar") {
                continue;
            }
            out.push(path);
            if out.len() >= limit {
                return Ok(());
            }
        }
    }
    Ok(())
}
