// Copyright (c) 2024-2025 Wanyeki Technologies LLC. All rights reserved.
// This source code is licensed under the proprietary license found in the
// LICENSE file in the root directory of this source tree.

//! SPICE simulation execution and result parsing (LTspice and ngspice)

use std::path::PathBuf;
use std::process::Command;
use encoding_rs::UTF_16LE;
use regex::Regex;
use tempfile::Builder;
use std::io::{BufRead, BufReader};

use crate::protocol::{SimulationResults, Trace};

/// Standard libraries bundled with the agent (fallback)
const STANDARD_LIBRARIES: &[&str] = &["LTC3.lib"];

/// Known ngspice installation paths on Windows
#[cfg(windows)]
const NGSPICE_PATHS_WINDOWS: &[&str] = &[
    r"C:\Program Files\ngspice\bin\ngspice.exe",
    r"C:\Program Files (x86)\ngspice\bin\ngspice.exe",
    r"C:\Spice64\bin\ngspice.exe",
    r"C:\Spice\bin\ngspice.exe",
];

/// Known ngspice installation paths on macOS
#[cfg(target_os = "macos")]
const NGSPICE_PATHS_MACOS: &[&str] = &[
    "/opt/homebrew/bin/ngspice",
    "/usr/local/bin/ngspice",
    "/opt/local/bin/ngspice",
];

/// Known LTspice installation paths on Windows
#[cfg(windows)]
const LTSPICE_PATHS_WINDOWS: &[&str] = &[
    r"C:\Program Files\LTC\LTspiceXVII\XVIIx64.exe",
    r"C:\Program Files\LTC\LTspice\LTspice.exe",
    r"C:\Program Files (x86)\LTC\LTspiceXVII\XVIIx86.exe",
    r"C:\Program Files (x86)\LTC\LTspice\LTspice.exe",
];

/// Known LTspice library paths on Windows
#[cfg(windows)]
const LTSPICE_LIB_PATHS_WINDOWS: &[&str] = &[
    r"C:\Program Files\LTC\LTspiceXVII\lib",
    r"C:\Program Files\LTC\LTspice\lib",
    r"C:\Program Files (x86)\LTC\LTspiceXVII\lib",
    r"C:\Program Files (x86)\LTC\LTspice\lib",
    r"C:\Users\Public\Documents\LTspiceXVII\lib",
];

/// Known LTspice installation paths on macOS
#[cfg(target_os = "macos")]
const LTSPICE_PATHS_MACOS: &[&str] = &[
    "/Applications/LTspice.app/Contents/MacOS/LTspice",
];

/// Known LTspice library paths on macOS
#[cfg(target_os = "macos")]
const LTSPICE_LIB_PATHS_MACOS: &[&str] = &[
    "/Applications/LTspice.app/Contents/Resources/lib",
    "~/Library/Application Support/LTspice/lib",
    "~/Documents/LTspiceXVII/lib",
];

/// Known ngspice library paths on Windows
#[cfg(windows)]
const NGSPICE_LIB_PATHS_WINDOWS: &[&str] = &[
    // ngspice installation directories
    r"C:\Program Files\ngspice\share\ngspice\scripts",
    r"C:\Program Files (x86)\ngspice\share\ngspice\scripts",
    r"C:\Spice64\share\ngspice\scripts",
    r"C:\Spice\share\ngspice\scripts",
    // Common user model directories
    r"C:\ngspice\lib",
    r"C:\ngspice\models",
];

/// Known ngspice library paths on macOS
#[cfg(target_os = "macos")]
const NGSPICE_LIB_PATHS_MACOS: &[&str] = &[
    // ngspice installation directories
    "/opt/homebrew/share/ngspice/scripts",
    "/usr/local/share/ngspice/scripts",
    "/opt/local/share/ngspice/scripts",
    "/opt/homebrew/Cellar/ngspice/*/share/ngspice/scripts",
    // User model directories
    "~/ngspice/lib",
    "~/ngspice/models",
    "~/.ngspice/lib",
    "~/.ngspice/models",
    "~/Documents/ngspice/lib",
    "~/Documents/ngspice/models",
    // System-wide model directories
    "/usr/local/share/ngspice/lib",
    "/opt/homebrew/share/ngspice/lib",
];

/// Detect LTspice installation
pub fn detect_ltspice() -> Option<String> {
    #[cfg(windows)]
    {
        for path in LTSPICE_PATHS_WINDOWS {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(path.to_string());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        for path in LTSPICE_PATHS_MACOS {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(path.to_string());
            }
        }
    }

    // Also check PATH
    if let Ok(output) = Command::new("which").arg("ltspice").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

/// Detect ngspice installation
pub fn detect_ngspice() -> Option<String> {
    #[cfg(windows)]
    {
        for path in NGSPICE_PATHS_WINDOWS {
            let p = PathBuf::from(path);
            if p.exists() {
                log::info!("Found ngspice at: {}", path);
                return Some(path.to_string());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        for path in NGSPICE_PATHS_MACOS {
            let p = PathBuf::from(path);
            if p.exists() {
                log::info!("Found ngspice at: {}", path);
                return Some(path.to_string());
            }
        }
    }

    // Also check PATH using 'which' on Unix or 'where' on Windows
    #[cfg(unix)]
    {
        if let Ok(output) = Command::new("which").arg("ngspice").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    log::info!("Found ngspice in PATH: {}", path);
                    return Some(path);
                }
            }
        }
    }

    #[cfg(windows)]
    {
        if let Ok(output) = Command::new("where").arg("ngspice").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .map(|s| s.trim().to_string());
                if let Some(p) = path {
                    if !p.is_empty() {
                        log::info!("Found ngspice in PATH: {}", p);
                        return Some(p);
                    }
                }
            }
        }
    }

    None
}

/// Detect LTspice library directory
pub fn detect_ltspice_lib_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        for path in LTSPICE_LIB_PATHS_WINDOWS {
            let p = PathBuf::from(path);
            if p.exists() && p.is_dir() {
                log::info!("Found LTspice library directory: {:?}", p);
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        for path in LTSPICE_LIB_PATHS_MACOS {
            // Expand ~ to home directory
            let expanded = if path.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&path[2..])
                } else {
                    PathBuf::from(path)
                }
            } else {
                PathBuf::from(path)
            };

            if expanded.exists() && expanded.is_dir() {
                log::info!("Found LTspice library directory: {:?}", expanded);
                return Some(expanded);
            }
        }
    }

    // Try to find lib directory relative to LTspice executable
    if let Some(ltspice_path) = detect_ltspice() {
        let exe_path = PathBuf::from(&ltspice_path);

        #[cfg(target_os = "macos")]
        {
            // On macOS: /Applications/LTspice.app/Contents/MacOS/LTspice -> /Applications/LTspice.app/Contents/Resources/lib
            if let Some(contents_dir) = exe_path.parent().and_then(|p| p.parent()) {
                let lib_dir = contents_dir.join("Resources").join("lib");
                if lib_dir.exists() {
                    log::info!("Found LTspice library directory relative to exe: {:?}", lib_dir);
                    return Some(lib_dir);
                }
            }
        }

        #[cfg(windows)]
        {
            // On Windows: C:\Program Files\LTC\LTspice\LTspice.exe -> C:\Program Files\LTC\LTspice\lib
            if let Some(exe_dir) = exe_path.parent() {
                let lib_dir = exe_dir.join("lib");
                if lib_dir.exists() {
                    log::info!("Found LTspice library directory relative to exe: {:?}", lib_dir);
                    return Some(lib_dir);
                }
            }
        }
    }

    log::warn!("Could not find LTspice library directory");
    None
}

