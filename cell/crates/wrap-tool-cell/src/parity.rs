//! The parity test: every case runs twice, once plainly on this machine (`exec` without limits) and once through
//! the `wrap.<tool>@1` cell on `--on NODE` over the mesh (`ce cell call`). It compares stdout, stderr and output by
//! sha256 and the exit code, and writes the table as JSON and Markdown.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use wrap_tool::*;

pub struct Case {
    pub tool: &'static str,
    pub label: &'static str,
    pub fixture: &'static str,
    pub in_ext: &'static str,
    pub out_ext: &'static str,
    pub args: &'static [&'static str],
}

pub const CASES: &[Case] = &[
    Case { tool: "convert", label: "resize 50%", fixture: "ducks.png", in_ext: "png", out_ext: "png", args: &["{in}", "-resize", "50%", "-strip", "{out}"] },
    Case { tool: "convert", label: "grayscale to JPEG q80", fixture: "ducks.png", in_ext: "png", out_ext: "jpg", args: &["{in}", "-colorspace", "Gray", "-quality", "80", "-strip", "{out}"] },
    Case { tool: "convert", label: "rotate 90, WebP", fixture: "ducks.png", in_ext: "png", out_ext: "webp", args: &["{in}", "-rotate", "90", "-strip", "{out}"] },
    Case { tool: "convert", label: "truncated PNG (error path)", fixture: "broken.png", in_ext: "png", out_ext: "png", args: &["{in}", "{out}"] },
    Case { tool: "pandoc", label: "Markdown to HTML (stdout)", fixture: "notes.md", in_ext: "md", out_ext: "", args: &["{in}", "-f", "gfm", "-t", "html"] },
    Case { tool: "pandoc", label: "Markdown to reStructuredText", fixture: "notes.md", in_ext: "md", out_ext: "rst", args: &["{in}", "-f", "markdown", "-t", "rst", "-o", "{out}"] },
    Case { tool: "jq", label: "filter", fixture: "ducks.json", in_ext: "json", out_ext: "", args: &["-c", "[.ducks[] | select(.age > 1) | .name]", "{in}"] },
    Case { tool: "jq", label: "invalid JSON (error path)", fixture: "broken.json", in_ext: "json", out_ext: "", args: &[".", "{in}"] },
];

fn bytes(v: &Value) -> Vec<u8> {
    match v {
        Value::Array(a) => a.iter().filter_map(|x| x.as_u64()).map(|x| x as u8).collect(),
        Value::String(s) => s.as_bytes().to_vec(),
        _ => vec![],
    }
}

fn ce() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let p = format!("{home}/.local/bin/ce");
    if Path::new(&p).exists() {
        p
    } else {
        "ce".into()
    }
}

/// One call through the cell. Returns the RunResp as JSON.
pub fn wrapped(node: &str, sdk: &Path, c: &Case, input: &[u8]) -> Result<Value, String> {
    let req = json!({ "args": c.args, "input": input, "in_ext": c.in_ext, "out_ext": c.out_ext });
    let tmp = std::env::temp_dir().join(format!("wrap-parity-{}.json", std::process::id()));
    std::fs::write(&tmp, req.to_string()).map_err(|e| e.to_string())?;
    let desc = sdk.join(format!("wrap.{}.json", c.tool));
    let out = Command::new(ce())
        .args(["cell", "call", &format!("wrap.{}", c.tool), "Run", "-", "--on", node, "--descriptor"])
        .arg(&desc)
        .stdin(std::fs::File::open(&tmp).map_err(|e| e.to_string())?)
        .output()
        .map_err(|e| format!("ce: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        return Err(format!("ce cell call: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)));
    }
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("ce output: {e}: {}", String::from_utf8_lossy(&out.stdout)))?;
    Ok(match v.get("Ok") {
        Some(ok) => ok.clone(),
        None => v,
    })
}

