mod correctness;
mod measure;

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use serde_json::Value;

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "usage: cpc-eval-harness correctness|oracle|ingest|query|scan|dedup --key value"
        );
        return ExitCode::from(2);
    }
    let cmd = args.remove(0);
    let result = match cmd.as_str() {
        "correctness" => {
            let work = Path::new(flag(&args, "--work").unwrap_or("eval/work/correctness"));
            let out = flag(&args, "--out");
            let value = correctness::run(work);
            if let Some(path) = out {
                write_json(Path::new(path), &value);
            }
            Ok(value)
        }
        "oracle" => {
            let chunks = req(&args, "--chunks");
            let all = req(&args, "--all");
            let out = req(&args, "--out");
            measure::oracle(Path::new(chunks), Path::new(all)).map(|v| {
                write_json(Path::new(out), &v);
                v
            })
        }
        "ingest" => {
            let con = req(&args, "--con");
            let dest = req(&args, "--dest");
            let oracle = read_json(Path::new(req(&args, "--oracle")));
            measure::ingest(Path::new(con), Path::new(dest), &oracle)
        }
        "query" => {
            let corpus = req(&args, "--corpus");
            let oracle = read_json(Path::new(req(&args, "--oracle")));
            measure::query(Path::new(corpus), &oracle, flag(&args, "--only"))
        }
        "scan" => {
            let chunks = req(&args, "--chunks");
            let oracle = read_json(Path::new(req(&args, "--oracle")));
            measure::scan(Path::new(chunks), &oracle, flag(&args, "--only"))
        }
        "dedup" => {
            let con = req(&args, "--con");
            let dest = req(&args, "--dest");
            measure::dedup(Path::new(con), Path::new(dest))
        }
        other => Err(format!("unknown command {other}")),
    };
    match result {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            let msg = scrub(&e);
            println!("{{\"ok\":false,\"error\":{}}}", serde_json::to_string(&msg).unwrap());
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).map(|s| s.as_str()))
}

fn req<'a>(args: &'a [String], name: &str) -> &'a str {
    flag(args, name).unwrap_or_else(|| {
        eprintln!("missing {name}");
        std::process::exit(2);
    })
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("read json: {}", scrub(&e.to_string()));
        std::process::exit(2);
    });
    serde_json::from_str(&text).unwrap_or_else(|e| {
        eprintln!("parse json: {e}");
        std::process::exit(2);
    })
}

fn write_json(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap_or_else(|e| {
        eprintln!("write json: {}", scrub(&e.to_string()));
        std::process::exit(1);
    });
}

fn scrub(s: &str) -> String {
    s.split_whitespace()
        .map(|w| if w.starts_with('/') { "PATH" } else { w })
        .collect::<Vec<_>>()
        .join(" ")
}