/// Detect ngspice library/scripts directory
pub fn detect_ngspice_lib_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        for path in NGSPICE_LIB_PATHS_WINDOWS {
            let p = PathBuf::from(path);
            if p.exists() && p.is_dir() {
                log::info!("Found ngspice library directory: {:?}", p);
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        for path in NGSPICE_LIB_PATHS_MACOS {
            // Expand ~ to home directory
            let expanded = if path.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&path[2..])
                } else {
                    PathBuf::from(path)
                }
            } else if path.contains('*') {
                // Handle glob patterns for Cellar paths
                if let Some(found) = expand_glob_path(path) {
                    found
                } else {
                    continue;
                }
            } else {
                PathBuf::from(path)
            };

            if expanded.exists() && expanded.is_dir() {
                log::info!("Found ngspice library directory: {:?}", expanded);
                return Some(expanded);
            }
        }
    }

    // Try to find lib directory relative to ngspice executable
    if let Some(ngspice_path) = detect_ngspice() {
        let exe_path = PathBuf::from(&ngspice_path);

        // On Unix: /opt/homebrew/bin/ngspice -> /opt/homebrew/share/ngspice/scripts
        if let Some(bin_dir) = exe_path.parent() {
            if let Some(prefix) = bin_dir.parent() {
                let lib_dir = prefix.join("share").join("ngspice").join("scripts");
                if lib_dir.exists() {
                    log::info!("Found ngspice library directory relative to exe: {:?}", lib_dir);
                    return Some(lib_dir);
                }
            }
        }
    }

    log::warn!("Could not find ngspice library directory");
    None
}

/// Expand a glob pattern path (simple implementation for Homebrew Cellar paths)
#[cfg(target_os = "macos")]
fn expand_glob_path(pattern: &str) -> Option<PathBuf> {
    use std::fs;

    // Split at the wildcard
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() != 2 {
        return None;
    }

    let prefix = PathBuf::from(parts[0].trim_end_matches('/'));
    let suffix = parts[1].trim_start_matches('/');

    // List directories in prefix and find the latest version
    if let Ok(entries) = fs::read_dir(&prefix) {
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();

        // Sort to get latest version
        versions.sort();

        if let Some(latest) = versions.last() {
            let full_path = latest.join(suffix);
            if full_path.exists() {
                return Some(full_path);
            }
        }
    }

    None
}

/// Get the path to bundled resources
fn get_resources_dir() -> Option<PathBuf> {
    // When running in development, resources are in src-tauri/resources
    // When bundled, they're in the app bundle's Resources directory

    // Try the bundled location first (macOS)
    #[cfg(target_os = "macos")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            // In a macOS bundle: .app/Contents/MacOS/binary -> .app/Contents/Resources
            if let Some(contents_dir) = exe_path.parent().and_then(|p| p.parent()) {
                let resources = contents_dir.join("Resources");
                if resources.exists() {
                    return Some(resources);
                }
            }
        }
    }

    // Try Windows bundled location
    #[cfg(windows)]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let resources = exe_dir.join("resources");
                if resources.exists() {
                    return Some(resources);
                }
            }
        }
    }

    // Development fallback - look relative to current dir
    let dev_resources = PathBuf::from("resources");
    if dev_resources.exists() {
        return Some(dev_resources);
    }

    // Try relative to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let resources = exe_dir.join("resources");
            if resources.exists() {
                return Some(resources);
            }
        }
    }

    None
}

/// Process .include and .lib directives in the netlist
/// Resolves library files from LTspice's library directory or bundled resources
fn process_includes(
    netlist: &str,
    temp_dir: &std::path::Path,
) -> Result<(String, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
    let mut processed_netlist = netlist.to_string();
    let mut copied_files: Vec<String> = Vec::new();

    // Match .include or .lib directives
    let include_pattern = Regex::new(r#"(?im)^\s*\.(?:include|lib)\s+(.+?)\s*$"#)?;

    let resources_dir = get_resources_dir();
    let ltspice_lib_dir = detect_ltspice_lib_dir();

    for cap in include_pattern.captures_iter(netlist) {
        let full_match = cap.get(0).unwrap().as_str();
        let path_str = cap.get(1).unwrap().as_str().trim_matches(|c| c == '"' || c == '\'');

        // Extract filename from path
        let file_name = std::path::Path::new(path_str)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path_str);

        // Check if the path is absolute and exists
        let path_as_is = PathBuf::from(path_str);
        if path_as_is.is_absolute() && path_as_is.exists() {
            log::info!("Using absolute library path: {:?}", path_as_is);
            continue; // Keep as-is, LTspice will find it
        }

        // Try to find the library in LTspice's lib directory (search recursively)
        if let Some(ref lib_dir) = ltspice_lib_dir {
            if let Some(found_path) = find_library_file(lib_dir, file_name) {
                // Copy the library to temp dir to ensure LTspice can access it
                let dest_path = temp_dir.join(file_name);
                if std::fs::copy(&found_path, &dest_path).is_ok() {
                    copied_files.push(file_name.to_string());
                    processed_netlist = processed_netlist.replace(
                        full_match,
                        &format!(".include {}", file_name),
                    );
                    log::info!("Copied LTspice library: {:?} -> {:?}", found_path, dest_path);
                    continue;
                }
            }
        }

        // Fallback: check if this is a standard library we bundle
        if STANDARD_LIBRARIES.contains(&file_name) {
            if let Some(ref res_dir) = resources_dir {
                let src_path = res_dir.join(file_name);
                let dest_path = temp_dir.join(file_name);

                if src_path.exists() {
                    std::fs::copy(&src_path, &dest_path)?;
                    copied_files.push(file_name.to_string());

                    // Update the netlist to use the local copy
                    processed_netlist = processed_netlist.replace(
                        full_match,
                        &format!(".include {}", file_name),
                    );

                    log::info!("Copied bundled library: {} -> {:?}", file_name, dest_path);
                    continue;
                }
            }
        }

        log::warn!("Library not found: {} - simulation may fail", file_name);
    }

    Ok((processed_netlist, copied_files))
}

/// Recursively search for a library file in a directory
fn find_library_file(dir: &PathBuf, file_name: &str) -> Option<PathBuf> {
    find_library_file_recursive(dir, file_name, 0, 4)
}

/// Recursively search for a library file with depth limit
fn find_library_file_recursive(dir: &PathBuf, file_name: &str, depth: usize, max_depth: usize) -> Option<PathBuf> {
    if depth > max_depth {
        return None;
    }

    // First check directly in the directory
    let direct_path = dir.join(file_name);
    if direct_path.exists() {
        return Some(direct_path);
    }

    // Check subdirectories
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_library_file_recursive(&path, file_name, depth + 1, max_depth) {
                    return Some(found);
                }
            }
        }
    }

    None
}

/// List available libraries from LTspice's library directory
pub fn list_available_libraries() -> Vec<String> {
    list_ltspice_libraries()
}

/// List available libraries from LTspice's library directory
pub fn list_ltspice_libraries() -> Vec<String> {
    let mut libraries = Vec::new();

    if let Some(lib_dir) = detect_ltspice_lib_dir() {
        collect_library_files(&lib_dir, &mut libraries, 0, 3);
    }

    // Sort and deduplicate
    libraries.sort();
    libraries.dedup();
    libraries
}

/// List available libraries/scripts from all ngspice directories
pub fn list_ngspice_libraries() -> Vec<String> {
    let mut libraries = Vec::new();

    // Collect from all known ngspice library paths
    let all_paths = get_all_ngspice_lib_dirs();
    for lib_dir in all_paths {
        collect_ngspice_files(&lib_dir, &mut libraries, 0, 3);
    }

    // Sort and deduplicate
    libraries.sort();
    libraries.dedup();
    libraries
}

