//! `wrap-tool-cell serve <tool>`: the `wrap.<tool>@1` cell behind this machine's door, gated by ce-cap (default deny).
//! `wrap-tool-cell parity --on NODE`: every parity case run plainly here and through the cell on NODE, as a table.
use wrap_tool::*;

mod parity;

macro_rules! serve_tool {
    ($m:ident, $Api:ident, $w:expr, $me:expr) => {{
        use ce_wire::blocking::Server;
        use wrap_tool_api::$m::*;
        let w = std::sync::Arc::new($w);
        let started = std::time::Instant::now();
        let gate = ce_gate::Gate::<$Api>::new($me.node_id(), ce_gate::mesh_roots()).into_fn();
        let tool = |i: &ToolInfo| ToolId { name: i.name.clone(), version: i.version.clone(), artifact_sha256: i.artifact_sha256.clone(), os_arch: i.os_arch.clone(), node: i.node.clone() };
        let (w1, w2) = (w.clone(), w.clone());
        Server::<$Api>::new(gate)
            .on::<HealthOp, _>(move |_: (), _| Ok(Health { ok: true, version: env!("CARGO_PKG_VERSION").into(), uptime_s: started.elapsed().as_secs() }))
            .on::<Run, _>(move |r: RunReq, _| {
                Ok(match w1.run(&r.args, &r.input, &r.in_ext, &r.out_ext) {
                    Ok((plan, o)) => Ok(RunResp {
                        exit: o.exit,
                        signal: o.signal.clone(),
                        stdout_sha256: sha256_hex(&o.stdout),
                        stderr_sha256: sha256_hex(&o.stderr),
                        output_cid: o.output_cid(),
                        stdout: o.stdout,
                        stderr: o.stderr,
                        output: o.output,
                        argv: plan.argv,
                        tool: tool(&w1.info),
                        ms: o.ms,
                    }),
                    Err(WrapError::Invalid(s)) => Err(RunError::Invalid(s)),
                    Err(WrapError::Limited(s)) => Err(RunError::Limited(s)),
                    Err(WrapError::Failed(s)) => Err(RunError::Failed(s)),
                })
            })
            .on::<Describe, _>(move |_: (), _| {
                let p = &w2.policy;
                Ok(Policy {
                    api: p.api.clone(),
                    license: p.license.clone(),
                    prefix: p.prefix.clone(),
                    flags: p.flags.iter().map(|(k, v)| Flag { name: k.clone(), value: v.clone() }).collect(),
                    positional: p.positional.clone(),
                    in_ext: p.in_ext.clone(),
                    out_ext: p.out_ext.clone(),
                    max_in_bytes: p.max_in_bytes,
                    max_out_bytes: p.max_out_bytes,
                    max_stdio_bytes: p.max_stdio_bytes,
                    timeout_ms: p.timeout_ms,
                    mem_mb: p.mem_mb,
                    concurrency: p.concurrency,
                    tool: tool(&w2.info),
                })
            })
            .serve_door()
            .map_err(|e| e.to_string())
    }};
}

fn node_short(id: String) -> String {
    id.chars().take(16).collect()
}

fn serve(tool: &str) -> Result<(), String> {
    let me = ce_gate::node_identity().map_err(|e| e.to_string())?;
    let w = Wrapped::new(tool, &node_short(hex(&me.node_id())))?;
    eprintln!("wrap.{tool}: {} {} sha256 {} ({})", w.info.name, w.info.version, w.info.artifact_sha256, w.info.os_arch);
    match tool {
        "convert" => serve_tool!(convert, WrapConvert, w, me),
        "pandoc" => serve_tool!(pandoc, WrapPandoc, w, me),
        "jq" => serve_tool!(jq, WrapJq, w, me),
        t => Err(format!("{t}: not a wrapped tool ({})", TOOLS.join(", "))),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let tool = a.get(1).cloned().unwrap_or_default();
    let r: Result<i32, String> = match a.first().map(String::as_str) {
        Some("serve") => serve(&tool).map(|_| 0),
        Some("parity") => parity::main(&a[1..]).map(|_| 0),
        _ => Err("usage: wrap-tool-cell serve <convert|pandoc|jq> | parity --on NODE [--tools a,b] [--json FILE] [--md FILE] [--fixtures DIR] [--sdk DIR]".into()),
    };
    match r {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("wrap-tool-cell: {e}");
            std::process::exit(1);
        }
    }
}
