//! wrap-tool: one command-line program behind a narrow argument allowlist, with size, time and memory limits.
//!
//! The same code builds the argv for the cell (`wrap-tool serve <tool>`) and for a plain local run
//! (`wrap-tool local <tool>`), so the parity test compares the program, not two ways of calling it.
//! A policy (`tools/<tool>.toml`) is data: the flags it allows and the value each takes, the input and output
//! extensions, the limits, and the environment the program gets (nothing else from the host).
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const TOOLS: &[&str] = &["convert", "pandoc", "jq"];

/// The built-in policies; `WRAP_TOOL_POLICY_DIR/<tool>.toml` overrides one.
pub fn builtin_policy(tool: &str) -> Option<&'static str> {
    match tool {
        "convert" => Some(include_str!("../../../tools/convert.toml")),
        "pandoc" => Some(include_str!("../../../tools/pandoc.toml")),
        "jq" => Some(include_str!("../../../tools/jq.toml")),
        _ => None,
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Policy {
    pub api: String,
    pub license: String,
    pub bin: Vec<String>,
    pub version_args: Vec<String>,
    pub prefix: Vec<String>,
    pub in_ext: Vec<String>,
    pub out_ext: Vec<String>,
    pub in_magic: bool,
    pub coder_prefix: bool,
    pub out_required: bool,
    pub positional: String,
    pub max_in_bytes: u64,
    pub max_out_bytes: u64,
    pub max_stdio_bytes: u64,
    pub timeout_ms: u64,
    pub cpu_s: u64,
    pub mem_mb: u64,
    pub max_args: usize,
    pub concurrency: u32,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub flags: BTreeMap<String, String>,
}

impl Policy {
    pub fn load(tool: &str) -> Result<Policy, String> {
        let text = match std::env::var_os("WRAP_TOOL_POLICY_DIR").map(|d| PathBuf::from(d).join(format!("{tool}.toml"))) {
            Some(p) if p.exists() => std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?,
            _ => builtin_policy(tool).ok_or_else(|| format!("{tool}: not a wrapped tool ({})", TOOLS.join(", ")))?.to_string(),
        };
        toml::from_str(&text).map_err(|e| format!("{tool} policy: {e}"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WrapError {
    Invalid(String),
    Limited(String),
    Failed(String),
}

impl std::fmt::Display for WrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrapError::Invalid(s) => write!(f, "invalid: {s}"),
            WrapError::Limited(s) => write!(f, "limited: {s}"),
            WrapError::Failed(s) => write!(f, "failed: {s}"),
        }
    }
}

use WrapError::*;

// ---------------------------------------------------------------- argument allowlist

fn ext_ok(list: &[String], e: &str) -> bool {
    list.iter().any(|x| x == e)
}

/// The file type the bytes say they are, for the image coders.
pub fn sniff(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("gif")
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else if b.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

fn coder(ext: &str) -> &str {
    match ext {
        "jpg" => "jpeg",
        e => e,
    }
}

/// Check one flag value against its kind (`none`, `out`, `geometry`, `int:lo:hi`, `num:lo:hi`, `enum:a|b`,
/// `text:N:deny|deny`).
pub fn value_ok(kind: &str, v: &str) -> Result<(), String> {
    let mut parts = kind.splitn(3, ':');
    let k = parts.next().unwrap_or("");
    let a = parts.next().unwrap_or("");
    let b = parts.next().unwrap_or("");
    let bad = |why: &str| Err(format!("value {v:?} is not {kind} ({why})"));
    match k {
        "geometry" => {
            if v.is_empty() || v.len() > 24 || !v.chars().all(|c| "0123456789x%!<>^@.+-,".contains(c)) || !v.chars().any(|c| c.is_ascii_digit()) {
                return bad("digits and x % ! < > ^ @ . + - , only");
            }
            Ok(())
        }
        "int" => {
            let Ok(n) = v.parse::<i64>() else { return bad("an integer") };
            let (lo, hi): (i64, i64) = (a.parse().unwrap_or(i64::MIN), b.parse().unwrap_or(i64::MAX));
            if n < lo || n > hi {
                return bad("out of range");
            }
            Ok(())
        }
        "num" => {
            let Ok(n) = v.parse::<f64>() else { return bad("a number") };
            let (lo, hi): (f64, f64) = (a.parse().unwrap_or(f64::MIN), b.parse().unwrap_or(f64::MAX));
            if !n.is_finite() || n < lo || n > hi || v.len() > 16 {
                return bad("out of range");
            }
            Ok(())
        }
        "enum" => {
            let all: Vec<&str> = kind["enum:".len()..].split('|').collect();
            if !all.contains(&v) {
                return bad("not one of the listed values");
            }
            Ok(())
        }
        "text" => {
            let max: usize = a.parse().unwrap_or(0);
            if v.is_empty() || v.chars().count() > max || v.chars().any(|c| c.is_control()) {
                return bad("printable, length-limited");
            }
            if let Some(w) = b.split('|').filter(|w| !w.is_empty()).find(|w| v.contains(*w)) {
                return Err(format!("value may not contain {w:?}"));
            }
            Ok(())
        }
        _ => Err(format!("policy kind {kind:?} unknown")),
    }
}

/// What will run: the argv (program name first, as shown to callers), the input file name, the output file name.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub argv: Vec<String>,
    pub in_file: String,
    pub out_file: Option<String>,
}

/// Validate a request and build the argv. `{in}` is the input file, `{out}` the output file.
pub fn plan(p: &Policy, args: &[String], input: &[u8], in_ext: &str, out_ext: &str) -> Result<Plan, WrapError> {
    if args.len() > p.max_args {
        return Err(Invalid(format!("{} arguments; at most {}", args.len(), p.max_args)));
    }
    if input.len() as u64 > p.max_in_bytes {
        return Err(Limited(format!("input is {} bytes; at most {}", input.len(), p.max_in_bytes)));
    }
    if !ext_ok(&p.in_ext, in_ext) {
        return Err(Invalid(format!("input type {in_ext:?}; allowed: {}", p.in_ext.join(", "))));
    }
    if p.in_magic {
        match sniff(input) {
            Some(s) if s == in_ext => {}
            Some(s) => return Err(Invalid(format!("the bytes are {s}, not {in_ext}"))),
            None => return Err(Invalid(format!("the bytes are not a {} file", p.in_ext.join("/")))),
        }
    }
    let in_file = format!("in.{in_ext}");
    let in_arg = if p.coder_prefix { format!("{}:{in_file}", coder(in_ext)) } else { in_file.clone() };
    let out_file = if out_ext.is_empty() { None } else { Some(format!("out.{out_ext}")) };
    let mut argv = vec![p.bin[0].clone()];
    argv.extend(p.prefix.iter().cloned());
    let (mut n_in, mut n_out, mut n_pos) = (0, 0, 0);
    let mut i = 0;
    while i < args.len() {
        let t = &args[i];
        if t.contains('\0') {
            return Err(Invalid("NUL in an argument".into()));
        }
        if t == "{in}" {
            n_in += 1;
            argv.push(in_arg.clone());
        } else if t == "{out}" {
            n_out += 1;
            argv.push(out_file.clone().ok_or_else(|| Invalid("{out} needs out_ext".into()))?);
        } else if let Some(kind) = p.flags.get(t.as_str()) {
            argv.push(t.clone());
            match kind.as_str() {
                "none" => {}
                "out" => {
                    if args.get(i + 1).map(String::as_str) != Some("{out}") {
                        return Err(Invalid(format!("{t} takes {{out}} only")));
                    }
                }
                k => {
                    let v = args.get(i + 1).ok_or_else(|| Invalid(format!("{t} needs a value")))?;
                    value_ok(k, v).map_err(|e| Invalid(format!("{t}: {e}")))?;
                    argv.push(v.clone());
                    i += 1;
                }
            }
        } else if !p.positional.is_empty() && n_pos == 0 && !t.starts_with('-') {
            value_ok(&p.positional, t).map_err(|e| Invalid(format!("argument: {e}")))?;
            n_pos += 1;
            argv.push(t.clone());
        } else {
            return Err(Invalid(format!("{t:?} is not on the {} allowlist", p.api)));
        }
        i += 1;
    }
    if n_in != 1 {
        return Err(Invalid("{in} must appear exactly once".into()));
    }
    if n_out > 1 || (p.out_required && n_out != 1) {
        return Err(Invalid(if p.out_required { "{out} must appear exactly once".into() } else { "{out} at most once".into() }));
    }
    if n_out == 1 && !ext_ok(&p.out_ext, out_ext) {
        return Err(Invalid(format!("output type {out_ext:?}; allowed: {}", p.out_ext.join(", "))));
    }
    if n_out == 0 && !out_ext.is_empty() {
        return Err(Invalid("out_ext given but no {out} in the arguments".into()));
    }
    Ok(Plan { argv, in_file, out_file: if n_out == 1 { out_file } else { None } })
}

// ---------------------------------------------------------------- the program

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    h.finalize().iter().map(|x| format!("{x:02x}")).collect()
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}

/// The executable: `WRAP_TOOL_BIN_DIR`, then `~/.local/share/demo-wrap-anything/bin`, then PATH.
pub fn find_exe(p: &Policy) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = vec![];
    if let Some(d) = std::env::var_os("WRAP_TOOL_BIN_DIR") {
        dirs.push(d.into());
    }
    dirs.push(home().join(".local/share/demo-wrap-anything/bin"));
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    for b in &p.bin {
        for d in &dirs {
            let c = d.join(b);
            if c.is_file() {
                return Some(c);
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
    pub artifact_sha256: String,
    pub os_arch: String,
    pub node: String,
}

/// Name, version line and the sha256 of the executable file itself (symlinks resolved).
pub fn tool_info(p: &Policy, exe: &Path, node: &str) -> ToolInfo {
    let real = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    let artifact_sha256 = std::fs::read(&real).map(|b| sha256_hex(&b)).unwrap_or_default();
    let version = Command::new(exe)
        .args(&p.version_args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim().to_string())
        .unwrap_or_default();
    ToolInfo { name: p.bin[0].clone(), version, artifact_sha256, os_arch: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH), node: node.into() }
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub exit: i32,
    pub signal: String,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub output: Vec<u8>,
    pub ms: u64,
}

impl Outcome {
    pub fn output_cid(&self) -> String {
        if self.output.is_empty() {
            String::new()
        } else {
            format!("sha256:{}", sha256_hex(&self.output))
        }
    }
}

static SEQ: AtomicU64 = AtomicU64::new(0);

fn reader(mut r: impl Read + Send + 'static, cap: u64) -> std::thread::JoinHandle<(Vec<u8>, bool)> {
    std::thread::spawn(move || {
        let mut buf = vec![];
        let mut chunk = [0u8; 16384];
        let mut over = false;
        loop {
            match r.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if (buf.len() + n) as u64 > cap {
                        over = true;
                        let room = cap as usize - buf.len().min(cap as usize);
                        buf.extend_from_slice(&chunk[..room.min(n)]);
                    } else {
                        buf.extend_from_slice(&chunk[..n]);
                    }
                }
            }
        }
        (buf, over)
    })
}