pub fn main(a: &[String]) -> Result<(), String> {
    let get = |n: &str| a.iter().position(|x| x == n).and_then(|i| a.get(i + 1).cloned());
    let node = get("--on").ok_or("--on NODE")?;
    let fixtures = PathBuf::from(get("--fixtures").unwrap_or_else(|| "tests/fixtures".into()));
    let sdk = PathBuf::from(get("--sdk").unwrap_or_else(|| "sdk/json".into()));
    let me = ce_gate::node_identity().map(|i| hex(&i.node_id()).chars().take(16).collect::<String>()).unwrap_or_default();
    let tools: Vec<String> = get("--tools").map(|t| t.split(',').map(String::from).collect()).unwrap_or_default();
    let mut rows = vec![];
    let mut all = true;
    for c in CASES.iter().filter(|c| tools.is_empty() || tools.iter().any(|t| t == c.tool)) {
        let input = std::fs::read(fixtures.join(c.fixture)).map_err(|e| format!("{}: {e}", c.fixture))?;
        let p = Policy::load(c.tool)?;
        let Some(exe) = find_exe(&p) else {
            eprintln!("skip {}: {} is not installed here", c.label, c.tool);
            continue;
        };
        let args: Vec<String> = c.args.iter().map(|s| s.to_string()).collect();
        let plan = plan(&p, &args, &input, c.in_ext, c.out_ext).map_err(|e| e.to_string())?;
        let l = exec(&p, &exe, &plan, &input, false).map_err(|e| e.to_string())?;
        let li = tool_info(&p, &exe, &me);
        let w = match wrapped(&node, &sdk, c, &input) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("{}: {e}", c.label);
                all = false;
                continue;
            }
        };
        let w_stdout = sha256_hex(&bytes(&w["stdout"]));
        let w_stderr = sha256_hex(&bytes(&w["stderr"]));
        let w_out = bytes(&w["output"]);
        let w_cid = if w_out.is_empty() { String::new() } else { format!("sha256:{}", sha256_hex(&w_out)) };
        let row = json!({
            "tool": c.tool, "case": c.label, "argv": plan.argv,
            "local": { "stdout_sha256": sha256_hex(&l.stdout), "stderr_sha256": sha256_hex(&l.stderr), "exit": l.exit, "output_cid": l.output_cid(),
                       "artifact_sha256": li.artifact_sha256, "version": li.version, "os_arch": li.os_arch, "node": me },
            "wrapped": { "stdout_sha256": w_stdout, "stderr_sha256": w_stderr, "exit": w["exit"], "output_cid": w_cid, "cid_reported": w["output_cid"],
                         "artifact_sha256": w["tool"]["artifact_sha256"], "version": w["tool"]["version"], "os_arch": w["tool"]["os_arch"], "node": w["tool"]["node"], "ms": w["ms"] },
            "same_stdout": sha256_hex(&l.stdout) == w_stdout,
            "same_stderr": sha256_hex(&l.stderr) == w_stderr,
            "same_exit": w["exit"].as_i64() == Some(l.exit as i64),
            "same_output": l.output_cid() == w_cid,
            "argv_match": w["argv"] == json!(plan.argv),
        });
        let ok = ["same_stdout", "same_stderr", "same_exit", "same_output", "argv_match"].iter().all(|k| row[k] == json!(true));
        all &= ok;
        eprintln!("{:8} {:32} exit {}/{} stdout {} stderr {} output {}", c.tool, c.label, l.exit, w["exit"], row["same_stdout"], row["same_stderr"], row["same_output"]);
        rows.push(row);
    }
    let doc = json!({ "caller": me, "on": node, "rows": rows, "all_match": all });
    if let Some(f) = get("--json") {
        std::fs::write(&f, serde_json::to_string_pretty(&doc).unwrap()).map_err(|e| format!("{f}: {e}"))?;
    }
    if let Some(f) = get("--md") {
        let mut md = String::from("| tool | case | exit local / wrapped | stdout sha256 local / wrapped | stderr sha256 local / wrapped | output local / wrapped | match |\n|---|---|---|---|---|---|---|\n");
        let s = |v: &Value| v.as_str().unwrap_or("").chars().take(12).collect::<String>();
        for r in doc["rows"].as_array().unwrap() {
            let cid = |v: &Value| v.as_str().unwrap_or("").trim_start_matches("sha256:").chars().take(12).collect::<String>();
            let ok = ["same_stdout", "same_stderr", "same_exit", "same_output"].iter().all(|k| r[k] == json!(true));
            md += &format!(
                "| {} | {} | {} / {} | {} / {} | {} / {} | {} / {} | {} |\n",
                r["tool"].as_str().unwrap(), r["case"].as_str().unwrap(), r["local"]["exit"], r["wrapped"]["exit"],
                s(&r["local"]["stdout_sha256"]), s(&r["wrapped"]["stdout_sha256"]), s(&r["local"]["stderr_sha256"]), s(&r["wrapped"]["stderr_sha256"]),
                cid(&r["local"]["output_cid"]), cid(&r["wrapped"]["output_cid"]), if ok { "yes" } else { "NO" }
            );
        }
        std::fs::write(&f, md).map_err(|e| format!("{f}: {e}"))?;
    }
    if all {
        Ok(())
    } else {
        Err("parity: at least one case differs or failed".into())
    }
}
