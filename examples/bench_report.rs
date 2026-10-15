//! Records the latest criterion run of the `proxy` benchmark group.
//!
//! ```text
//! bench_report snapshot <label>  → benches/results/<date>-<label>.json, then re-render benches/BASELINE.md
//! ```
//!
//! The PR gate is the paired mode of `benches/proxy.rs`; snapshots are reference history.
//!
//! Run through `scripts/bench.sh`, which sets `CRITERION_HOME` and the `BENCH_*` metadata.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};

const GROUP: &str = "proxy";
/// Display order; scenarios not listed here are appended alphabetically.
const SCENARIOS: &[&str] = &[
    "target_cold_ttfb",
    "target_warm_ttfb",
    "direct_ttfb",
    "burst_cold_20",
    "first_request_after_start",
    "throughput_down_64m",
    "throughput_bidir_32m",
];
const MARK_START: &str = "<!-- results:start -->";
const MARK_END: &str = "<!-- results:end -->";

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["snapshot", label] => snapshot(label),
        _ => {
            eprintln!("usage: bench_report snapshot <label>");
            ExitCode::from(2)
        }
    }
}

fn criterion_home() -> PathBuf {
    env::var_os("CRITERION_HOME").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/criterion"),
        PathBuf::from,
    )
}

fn results_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/results")
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Scenario directories present in the latest (`new`) run, in display order.
fn scenarios(home: &Path) -> Vec<String> {
    let mut found = fs::read_dir(home.join(GROUP))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().join("new/estimates.json").exists())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    found.sort_by_key(|name| {
        (
            SCENARIOS
                .iter()
                .position(|known| known == name)
                .unwrap_or(usize::MAX),
            name.clone(),
        )
    });
    found
}

struct Estimate {
    median_ns: f64,
    ci95_ns: (f64, f64),
    throughput_bytes: Option<u64>,
}

fn estimate(home: &Path, scenario: &str, run: &str) -> Option<Estimate> {
    let dir = home.join(GROUP).join(scenario).join(run);
    let estimates = read_json(&dir.join("estimates.json"))?;
    let median = &estimates["median"];
    let throughput_bytes = read_json(&home.join(GROUP).join(scenario).join("new/benchmark.json"))
        .and_then(|benchmark| benchmark["throughput"]["Bytes"].as_u64());
    Some(Estimate {
        median_ns: median["point_estimate"].as_f64()?,
        ci95_ns: (
            median["confidence_interval"]["lower_bound"].as_f64()?,
            median["confidence_interval"]["upper_bound"].as_f64()?,
        ),
        throughput_bytes,
    })
}

fn extras(home: &Path, run: &str) -> Value {
    read_json(&home.join("extras").join(format!("{run}.json"))).unwrap_or_else(|| json!({}))
}

fn format_time(ns: f64) -> String {
    if ns >= 1e9 {
        format!("{:.2} s", ns / 1e9)
    } else if ns >= 1e6 {
        format!("{:.2} ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.1} µs", ns / 1e3)
    } else {
        format!("{ns:.0} ns")
    }
}

fn mib_per_s(bytes: u64, ns: f64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0) / (ns / 1e9)
}

fn metadata(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| "unknown".to_owned())
}

