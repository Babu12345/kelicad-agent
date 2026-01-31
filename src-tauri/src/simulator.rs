// Copyright (c) 2024-2025 Wanyeki Technologies LLC. All rights reserved.
// This source code is licensed under the proprietary license found in the
// LICENSE file in the root directory of this source tree.

//! LTspice simulation execution and result parsing

use std::path::PathBuf;
use std::process::Command;
use encoding_rs::UTF_16LE;
use regex::Regex;
use tempfile::Builder;

use crate::protocol::{SimulationResults, Trace};

/// Standard libraries bundled with the agent
const STANDARD_LIBRARIES: &[&str] = &["LTC3.lib"];

/// Known LTspice installation paths on Windows
#[cfg(windows)]
const LTSPICE_PATHS_WINDOWS: &[&str] = &[
    r"C:\Program Files\LTC\LTspiceXVII\XVIIx64.exe",
    r"C:\Program Files\LTC\LTspice\LTspice.exe",
    r"C:\Program Files (x86)\LTC\LTspiceXVII\XVIIx86.exe",
    r"C:\Program Files (x86)\LTC\LTspice\LTspice.exe",
];

/// Known LTspice installation paths on macOS
#[cfg(target_os = "macos")]
const LTSPICE_PATHS_MACOS: &[&str] = &[
    "/Applications/LTspice.app/Contents/MacOS/LTspice",
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
/// Copies standard library files to the temp directory and updates paths
fn process_includes(
    netlist: &str,
    temp_dir: &std::path::Path,
) -> Result<(String, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
    let mut processed_netlist = netlist.to_string();
    let mut copied_files: Vec<String> = Vec::new();

    // Match .include or .lib directives
    let include_pattern = Regex::new(r#"(?im)^\s*\.(?:include|lib)\s+(.+?)\s*$"#)?;

    let resources_dir = get_resources_dir();

    for cap in include_pattern.captures_iter(netlist) {
        let full_match = cap.get(0).unwrap().as_str();
        let path_str = cap.get(1).unwrap().as_str().trim_matches(|c| c == '"' || c == '\'');

        // Extract filename from path
        let file_name = std::path::Path::new(path_str)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path_str);

        // Check if this is a standard library we bundle
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

                    log::info!("Copied standard library: {} -> {:?}", file_name, dest_path);
                } else {
                    log::warn!("Standard library not found: {:?}", src_path);
                }
            } else {
                log::warn!("Resources directory not found, cannot copy library: {}", file_name);
            }
        }
    }

    Ok((processed_netlist, copied_files))
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

    Ok(SimulationResults {
        time,
        traces,
        analysis_type: analysis_type.to_string(),
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
}