/// Get all existing ngspice library directories
fn get_all_ngspice_lib_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(windows)]
    {
        for path in NGSPICE_LIB_PATHS_WINDOWS {
            let p = PathBuf::from(path);
            if p.exists() && p.is_dir() {
                dirs.push(p);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        for path in NGSPICE_LIB_PATHS_MACOS {
            // Expand ~ to home directory
            let expanded = if path.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&path[2..])
                } else {
                    continue;
                }
            } else if path.contains('*') {
                // Handle glob patterns for Cellar paths
                if let Some(found) = expand_glob_path(path) {
                    found
                } else {
                    continue;
                }
            } else {
                PathBuf::from(path)
            };

            if expanded.exists() && expanded.is_dir() {
                dirs.push(expanded);
            }
        }
    }

    // Also try relative to ngspice executable
    if let Some(ngspice_path) = detect_ngspice() {
        let exe_path = PathBuf::from(&ngspice_path);

        // On Unix: /opt/homebrew/bin/ngspice -> /opt/homebrew/share/ngspice/scripts
        if let Some(bin_dir) = exe_path.parent() {
            if let Some(prefix) = bin_dir.parent() {
                let scripts_dir = prefix.join("share").join("ngspice").join("scripts");
                if scripts_dir.exists() && !dirs.contains(&scripts_dir) {
                    dirs.push(scripts_dir);
                }

                // Also check for lib subdirectory
                let lib_dir = prefix.join("share").join("ngspice").join("lib");
                if lib_dir.exists() && !dirs.contains(&lib_dir) {
                    dirs.push(lib_dir);
                }
            }
        }
    }

    log::info!("Found {} ngspice library directories", dirs.len());
    dirs
}

/// Recursively collect ngspice script files from a directory
fn collect_ngspice_files(dir: &PathBuf, libraries: &mut Vec<String>, depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_ngspice_files(&path, libraries, depth + 1, max_depth);
            } else if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                // ngspice uses various script/include file extensions
                // .lib = library, .mod = model, .inc = include, .sub = subcircuit
                // .cir/.spi/.sp = circuit/spice files
                if ext_str == "lib" || ext_str == "mod" || ext_str == "inc"
                   || ext_str == "sub" || ext_str == "cir" || ext_str == "spi" || ext_str == "sp" {
                    if let Some(file_name) = path.file_name() {
                        libraries.push(file_name.to_string_lossy().to_string());
                    }
                }
            } else {
                // Also check files without extensions (ngspice scripts often have no extension)
                if let Some(file_name) = path.file_name() {
                    let name = file_name.to_string_lossy();
                    // Include common script file patterns
                    if name.starts_with("spinit") || name.ends_with("rc") {
                        libraries.push(name.to_string());
                    }
                }
            }
        }
    }
}

/// Recursively collect library files from a directory
fn collect_library_files(dir: &PathBuf, libraries: &mut Vec<String>, depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_library_files(&path, libraries, depth + 1, max_depth);
            } else if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                // Common library file extensions
                if ext_str == "lib" || ext_str == "sub" || ext_str == "mod" || ext_str == "inc" {
                    if let Some(file_name) = path.file_name() {
                        libraries.push(file_name.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
}

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Run an LTspice simulation
/// The process_id_holder will be updated with the PID when the process starts
pub async fn run_ltspice_simulation(
    ltspice_path: &str,
    netlist: &str,
    waveform_quality: &str,
    process_id_holder: Option<Arc<AtomicU32>>,
) -> Result<SimulationResults, Box<dyn std::error::Error + Send + Sync>> {
    // Create temp directory with kelicad prefix
    let temp_dir = Builder::new().prefix("kelicad-sim-").tempdir()?;
    log::info!("Created temp directory: {:?}", temp_dir.path());
    let netlist_path = temp_dir.path().join("circuit.net");
    let raw_path = temp_dir.path().join("circuit.raw");
    let log_path = temp_dir.path().join("circuit.log");

    // Process includes - copy standard libraries to temp dir and update paths
    let (processed_netlist, _copied_files) = process_includes(netlist, temp_dir.path())?;

    // Prepare netlist with required directives
    let prepared_netlist = prepare_netlist(&processed_netlist, waveform_quality);
    std::fs::write(&netlist_path, &prepared_netlist)?;

    log::info!("Running LTspice simulation...");

    // Run LTspice in batch mode using spawn() so we can get the PID
    let output = tokio::task::spawn_blocking({
        let ltspice_path = ltspice_path.to_string();
        let netlist_path = netlist_path.clone();
        move || {
            let child = Command::new(&ltspice_path)
                .arg("-b")
                .arg(&netlist_path)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            let pid = child.id();
            log::info!("LTspice process started with PID: {}", pid);

            // Store the PID in the holder if provided
            if let Some(holder) = process_id_holder {
                holder.store(pid, Ordering::SeqCst);
            }

            // Wait for the process to complete
            let output = child.wait_with_output()?;
            Ok::<_, std::io::Error>(output)
        }
    })
    .await??;

    if !output.status.success() {
        // Try to read log file for error details
        let log_content = std::fs::read_to_string(&log_path).unwrap_or_default();
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "LTspice failed: {}\n{}",
            stderr,
            log_content
        )
        .into());
    }

    // Check if raw file exists
    if !raw_path.exists() {
        return Err("No .raw file generated - simulation may have failed".into());
    }

    // Parse the raw file
    log::info!("Parsing raw file: {:?}", raw_path);
    let results = parse_raw_file(&raw_path)?;

    Ok(results)
}

/// Run an ngspice simulation
/// The process_id_holder will be updated with the PID when the process starts
pub async fn run_ngspice_simulation(
    ngspice_path: &str,
    netlist: &str,
    _waveform_quality: &str,
    process_id_holder: Option<Arc<AtomicU32>>,
) -> Result<SimulationResults, Box<dyn std::error::Error + Send + Sync>> {
    // Create temp directory with kelicad prefix
    let temp_dir = Builder::new().prefix("kelicad-ngspice-").tempdir()?;
    log::info!("Created temp directory for ngspice: {:?}", temp_dir.path());
    let netlist_path = temp_dir.path().join("circuit.cir");
    let raw_path = temp_dir.path().join("circuit.raw");

    // Prepare netlist with .control section for raw output
    let prepared_netlist = prepare_ngspice_netlist(netlist, &raw_path);
    std::fs::write(&netlist_path, &prepared_netlist)?;

    log::info!("Running ngspice simulation...");

    // Run ngspice in batch mode
    let output = tokio::task::spawn_blocking({
        let ngspice_path = ngspice_path.to_string();
        let netlist_path = netlist_path.clone();
        move || {
            let child = Command::new(&ngspice_path)
                .arg("-b")  // batch mode
                .arg(&netlist_path)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            let pid = child.id();
            log::info!("ngspice process started with PID: {}", pid);

            // Store the PID in the holder if provided
            if let Some(holder) = process_id_holder {
                holder.store(pid, Ordering::SeqCst);
            }

            // Wait for the process to complete
            let output = child.wait_with_output()?;
            Ok::<_, std::io::Error>(output)
        }
    })
    .await??;

    // ngspice returns non-zero for various reasons, check stderr for actual errors
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Log output for debugging
    if !stdout.is_empty() {
        log::info!("ngspice stdout: {}", stdout);
    }
    if !stderr.is_empty() {
        log::warn!("ngspice stderr: {}", stderr);
    }

    // Check for fatal errors in output
    let combined_output = format!("{}\n{}", stdout, stderr);
    if let Some(error_msg) = extract_ngspice_error(&combined_output) {
        return Err(error_msg.into());
    }

    // Check if raw file exists
    if !raw_path.exists() {
        return Err(format!(
            "No .raw file generated - simulation may have failed.\nStdout: {}\nStderr: {}",
            stdout, stderr
        ).into());
    }

    // Parse the raw file (ngspice uses ASCII format by default)
    log::info!("Parsing ngspice raw file: {:?}", raw_path);
    let results = parse_ngspice_raw_file(&raw_path)?;

    Ok(results)
}