fn snapshot(label: &str) -> ExitCode {
    let home = criterion_home();
    let mut results = serde_json::Map::new();
    for scenario in scenarios(&home) {
        let Some(estimate) = estimate(&home, &scenario, "new") else {
            continue;
        };
        let mut entry = json!({
            "median_ns": estimate.median_ns.round(),
            "ci95_ns": [estimate.ci95_ns.0.round(), estimate.ci95_ns.1.round()],
        });
        if let Some(bytes) = estimate.throughput_bytes {
            entry["mib_per_s"] =
                json!((mib_per_s(bytes, estimate.median_ns) * 10.0).round() / 10.0);
        }
        results.insert(scenario, entry);
    }
    if let Some(extras) = extras(&home, "new").as_object() {
        for (scenario, metrics) in extras {
            if let (Some(Value::Object(entry)), Some(metrics)) =
                (results.get_mut(scenario), metrics.as_object())
            {
                entry.extend(metrics.clone());
            }
        }
    }
    if results.is_empty() {
        eprintln!("no criterion results under {}", home.display());
        return ExitCode::FAILURE;
    }

    let date = metadata("BENCH_DATE");
    let snapshot = json!({
        "label": label,
        "commit": metadata("BENCH_COMMIT"),
        "date": date,
        "machine": {
            "cpu": metadata("BENCH_CPU"),
            "os": metadata("BENCH_OS"),
            "rustc": metadata("BENCH_RUSTC"),
        },
        "results": results,
    });
    let dir = results_dir();
    fs::create_dir_all(&dir).expect("create results directory");
    let path = dir.join(format!("{date}-{label}.json"));
    fs::write(
        &path,
        serde_json::to_string_pretty(&snapshot).expect("encode snapshot") + "\n",
    )
    .expect("write snapshot");
    println!("wrote {}", path.display());
    render_baseline_md(&dir);
    ExitCode::SUCCESS
}

/// Rewrites the generated table in `benches/BASELINE.md`: one column per snapshot.
fn render_baseline_md(dir: &Path) {
    let mut snapshots = fs::read_dir(dir)
        .expect("read results directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    snapshots.sort();
    let snapshots = snapshots
        .iter()
        .filter_map(|path| read_json(path))
        .collect::<Vec<_>>();

    let mut rows: BTreeMap<(usize, String), Vec<String>> = BTreeMap::new();
    for (column, snapshot) in snapshots.iter().enumerate() {
        for (scenario, result) in snapshot["results"].as_object().into_iter().flatten() {
            let order = SCENARIOS
                .iter()
                .position(|known| known == scenario)
                .unwrap_or(usize::MAX);
            let mut cell = format_time(result["median_ns"].as_f64().unwrap_or(f64::NAN));
            if let Some(rate) = result["mib_per_s"].as_f64() {
                cell = format!("{rate:.0} MiB/s");
            }
            if let Some(queries) = result["doh_queries_per_burst"].as_f64() {
                cell = format!("{cell} · {queries:.0} DoH q");
            }
            rows.entry((order, scenario.clone()))
                .or_insert_with(|| vec!["–".to_owned(); snapshots.len()])[column] = cell;
        }
    }

    let mut table = String::new();
    table.push_str("| Scenario |");
    for snapshot in &snapshots {
        table.push_str(&format!(" {} |", snapshot["label"].as_str().unwrap_or("?")));
    }
    table.push_str("\n|---|");
    table.push_str(&"---|".repeat(snapshots.len()));
    table.push('\n');
    for ((_, scenario), cells) in &rows {
        table.push_str(&format!("| `{scenario}` | {} |\n", cells.join(" | ")));
    }
    table
        .push_str("\n| Snapshot | Commit | Date | CPU | OS | rustc |\n|---|---|---|---|---|---|\n");
    for snapshot in &snapshots {
        table.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} |\n",
            snapshot["label"].as_str().unwrap_or("?"),
            snapshot["commit"].as_str().unwrap_or("?"),
            snapshot["date"].as_str().unwrap_or("?"),
            snapshot["machine"]["cpu"].as_str().unwrap_or("?"),
            snapshot["machine"]["os"].as_str().unwrap_or("?"),
            snapshot["machine"]["rustc"].as_str().unwrap_or("?"),
        ));
    }

    let path = dir.parent().expect("benches directory").join("BASELINE.md");
    let current = fs::read_to_string(&path).unwrap_or_default();
    let rendered = match (current.find(MARK_START), current.find(MARK_END)) {
        (Some(start), Some(end)) if start < end => format!(
            "{}{MARK_START}\n{table}{}",
            &current[..start],
            &current[end..]
        ),
        _ => format!("{current}\n{MARK_START}\n{table}{MARK_END}\n"),
    };
    fs::write(&path, rendered).expect("write BASELINE.md");
    println!("updated {}", path.display());
}