/// Run a plan in a fresh directory. `limits`: the cell's sandbox (rlimits, own session, wall-clock kill);
/// `false` is the plain local run the parity test compares against (same argv, same environment, same file names).
pub fn exec(p: &Policy, exe: &Path, plan: &Plan, input: &[u8], limits: bool) -> Result<Outcome, WrapError> {
    let dir = std::env::temp_dir().join(format!("wrap-tool-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).map_err(|e| Failed(format!("work dir: {e}")))?;
    let r = exec_in(p, exe, plan, input, limits, &dir);
    let _ = std::fs::remove_dir_all(&dir); // our own scratch dir, made above
    r
}

fn exec_in(p: &Policy, exe: &Path, plan: &Plan, input: &[u8], limits: bool, dir: &Path) -> Result<Outcome, WrapError> {
    std::fs::write(dir.join(&plan.in_file), input).map_err(|e| Failed(format!("input: {e}")))?;
    let mut cmd = Command::new(exe);
    cmd.args(&plan.argv[1..]).current_dir(dir).env_clear().env("PATH", "/usr/bin:/bin").env("HOME", dir).env("LC_ALL", "C.UTF-8");
    cmd.envs(&p.env);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if limits {
        let (cpu, mem, fsize) = (p.cpu_s, p.mem_mb << 20, p.max_out_bytes.max(1 << 20) + 1);
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(move || {
                libc::setsid();
                let set = |res, v: u64| {
                    let l = libc::rlimit { rlim_cur: v as libc::rlim_t, rlim_max: v as libc::rlim_t };
                    libc::setrlimit(res, &l);
                };
                set(libc::RLIMIT_CPU, cpu);
                if mem > 0 {
                    set(libc::RLIMIT_DATA, mem);
                }
                set(libc::RLIMIT_FSIZE, fsize);
                set(libc::RLIMIT_CORE, 0);
                set(libc::RLIMIT_NOFILE, 256);
                Ok(())
            });
        }
    }
    let t0 = Instant::now();
    let mut child = cmd.spawn().map_err(|e| Failed(format!("start {}: {e}", p.bin[0])))?;
    let out = reader(child.stdout.take().unwrap(), p.max_stdio_bytes);
    let err = reader(child.stderr.take().unwrap(), p.max_stdio_bytes);
    let deadline = Duration::from_millis(p.timeout_ms);
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if limits && t0.elapsed() > deadline {
                    timed_out = true;
                    unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                    let _ = child.kill();
                    break child.wait().map_err(|e| Failed(e.to_string()))?;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(Failed(e.to_string())),
        }
    };
    let ms = t0.elapsed().as_millis() as u64;
    let (stdout, o_over) = out.join().unwrap_or_default();
    let (stderr, e_over) = err.join().unwrap_or_default();
    if timed_out {
        return Err(Limited(format!("{} ran past {} ms and was stopped", p.bin[0], p.timeout_ms)));
    }
    if limits && (o_over || e_over) {
        return Err(Limited(format!("stdout or stderr past {} bytes", p.max_stdio_bytes)));
    }
    use std::os::unix::process::ExitStatusExt;
    let (exit, signal) = match (status.code(), status.signal()) {
        (Some(c), _) => (c, String::new()),
        (None, Some(s)) => (128 + s, format!("signal {s}")),
        _ => (-1, "unknown".into()),
    };
    let output = match &plan.out_file {
        Some(f) => match std::fs::metadata(dir.join(f)) {
            Ok(m) if limits && m.len() > p.max_out_bytes => return Err(Limited(format!("output past {} bytes", p.max_out_bytes))),
            Ok(_) => std::fs::read(dir.join(f)).unwrap_or_default(),
            Err(_) => vec![],
        },
        None => vec![],
    };
    Ok(Outcome { exit, signal, stdout, stderr, output, ms })
}