/// Extract meaningful error message from ngspice output
/// Returns Some(error_message) if errors found, None otherwise
fn extract_ngspice_error(output: &str) -> Option<String> {
    let lines: Vec<&str> = output.lines().collect();
    let mut error_lines: Vec<String> = Vec::new();
    let mut found_error = false;

    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();

        // Skip notes and warnings, and "0 error" lines
        if lower.contains("note:") || lower.contains("warning:") || lower.contains("0 error") {
            continue;
        }

        // Capture error lines and context
        if lower.contains("error") {
            found_error = true;

            // Include previous line if it has line number info
            if i > 0 {
                let prev = lines[i - 1];
                if prev.to_lowercase().contains("line") && prev.contains(char::is_numeric) {
                    if !error_lines.contains(&prev.trim().to_string()) {
                        error_lines.push(prev.trim().to_string());
                    }
                }
            }

            error_lines.push(line.trim().to_string());

            // Include next line if it's a continuation (indented or has more detail)
            if i + 1 < lines.len() {
                let next = lines[i + 1];
                if next.starts_with(' ') || next.starts_with('\t') {
                    error_lines.push(next.trim().to_string());
                }
            }
        }
        // Also capture "Error on line X" or "line X:" context
        else if lower.contains("error on line") || (lower.contains("line") && lower.contains(':') && line.contains(char::is_numeric)) {
            error_lines.push(line.trim().to_string());
            found_error = true;
        }
    }

    if !error_lines.is_empty() {
        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        let unique_lines: Vec<String> = error_lines
            .into_iter()
            .filter(|line| seen.insert(line.clone()))
            .collect();
        Some(unique_lines.join("\n"))
    } else if found_error {
        Some("ngspice simulation failed".to_string())
    } else {
        None
    }
}

/// Prepare netlist for ngspice with .control section
fn prepare_ngspice_netlist(netlist: &str, raw_path: &PathBuf) -> String {
    let mut lines: Vec<String> = netlist.lines().map(|s| s.to_string()).collect();

    // Find the .end line
    let end_idx = lines.iter().position(|l| l.trim().to_lowercase() == ".end");

    // Check if there's already a .control section
    let has_control = netlist.to_lowercase().contains(".control");

    if !has_control {
        // Add .control section before .end to write raw file
        // ngspice on Unix doesn't like quoted paths - use the path directly
        // For paths with spaces, we use single quotes (ngspice handles these better)
        let raw_path_str = raw_path.to_string_lossy().replace('\\', "/");

        // Use single quotes only if the path contains spaces, otherwise no quotes
        let write_cmd = if raw_path_str.contains(' ') {
            format!("write '{}' all", raw_path_str)
        } else {
            format!("write {} all", raw_path_str)
        };

        let control_section = vec![
            ".control".to_string(),
            "run".to_string(),
            write_cmd,
            "quit".to_string(),
            ".endc".to_string(),
        ];

        if let Some(idx) = end_idx {
            for (i, line) in control_section.into_iter().enumerate() {
                lines.insert(idx + i, line);
            }
        } else {
            // No .end found, append control section and .end
            lines.extend(control_section);
            lines.push(".end".to_string());
        }
    }

    lines.join("\n")
}

