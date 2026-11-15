//! `wrap-tool local <tool> --in FILE [--out-ext E] [--out FILE] -- ARGS`: the argv a `wrap.<tool>@1` cell runs,
//! run plainly on this machine, with the hashes the cell reports, so anyone can check a wrapped result locally.
//! `wrap-tool policy <tool>`: the allowlist and limits.
use std::io::Write;
use wrap_tool::*;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn local(tool: &str, rest: &[String]) -> Result<i32, String> {
    let split = rest.iter().position(|a| a == "--").unwrap_or(rest.len());
    let (opts, args) = (&rest[..split], rest.get(split + 1..).unwrap_or(&[]));
    let inp = flag(opts, "--in").ok_or("--in FILE")?;
    let input = std::fs::read(&inp).map_err(|e| format!("{inp}: {e}"))?;
    let in_ext = std::path::Path::new(&inp).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let in_ext = if in_ext == "jpeg" { "jpg".to_string() } else { in_ext };
    let out_ext = flag(opts, "--out-ext").unwrap_or_default();
    let p = Policy::load(tool)?;
    let exe = find_exe(&p).ok_or_else(|| format!("{} not installed", p.bin[0]))?;
    let plan = plan(&p, args, &input, &in_ext, &out_ext).map_err(|e| e.to_string())?;
    let o = exec(&p, &exe, &plan, &input, false).map_err(|e| e.to_string())?;
    std::io::stdout().write_all(&o.stdout).ok();
    std::io::stderr().write_all(&o.stderr).ok();
    if let Some(f) = flag(opts, "--out") {
        std::fs::write(&f, &o.output).map_err(|e| format!("{f}: {e}"))?;
    }
    let info = tool_info(&p, &exe, "local");
    eprintln!("# argv {:?}\n# stdout sha256 {}\n# stderr sha256 {}\n# exit {}\n# output {}\n# artifact sha256 {} {} ({})", plan.argv, sha256_hex(&o.stdout), sha256_hex(&o.stderr), o.exit, o.output_cid(), info.artifact_sha256, info.version, info.os_arch);
    Ok(o.exit)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let tool = a.get(1).cloned().unwrap_or_default();
    let r: Result<i32, String> = match a.first().map(String::as_str) {
        Some("local") => local(&tool, &a[2.min(a.len())..]),
        Some("policy") => Policy::load(&tool).map(|p| {
            println!("{p:#?}");
            0
        }),
        _ => Err("usage: wrap-tool local <convert|pandoc|jq> --in FILE [--out-ext E] [--out FILE] -- ARGS | policy <tool>".into()),
    };
    match r {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("wrap-tool: {e}");
            std::process::exit(1);
        }
    }
}