// ---------------------------------------------------------------- the cell's state

/// One wrapped tool as the cell serves it: policy, executable, identity, and a concurrency limit.
pub struct Wrapped {
    pub policy: Policy,
    pub exe: PathBuf,
    pub info: ToolInfo,
    busy: AtomicU32,
}

impl Wrapped {
    pub fn new(tool: &str, node: &str) -> Result<Wrapped, String> {
        let policy = Policy::load(tool)?;
        let exe = find_exe(&policy).ok_or_else(|| format!("{}: not installed on this node", policy.bin[0]))?;
        let info = tool_info(&policy, &exe, node);
        Ok(Wrapped { policy, exe, info, busy: AtomicU32::new(0) })
    }

    pub fn run(&self, args: &[String], input: &[u8], in_ext: &str, out_ext: &str) -> Result<(Plan, Outcome), WrapError> {
        let plan = plan(&self.policy, args, input, in_ext, out_ext)?;
        if self.busy.fetch_add(1, Ordering::SeqCst) >= self.policy.concurrency {
            self.busy.fetch_sub(1, Ordering::SeqCst);
            return Err(Limited(format!("{} runs at a time; try again", self.policy.concurrency)));
        }
        let r = exec(&self.policy, &self.exe, &plan, input, true);
        self.busy.fetch_sub(1, Ordering::SeqCst);
        r.map(|o| (plan, o))
    }
}