/// Parse ngspice raw file format (supports both ASCII and binary, including complex numbers for AC analysis)
fn parse_ngspice_raw_file(path: &PathBuf) -> Result<SimulationResults, Box<dyn std::error::Error + Send + Sync>> {
    // Read the entire file as bytes first
    let data = std::fs::read(path)?;

    // Find where the header ends and data begins
    // Header is ASCII, so we can safely convert it
    let mut num_vars = 0;
    let mut num_points = 0;
    let mut variables: Vec<(String, String)> = Vec::new();
    let mut analysis_type = "transient".to_string();
    let mut x_axis_label = "time".to_string();
    let mut is_binary = false;
    let mut is_complex = false; // AC analysis uses complex numbers
    let mut data_start_offset = 0;

    // Parse header line by line until we hit Values: or Binary:
    let mut in_variables = false;
    let mut line_start = 0;
    let mut var_index = 0;

    for (i, &byte) in data.iter().enumerate() {
        if byte == b'\n' {
            // Extract the line (handle potential \r\n)
            let line_end = if i > 0 && data[i - 1] == b'\r' { i - 1 } else { i };
            let line_bytes = &data[line_start..line_end];

            // Try to parse as UTF-8, skip if invalid
            if let Ok(line) = std::str::from_utf8(line_bytes) {
                let line = line.trim();

                if line.starts_with("Plotname:") {
                    let plotname = line.split(':').nth(1).map(|s| s.trim().to_lowercase()).unwrap_or_default();
                    // Check for specific analysis types - order matters to avoid false matches
                    // "DC transfer characteristic" contains "ac" in "characteristic", so check DC first
                    if plotname.contains("dc") || plotname.contains("operating point") {
                        analysis_type = "dc".to_string();
                    } else if plotname.contains("ac analysis") || plotname.starts_with("ac ") {
                        analysis_type = "ac".to_string();
                    } else if plotname.contains("transient") {
                        analysis_type = "transient".to_string();
                    } else {
                        // Default to transient for unknown analysis types
                        analysis_type = "transient".to_string();
                    }
                } else if line.starts_with("Flags:") {
                    // Check for complex flag (used in AC analysis)
                    let flags = line.to_lowercase();
                    is_complex = flags.contains("complex");
                    log::info!("Flags line: {}, is_complex={}", line, is_complex);
                } else if line.starts_with("No. Variables:") {
                    if let Some(n) = line.split(':').nth(1) {
                        num_vars = n.trim().parse().unwrap_or(0);
                    }
                } else if line.starts_with("No. Points:") {
                    if let Some(n) = line.split(':').nth(1) {
                        num_points = n.trim().parse().unwrap_or(0);
                    }
                } else if line == "Variables:" {
                    in_variables = true;
                    var_index = 0;
                } else if line == "Values:" {
                    is_binary = false;
                    data_start_offset = i + 1;
                    break;
                } else if line == "Binary:" {
                    is_binary = true;
                    data_start_offset = i + 1;
                    break;
                } else if in_variables && !line.is_empty() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let name = parts[1].to_string();
                        let var_type = parts[2].to_string();
                        // First variable (index 0) is the independent variable
                        if var_index == 0 {
                            x_axis_label = name.to_lowercase();
                        }
                        variables.push((name, var_type));
                        var_index += 1;
                    }
                }
            }

            line_start = i + 1;
        }
    }

    log::info!("ngspice raw file: num_vars={}, num_points={}, is_binary={}, is_complex={}, data_offset={}",
               num_vars, num_points, is_binary, is_complex, data_start_offset);

    if num_vars == 0 || variables.is_empty() {
        return Err("Could not parse ngspice raw file header".into());
    }

    let mut all_data: Vec<Vec<f64>> = vec![Vec::with_capacity(num_points); num_vars];

    if is_binary {
        // Parse binary data - ngspice uses float64 for all values
        // For complex data, each variable has 2 float64 values (real, imaginary)
        let binary_data = &data[data_start_offset..];
        let values_per_var = if is_complex { 2 } else { 1 };
        let bytes_per_point = num_vars * values_per_var * 8; // All float64

        log::info!("Parsing binary data: {} bytes, expecting {} points x {} vars x {} values x 8 bytes = {} bytes",
                   binary_data.len(), num_points, num_vars, values_per_var, num_points * bytes_per_point);

        for point in 0..num_points {
            let point_offset = point * bytes_per_point;
            if point_offset + bytes_per_point > binary_data.len() {
                log::warn!("Binary data truncated at point {}", point);
                break;
            }

            for var in 0..num_vars {
                if is_complex {
                    // Read real and imaginary parts, compute magnitude
                    let real_offset = point_offset + var * 16; // 2 x 8 bytes per complex value
                    let imag_offset = real_offset + 8;
                    if let (Ok(real), Ok(imag)) = (read_f64_le(binary_data, real_offset), read_f64_le(binary_data, imag_offset)) {
                        // For the independent variable (frequency), just use the real part
                        // For other variables, compute magnitude: sqrt(real² + imag²)
                        let value = if var == 0 {
                            real // Frequency is real
                        } else {
                            (real * real + imag * imag).sqrt() // Magnitude for voltages/currents
                        };
                        all_data[var].push(value);
                    }
                } else {
                    let offset = point_offset + var * 8;
                    if let Ok(value) = read_f64_le(binary_data, offset) {
                        all_data[var].push(value);
                    }
                }
            }
        }
    } else {
        // Parse ASCII values
        // ngspice ASCII format for complex data:
        //   <point_index>\t<real>,<imag>   <- first variable (frequency)
        //   \t<real>,<imag>                <- second variable
        //   \t<real>,<imag>                <- third variable, etc.
        // For real data, values are just single numbers
        let values_data = &data[data_start_offset..];
        if let Ok(values_str) = std::str::from_utf8(values_data) {
            let mut current_var_index = 0;

            for line in values_str.lines() {
                if line.is_empty() {
                    continue;
                }

                // Check if this is a new point (starts with point index)
                // New points: " <index>\t<value>" -> after trim: "<index>\t<value>" (has tab)
                // Continuation: "\t<value>" -> after trim: "<value>" (no tab)
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // Determine if this is a new point or continuation
                // A new point line has "index\tvalue" format (contains tab and starts with digit)
                // A continuation line has just "value" (no tab after trimming)
                let is_new_point = trimmed.contains('\t') &&
                    trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);

                if is_new_point {
                    // New data point - reset variable index
                    current_var_index = 0;
                }

                // Extract the value part (after index for new points, or the whole line for continuations)
                let value_part = if is_new_point {
                    // Skip the point index - find the tab separator
                    if let Some(tab_pos) = trimmed.find('\t') {
                        &trimmed[tab_pos + 1..]
                    } else {
                        // No tab found, try space separation
                        trimmed.split_whitespace().nth(1).unwrap_or("")
                    }
                } else {
                    // Continuation line - just trim the leading whitespace
                    trimmed
                };

                let value_part = value_part.trim();
                if value_part.is_empty() {
                    continue;
                }

                if is_complex {
                    // Parse complex value: "real,imag"
                    let parts: Vec<&str> = value_part.split(',').collect();
                    if parts.len() >= 2 {
                        if let (Ok(real), Ok(imag)) = (parts[0].trim().parse::<f64>(), parts[1].trim().parse::<f64>()) {
                            // For frequency (var 0), use real part; for others, compute magnitude
                            let value = if current_var_index == 0 {
                                real
                            } else {
                                (real * real + imag * imag).sqrt()
                            };

                            if current_var_index < all_data.len() {
                                all_data[current_var_index].push(value);
                            }
                            current_var_index += 1;
                        }
                    }
                } else {
                    // Parse real value
                    if let Ok(value) = value_part.parse::<f64>() {
                        if current_var_index < all_data.len() {
                            all_data[current_var_index].push(value);
                        }
                        current_var_index += 1;
                    }
                }
            }
        } else {
            return Err("Could not parse ngspice ASCII values as UTF-8".into());
        }
    }

    log::info!("Parsed ngspice raw: num_vars={}, num_points={}, actual_points={}, is_complex={}",
               num_vars, num_points, all_data.get(0).map(|v| v.len()).unwrap_or(0), is_complex);

    if all_data.is_empty() || all_data[0].is_empty() {
        return Err("Could not parse ngspice raw file - no data found".into());
    }

    // Build results
    let time = all_data.get(0).cloned().unwrap_or_default();

    let traces: Vec<Trace> = variables
        .iter()
        .enumerate()
        .skip(1) // Skip time/frequency variable
        .map(|(i, (name, var_type))| {
            let unit = match var_type.as_str() {
                "voltage" => "V",
                "current" => "A",
                "time" => "s",
                "frequency" => "Hz",
                _ => "",
            };
            Trace {
                name: name.clone(),
                data: all_data.get(i).cloned().unwrap_or_default(),
                unit: unit.to_string(),
            }
        })
        .collect();

    Ok(SimulationResults {
        time,
        traces,
        analysis_type,
        x_axis_label: Some(x_axis_label),
    })
}

/// Prepare netlist with required directives for proper output
fn prepare_netlist(netlist: &str, waveform_quality: &str) -> String {
    let mut lines: Vec<String> = netlist.lines().map(|s| s.to_string()).collect();

    // Add .backanno if not present
    if !netlist.to_lowercase().contains(".backanno") {
        // Find the .end line and insert before it
        if let Some(end_idx) = lines.iter().position(|l| l.trim().to_lowercase() == ".end") {
            lines.insert(end_idx, ".backanno".to_string());
        }
    }

    // Add .save all if no .save directive
    if !netlist.to_lowercase().contains(".save") {
        if let Some(end_idx) = lines.iter().position(|l| l.trim().to_lowercase() == ".end") {
            lines.insert(end_idx, ".save all".to_string());
        }
    }

    // Add plotwinsize option based on quality
    let plotwinsize = match waveform_quality {
        "fast" => 128,
        "balanced" => 0,
        "smooth" => 0,
        _ => 0,
    };

    if !netlist.to_lowercase().contains(".options plotwinsize") {
        if let Some(end_idx) = lines.iter().position(|l| l.trim().to_lowercase() == ".end") {
            lines.insert(end_idx, format!(".options plotwinsize={}", plotwinsize));
        }
    }

    lines.join("\n")
}

/// Parse an LTspice .raw file (binary format)
fn parse_raw_file(path: &PathBuf) -> Result<SimulationResults, Box<dyn std::error::Error + Send + Sync>> {
    let data = std::fs::read(path)?;

    // LTspice raw files have a UTF-16LE header followed by binary data
    // Find the "Binary:" marker

    // First, decode as UTF-16LE to find header info
    let (header_text, _, _) = UTF_16LE.decode(&data);

    // Parse header to get variable names and count
    let mut num_vars = 0;
    let mut num_points = 0;
    let mut variables: Vec<(String, String)> = Vec::new();
    let mut in_variables = false;
    let mut is_double = false; // float32 by default, float64 if "double" in Flags

    for line in header_text.lines() {
        let line = line.trim();

        if line.starts_with("No. Variables:") {
            if let Some(n) = line.split(':').nth(1) {
                num_vars = n.trim().parse().unwrap_or(0);
            }
        } else if line.starts_with("No. Points:") {
            if let Some(n) = line.split(':').nth(1) {
                num_points = n.trim().parse().unwrap_or(0);
            }
        } else if line.starts_with("Flags:") {
            // Check if double precision: "Flags: real double forward" vs "Flags: real forward"
            is_double = line.to_lowercase().contains("double");
        } else if line == "Variables:" {
            in_variables = true;
        } else if line == "Binary:" {
            break;
        } else if in_variables && !line.is_empty() {
            // Parse variable line: "0\ttime\ttime"
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let name = parts[1].to_string();
                let var_type = parts[2].to_string();
                variables.push((name, var_type));
            }
        }
    }

    log::info!("Parsed header: num_vars={}, num_points={}, variables={:?}", num_vars, num_points, variables);

    if num_vars == 0 || num_points == 0 {
        return Err("Could not parse raw file header".into());
    }

    if variables.len() != num_vars {
        log::warn!("Variable count mismatch: header says {} but parsed {}", num_vars, variables.len());
    }

    // Find the binary data start marker - try different formats
    // LTspice on Windows uses UTF-16LE with \n, macOS might use different formats
    let binary_start = find_binary_marker(&data)
        .ok_or("Could not find binary data marker")?;

    // Read binary data
    // LTspice "real" format: time is float64, other variables are float32
    // LTspice "real double" format: all variables are float64
    let binary_data = &data[binary_start..];

    // Calculate expected size: time (8 bytes) + other vars (4 bytes each) per point
    // Unless is_double, then all are 8 bytes
    let bytes_per_point = if is_double {
        num_vars * 8
    } else {
        8 + (num_vars - 1) * 4  // time is always float64, others are float32
    };
    let expected_size = num_points * bytes_per_point;

    log::info!("Binary data: {} bytes, expecting {} bytes ({} points x {} bytes/point, is_double={})",
        binary_data.len(), expected_size, num_points, bytes_per_point, is_double);

    if binary_data.len() < expected_size {
        return Err(format!(
            "Binary data too short: expected {} bytes, got {}",
            expected_size,
            binary_data.len()
        )
        .into());
    }

    // Parse the binary data using direct offset reads (matching TypeScript implementation)
    let mut all_data: Vec<Vec<f64>> = vec![Vec::with_capacity(num_points); num_vars];

    for point in 0..num_points {
        let point_offset = point * bytes_per_point;

        // Read time (always float64, 8 bytes)
        let time_value = read_f64_le(binary_data, point_offset)?;
        all_data[0].push(time_value);

        // Read other variables
        for var in 1..num_vars {
            let value = if is_double {
                // All float64
                let offset = point_offset + var * 8;
                read_f64_le(binary_data, offset)?
            } else {
                // Other variables are float32
                let offset = point_offset + 8 + (var - 1) * 4;
                read_f32_le(binary_data, offset)? as f64
            };
            all_data[var].push(value);
        }
    }

    // Build results
    let time = if !all_data.is_empty() {
        all_data[0].clone()
    } else {
        vec![]
    };

    // Debug: log min/max for each variable
    for (i, (name, _)) in variables.iter().enumerate() {
        if let Some(data) = all_data.get(i) {
            let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            log::info!("Variable {}: {} - min={:.6}, max={:.6}, points={}", i, name, min, max, data.len());
        }
    }

    let traces: Vec<Trace> = variables
        .iter()
        .enumerate()
        .skip(1) // Skip time variable (index 0)
        .map(|(i, (name, var_type))| {
            // i is the original index (1, 2, 3, ...) so it matches all_data indices
            let unit = match var_type.as_str() {
                "voltage" => "V",
                "current" => "A",
                "time" => "s",
                _ => "",
            };
            Trace {
                name: name.clone(),
                data: all_data.get(i).cloned().unwrap_or_default(),
                unit: unit.to_string(),
            }
        })
        .collect();

    // Determine analysis type from directives in header
    let analysis_type = if header_text.to_lowercase().contains("transient analysis") {
        "transient"
    } else if header_text.to_lowercase().contains("ac analysis") {
        "ac"
    } else if header_text.to_lowercase().contains("dc") {
        "dc"
    } else {
        "transient"
    };

    // Get the x-axis label from the first variable name
    let x_axis_label = variables.first()
        .map(|(name, _)| name.to_lowercase())
        .unwrap_or_else(|| "time".to_string());

    Ok(SimulationResults {
        time,
        traces,
        analysis_type: analysis_type.to_string(),
        x_axis_label: Some(x_axis_label),
    })
}

/// Read a little-endian f64 from a byte slice at the given offset
fn read_f64_le(data: &[u8], offset: usize) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
    if offset + 8 > data.len() {
        return Err(format!("Buffer overflow reading f64 at offset {}", offset).into());
    }
    let bytes: [u8; 8] = data[offset..offset + 8].try_into()?;
    Ok(f64::from_le_bytes(bytes))
}

/// Read a little-endian f32 from a byte slice at the given offset
fn read_f32_le(data: &[u8], offset: usize) -> Result<f32, Box<dyn std::error::Error + Send + Sync>> {
    if offset + 4 > data.len() {
        return Err(format!("Buffer overflow reading f32 at offset {}", offset).into());
    }
    let bytes: [u8; 4] = data[offset..offset + 4].try_into()?;
    Ok(f32::from_le_bytes(bytes))
}

/// Find a byte subsequence in a slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Find the binary data marker in LTspice raw file
/// Tries multiple formats: UTF-16LE with \n, UTF-16LE with \r\n, UTF-8
fn find_binary_marker(data: &[u8]) -> Option<usize> {
    log::info!("Searching for binary marker in {} bytes of data", data.len());

    // UTF-16LE encoding: each ASCII char becomes 2 bytes (char, 0x00)
    // "Binary:\n" in UTF-16LE = [0x42, 0x00, 0x69, 0x00, 0x6E, 0x00, 0x61, 0x00, 0x72, 0x00, 0x79, 0x00, 0x3A, 0x00, 0x0A, 0x00]
    let marker1_utf16le: Vec<u8> = "Binary:\n".encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    log::info!("Trying UTF-16LE marker: {:?}", marker1_utf16le);
    if let Some(pos) = find_subsequence(data, &marker1_utf16le) {
        log::info!("Found UTF-16LE Binary:\\n at position {}", pos);
        return Some(pos + marker1_utf16le.len());
    }

    // Try UTF-16LE "Binary:\r\n"
    let marker2_utf16le: Vec<u8> = "Binary:\r\n".encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    if let Some(pos) = find_subsequence(data, &marker2_utf16le) {
        log::info!("Found UTF-16LE Binary:\\r\\n at position {}", pos);
        return Some(pos + marker2_utf16le.len());
    }

    // Try UTF-8 "Binary:\n"
    let marker3 = b"Binary:\n";
    if let Some(pos) = find_subsequence(data, marker3) {
        log::info!("Found UTF-8 Binary:\\n at position {}", pos);
        return Some(pos + marker3.len());
    }

    // Try UTF-8 "Binary:\r\n"
    let marker4 = b"Binary:\r\n";
    if let Some(pos) = find_subsequence(data, marker4) {
        log::info!("Found UTF-8 Binary:\\r\\n at position {}", pos);
        return Some(pos + marker4.len());
    }

    log::error!("No binary marker found! First 100 bytes: {:?}", &data[..std::cmp::min(100, data.len())]);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_netlist_adds_backanno() {
        let netlist = "* Test\nV1 in 0 1\nR1 in out 1k\n.tran 1m\n.end";
        let prepared = prepare_netlist(netlist, "balanced");
        assert!(prepared.contains(".backanno"));
    }

    #[test]
    fn test_prepare_netlist_adds_save_all() {
        let netlist = "* Test\nV1 in 0 1\n.tran 1m\n.end";
        let prepared = prepare_netlist(netlist, "balanced");
        assert!(prepared.contains(".save all"));
    }

    #[test]
    fn test_prepare_netlist_does_not_duplicate_backanno() {
        let netlist = "* Test\nV1 in 0 1\n.backanno\n.tran 1m\n.end";
        let prepared = prepare_netlist(netlist, "balanced");
        // Should only have one .backanno
        let count = prepared.matches(".backanno").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_prepare_netlist_does_not_duplicate_save() {
        let netlist = "* Test\nV1 in 0 1\n.save V(out)\n.tran 1m\n.end";
        let prepared = prepare_netlist(netlist, "balanced");
        // Should not add .save all if .save already exists
        assert!(!prepared.contains(".save all"));
    }

    #[test]
    fn test_prepare_netlist_plotwinsize_fast() {
        let netlist = "* Test\nV1 in 0 1\n.tran 1m\n.end";
        let prepared = prepare_netlist(netlist, "fast");
        assert!(prepared.contains(".options plotwinsize=128"));
    }

    #[test]
    fn test_prepare_netlist_plotwinsize_balanced() {
        let netlist = "* Test\nV1 in 0 1\n.tran 1m\n.end";
        let prepared = prepare_netlist(netlist, "balanced");
        assert!(prepared.contains(".options plotwinsize=0"));
    }

    #[test]
    fn test_prepare_netlist_plotwinsize_smooth() {
        let netlist = "* Test\nV1 in 0 1\n.tran 1m\n.end";
        let prepared = prepare_netlist(netlist, "smooth");
        assert!(prepared.contains(".options plotwinsize=0"));
    }

    #[test]
    fn test_prepare_netlist_preserves_content() {
        let netlist = "* My Circuit\nV1 in 0 DC 5\nR1 in out 1k\nC1 out 0 1u\n.tran 10m\n.end";
        let prepared = prepare_netlist(netlist, "balanced");
        assert!(prepared.contains("* My Circuit"));
        assert!(prepared.contains("V1 in 0 DC 5"));
        assert!(prepared.contains("R1 in out 1k"));
        assert!(prepared.contains("C1 out 0 1u"));
        assert!(prepared.contains(".tran 10m"));
        assert!(prepared.contains(".end"));
    }

    #[test]
    fn test_prepare_netlist_inserts_before_end() {
        let netlist = "* Test\nV1 in 0 1\n.tran 1m\n.end";
        let prepared = prepare_netlist(netlist, "balanced");
        let lines: Vec<&str> = prepared.lines().collect();

        // Find positions
        let backanno_pos = lines.iter().position(|l| l.contains(".backanno"));
        let save_pos = lines.iter().position(|l| l.contains(".save all"));
        let end_pos = lines.iter().position(|l| l.trim().to_lowercase() == ".end");

        assert!(backanno_pos.is_some());
        assert!(save_pos.is_some());
        assert!(end_pos.is_some());

        // All directives should be before .end
        assert!(backanno_pos.unwrap() < end_pos.unwrap());
        assert!(save_pos.unwrap() < end_pos.unwrap());
    }

    #[test]
    fn test_prepare_netlist_case_insensitive() {
        // Test with uppercase .END
        let netlist = "* Test\nV1 in 0 1\n.tran 1m\n.END";
        let prepared = prepare_netlist(netlist, "balanced");
        assert!(prepared.contains(".backanno"));
        assert!(prepared.contains(".save all"));
    }

    #[test]
    fn test_find_subsequence_found() {
        let haystack = b"hello world binary data here";
        let needle = b"binary";
        let pos = find_subsequence(haystack, needle);
        assert_eq!(pos, Some(12));
    }

    #[test]
    fn test_find_subsequence_not_found() {
        let haystack = b"hello world";
        let needle = b"xyz";
        let pos = find_subsequence(haystack, needle);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_find_subsequence_at_start() {
        let haystack = b"hello world";
        let needle = b"hello";
        let pos = find_subsequence(haystack, needle);
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn test_find_subsequence_at_end() {
        let haystack = b"hello world";
        let needle = b"world";
        let pos = find_subsequence(haystack, needle);
        assert_eq!(pos, Some(6));
    }

    #[test]
    #[should_panic(expected = "window size must be non-zero")]
    fn test_find_subsequence_empty_needle_panics() {
        // Empty needle causes panic in windows() - this is expected behavior
        let haystack = b"hello";
        let needle: &[u8] = b"";
        let _ = find_subsequence(haystack, needle);
    }

    #[test]
    fn test_find_subsequence_needle_longer_than_haystack() {
        let haystack = b"hi";
        let needle = b"hello world";
        let pos = find_subsequence(haystack, needle);
        assert_eq!(pos, None);
    }

    // Test that the detection function exists and returns Option
    #[test]
    fn test_detect_ltspice_returns_option() {
        // This test just verifies the function exists and returns an Option
        // The actual detection depends on system configuration
        let result = detect_ltspice();
        // Result is either Some(path) or None, both are valid
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn test_prepare_netlist_complex_circuit() {
        let netlist = r#"* WiFi Wakeup Receiver
* Power supply
V1 VCC 0 3.3

* Antenna input (simulated signal)
V2 ANT 0 SINE(0 100m 915Meg)

* Matching network
L1 ANT match1 10n
C1 match1 0 1p

* Detector diode
D1 match1 det DSCHOTTKY
C2 det 0 100p
R1 det 0 1Meg

* Comparator
XU1 det ref wake VCC 0 LTC2063
R2 VCC ref 100k
R3 ref 0 100k

.tran 0 10u 0 1n
.model DSCHOTTKY D(Is=1e-8 Rs=10 N=1.05)
.end"#;

        let prepared = prepare_netlist(netlist, "smooth");

        // Verify original content preserved
        assert!(prepared.contains("* WiFi Wakeup Receiver"));
        assert!(prepared.contains("V1 VCC 0 3.3"));
        assert!(prepared.contains("L1 ANT match1 10n"));
        assert!(prepared.contains("XU1 det ref wake VCC 0 LTC2063"));
        assert!(prepared.contains(".model DSCHOTTKY"));

        // Verify directives added
        assert!(prepared.contains(".backanno"));
        assert!(prepared.contains(".save all"));
        assert!(prepared.contains(".options plotwinsize=0"));
    }

    #[test]
    fn test_prepare_ngspice_netlist_adds_control_section() {
        let netlist = "* Test\nVin in 0 AC 1\nR1 in out 1k\nC1 out 0 100n\n.ac dec 10 1 100k\n.end";
        let raw_path = PathBuf::from("/tmp/test.raw");
        let prepared = prepare_ngspice_netlist(netlist, &raw_path);

        assert!(prepared.contains(".control"));
        assert!(prepared.contains("run"));
        assert!(prepared.contains("write"));
        assert!(prepared.contains(".endc"));
    }

    #[test]
    fn test_prepare_ngspice_netlist_preserves_existing_control() {
        let netlist = "* Test\nVin in 0 AC 1\n.control\nrun\n.endc\n.end";
        let raw_path = PathBuf::from("/tmp/test.raw");
        let prepared = prepare_ngspice_netlist(netlist, &raw_path);

        // Should not add another .control section
        let control_count = prepared.matches(".control").count();
        assert_eq!(control_count, 1);
    }

    #[test]
    fn test_parse_ngspice_raw_file_transient() {
        // Create a mock ngspice ASCII raw file for transient analysis
        let raw_content = r#"Title: * test circuit
Date: Sat Feb  7 12:00:00  2026
Plotname: Transient Analysis
Flags: real
No. Variables: 3
No. Points: 3
Variables:
	0	time	time
	1	v(in)	voltage
	2	v(out)	voltage
Values:
 0	0.000000000000000e+00
	1.000000000000000e+00
	0.000000000000000e+00

 1	1.000000000000000e-03
	1.000000000000000e+00
	5.000000000000000e-01

 2	2.000000000000000e-03
	1.000000000000000e+00
	8.000000000000000e-01
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let raw_path = temp_dir.path().join("test.raw");
        std::fs::write(&raw_path, raw_content).unwrap();

        let results = parse_ngspice_raw_file(&raw_path).unwrap();

        assert_eq!(results.analysis_type, "transient");
        assert_eq!(results.x_axis_label, Some("time".to_string()));
        assert_eq!(results.time.len(), 3);
        assert_eq!(results.traces.len(), 2);

        // Check time values
        assert!((results.time[0] - 0.0).abs() < 1e-10);
        assert!((results.time[1] - 0.001).abs() < 1e-10);
        assert!((results.time[2] - 0.002).abs() < 1e-10);

        // Check v(in) trace
        let v_in = &results.traces[0];
        assert_eq!(v_in.name, "v(in)");
        assert_eq!(v_in.unit, "V");
        assert!((v_in.data[0] - 1.0).abs() < 1e-10);

        // Check v(out) trace
        let v_out = &results.traces[1];
        assert_eq!(v_out.name, "v(out)");
        assert!((v_out.data[0] - 0.0).abs() < 1e-10);
        assert!((v_out.data[1] - 0.5).abs() < 1e-10);
        assert!((v_out.data[2] - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_parse_ngspice_raw_file_ac_complex() {
        // Create a mock ngspice ASCII raw file for AC analysis with complex values
        // Complex values are formatted as "real,imag"
        let raw_content = r#"Title: * ac test circuit
Date: Sat Feb  7 12:00:00  2026
Plotname: AC Analysis
Flags: complex
No. Variables: 3
No. Points: 3
Variables:
	0	frequency	frequency grid=3
	1	v(in)	voltage
	2	v(out)	voltage
Values:
 0	1.000000000000000e+00,0.000000000000000e+00
	1.000000000000000e+00,0.000000000000000e+00
	1.000000000000000e+00,0.000000000000000e+00

 1	1.000000000000000e+01,0.000000000000000e+00
	1.000000000000000e+00,0.000000000000000e+00
	7.071067811865476e-01,-7.071067811865476e-01

 2	1.000000000000000e+02,0.000000000000000e+00
	1.000000000000000e+00,0.000000000000000e+00
	9.950371902099893e-02,-9.950371902099893e-01
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let raw_path = temp_dir.path().join("test_ac.raw");
        std::fs::write(&raw_path, raw_content).unwrap();

        let results = parse_ngspice_raw_file(&raw_path).unwrap();

        assert_eq!(results.analysis_type, "ac");
        assert_eq!(results.x_axis_label, Some("frequency".to_string()));
        assert_eq!(results.time.len(), 3); // "time" field holds frequency for AC
        assert_eq!(results.traces.len(), 2);

        // Check frequency values (stored in "time" field)
        assert!((results.time[0] - 1.0).abs() < 1e-10);
        assert!((results.time[1] - 10.0).abs() < 1e-10);
        assert!((results.time[2] - 100.0).abs() < 1e-10);

        // Check v(in) trace - should be magnitude of (1, 0) = 1
        let v_in = &results.traces[0];
        assert_eq!(v_in.name, "v(in)");
        assert!((v_in.data[0] - 1.0).abs() < 1e-10);
        assert!((v_in.data[1] - 1.0).abs() < 1e-10);
        assert!((v_in.data[2] - 1.0).abs() < 1e-10);

        // Check v(out) trace - magnitudes computed from complex values
        let v_out = &results.traces[1];
        assert_eq!(v_out.name, "v(out)");

        // At 1 Hz: magnitude of (1, 0) = 1
        assert!((v_out.data[0] - 1.0).abs() < 1e-10);

        // At 10 Hz: magnitude of (0.707, -0.707) = sqrt(0.5 + 0.5) = 1
        assert!((v_out.data[1] - 1.0).abs() < 1e-6);

        // At 100 Hz: magnitude of (0.0995, -0.995) = sqrt(0.0099 + 0.990) ≈ 1
        assert!((v_out.data[2] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_parse_ngspice_raw_file_dc_analysis() {
        // Create a mock ngspice ASCII raw file for DC analysis
        let raw_content = r#"Title: * dc test circuit
Date: Sat Feb  7 12:00:00  2026
Plotname: DC transfer characteristic
Flags: real
No. Variables: 2
No. Points: 3
Variables:
	0	v-sweep	voltage
	1	v(out)	voltage
Values:
 0	0.000000000000000e+00
	0.000000000000000e+00

 1	2.500000000000000e+00
	2.500000000000000e+00

 2	5.000000000000000e+00
	5.000000000000000e+00
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let raw_path = temp_dir.path().join("test_dc.raw");
        std::fs::write(&raw_path, raw_content).unwrap();

        let results = parse_ngspice_raw_file(&raw_path).unwrap();

        assert_eq!(results.analysis_type, "dc");
        assert_eq!(results.x_axis_label, Some("v-sweep".to_string()));
        assert_eq!(results.time.len(), 3);
        assert_eq!(results.traces.len(), 1);

        // Check sweep values
        assert!((results.time[0] - 0.0).abs() < 1e-10);
        assert!((results.time[1] - 2.5).abs() < 1e-10);
        assert!((results.time[2] - 5.0).abs() < 1e-10);

        // Check v(out) trace
        let v_out = &results.traces[0];
        assert_eq!(v_out.name, "v(out)");
        assert!((v_out.data[0] - 0.0).abs() < 1e-10);
        assert!((v_out.data[1] - 2.5).abs() < 1e-10);
        assert!((v_out.data[2] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_complex_magnitude_calculation() {
        // Test the magnitude calculation: sqrt(real² + imag²)
        // This verifies the math used in AC analysis parsing

        // Pure real: (3, 0) -> magnitude = 3
        let real = 3.0_f64;
        let imag = 0.0_f64;
        let magnitude = (real * real + imag * imag).sqrt();
        assert!((magnitude - 3.0).abs() < 1e-10);

        // Pure imaginary: (0, 4) -> magnitude = 4
        let real = 0.0_f64;
        let imag = 4.0_f64;
        let magnitude = (real * real + imag * imag).sqrt();
        assert!((magnitude - 4.0).abs() < 1e-10);

        // 3-4-5 triangle: (3, 4) -> magnitude = 5
        let real = 3.0_f64;
        let imag = 4.0_f64;
        let magnitude = (real * real + imag * imag).sqrt();
        assert!((magnitude - 5.0).abs() < 1e-10);

        // Negative values: (-3, -4) -> magnitude = 5
        let real = -3.0_f64;
        let imag = -4.0_f64;
        let magnitude = (real * real + imag * imag).sqrt();
        assert!((magnitude - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_detect_ngspice_returns_option() {
        // This test verifies the function exists and returns an Option
        let result = detect_ngspice();
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn test_extract_ngspice_error_with_line_number() {
        let output = r#"Circuit: * test
Note: some note here
Error on line 14 or its substitute:
    Simulation interrupted due to error!
"#;
        let error = extract_ngspice_error(output);
        assert!(error.is_some());
        let msg = error.unwrap();
        assert!(msg.contains("Error on line 14"));
        assert!(msg.contains("Simulation interrupted"));
    }

    #[test]
    fn test_extract_ngspice_error_no_error() {
        let output = r#"Circuit: * test
Note: no problems
Simulation completed with 0 errors
"#;
        let error = extract_ngspice_error(output);
        assert!(error.is_none());
    }

    #[test]
    fn test_extract_ngspice_error_skips_notes() {
        let output = r#"Note: this is a note
Warning: this is a warning
Error: real error here
"#;
        let error = extract_ngspice_error(output);
        assert!(error.is_some());
        let msg = error.unwrap();
        assert!(msg.contains("real error here"));
        assert!(!msg.contains("this is a note"));
    }

    #[test]
    fn test_extract_ngspice_error_includes_context() {
        let output = r#"line 5:
Error: Unknown device
    Did you mean R1?
"#;
        let error = extract_ngspice_error(output);
        assert!(error.is_some());
        let msg = error.unwrap();
        assert!(msg.contains("Error: Unknown device"));
    }
}
