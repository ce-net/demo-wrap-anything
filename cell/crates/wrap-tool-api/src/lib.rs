//! Typed API of the `wrap.<tool>@1` cells: one command-line program behind a narrow argument allowlist, with size and
//! time limits. Every tool has the same shape, so one client speaks to all of them; each is its own api (its own
//! name, rights and schema hash) so a grant for `wrap.convert:run` never reaches pandoc. Types only: depend on this
//! crate to call a cell; TypeScript (browser, over wire.ws-door) and Python clients come from `ce-wire-gen`.

/// `wrap.convert@1`: ImageMagick convert as a mesh service.
pub mod convert {
    ce_wire::api! {
        api WrapConvert = "wrap.convert", version 1;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// `args` is the argv after the program name. `{in}` stands for the input file, `{out}` for the output
            /// file (`out.<out_ext>`); every other token must be on the tool's allowlist (`Describe`).
            pub struct RunReq { pub args: Vec<String>, pub input: Vec<u8>, pub in_ext: String, pub out_ext: String }
            /// The exact program that ran: name, first line of its version output, sha256 of the executable file,
            /// `<os>-<arch>`, and the short id of the node it ran on.
            pub struct ToolId { pub name: String, pub version: String, pub artifact_sha256: String, pub os_arch: String, pub node: String }
            /// `argv` is what ran (program first), so anyone can run the same command locally and compare hashes.
            /// `output_cid` is `sha256:<hex>` of `output` (the `{out}` file; empty when the call has none), the same
            /// address ce-blob stores it under. `signal` is set when the process was killed (exit is then 128+n).
            pub struct RunResp { pub exit: i32, pub signal: String, pub stdout: Vec<u8>, pub stderr: Vec<u8>, pub stdout_sha256: String, pub stderr_sha256: String, pub output: Vec<u8>, pub output_cid: String, pub argv: Vec<String>, pub tool: ToolId, pub ms: u64 }
            pub struct Flag { pub name: String, pub value: String }
            /// The allowlist and limits, as data.
            pub struct Policy { pub api: String, pub license: String, pub prefix: Vec<String>, pub flags: Vec<Flag>, pub positional: String, pub in_ext: Vec<String>, pub out_ext: Vec<String>, pub max_in_bytes: u64, pub max_out_bytes: u64, pub max_stdio_bytes: u64, pub timeout_ms: u64, pub mem_mb: u64, pub concurrency: u32, pub tool: ToolId }
            pub enum RunError { Invalid(String), Limited(String), Failed(String) }
        }
        ops {
            1 => HealthOp(()) -> Health, needs "wrap.convert:health";
            2 => Run(RunReq) -> Result<RunResp, RunError>, needs "wrap.convert:run", deadline_ms 30000;
            3 => Describe(()) -> Policy, needs "wrap.convert:describe";
        }
    }
}

/// `wrap.pandoc@1`: pandoc as a mesh service.
pub mod pandoc {
    ce_wire::api! {
        api WrapPandoc = "wrap.pandoc", version 1;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// `args` is the argv after the program name. `{in}` stands for the input file, `{out}` for the output
            /// file (`out.<out_ext>`); every other token must be on the tool's allowlist (`Describe`).
            pub struct RunReq { pub args: Vec<String>, pub input: Vec<u8>, pub in_ext: String, pub out_ext: String }
            /// The exact program that ran: name, first line of its version output, sha256 of the executable file,
            /// `<os>-<arch>`, and the short id of the node it ran on.
            pub struct ToolId { pub name: String, pub version: String, pub artifact_sha256: String, pub os_arch: String, pub node: String }
            /// `argv` is what ran (program first), so anyone can run the same command locally and compare hashes.
            /// `output_cid` is `sha256:<hex>` of `output` (the `{out}` file; empty when the call has none), the same
            /// address ce-blob stores it under. `signal` is set when the process was killed (exit is then 128+n).
            pub struct RunResp { pub exit: i32, pub signal: String, pub stdout: Vec<u8>, pub stderr: Vec<u8>, pub stdout_sha256: String, pub stderr_sha256: String, pub output: Vec<u8>, pub output_cid: String, pub argv: Vec<String>, pub tool: ToolId, pub ms: u64 }
            pub struct Flag { pub name: String, pub value: String }
            /// The allowlist and limits, as data.
            pub struct Policy { pub api: String, pub license: String, pub prefix: Vec<String>, pub flags: Vec<Flag>, pub positional: String, pub in_ext: Vec<String>, pub out_ext: Vec<String>, pub max_in_bytes: u64, pub max_out_bytes: u64, pub max_stdio_bytes: u64, pub timeout_ms: u64, pub mem_mb: u64, pub concurrency: u32, pub tool: ToolId }
            pub enum RunError { Invalid(String), Limited(String), Failed(String) }
        }
        ops {
            1 => HealthOp(()) -> Health, needs "wrap.pandoc:health";
            2 => Run(RunReq) -> Result<RunResp, RunError>, needs "wrap.pandoc:run", deadline_ms 30000;
            3 => Describe(()) -> Policy, needs "wrap.pandoc:describe";
        }
    }
}

/// `wrap.jq@1`: jq as a mesh service.
pub mod jq {
    ce_wire::api! {
        api WrapJq = "wrap.jq", version 1;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// `args` is the argv after the program name. `{in}` stands for the input file, `{out}` for the output
            /// file (`out.<out_ext>`); every other token must be on the tool's allowlist (`Describe`).
            pub struct RunReq { pub args: Vec<String>, pub input: Vec<u8>, pub in_ext: String, pub out_ext: String }
            /// The exact program that ran: name, first line of its version output, sha256 of the executable file,
            /// `<os>-<arch>`, and the short id of the node it ran on.
            pub struct ToolId { pub name: String, pub version: String, pub artifact_sha256: String, pub os_arch: String, pub node: String }
            /// `argv` is what ran (program first), so anyone can run the same command locally and compare hashes.
            /// `output_cid` is `sha256:<hex>` of `output` (the `{out}` file; empty when the call has none), the same
            /// address ce-blob stores it under. `signal` is set when the process was killed (exit is then 128+n).
            pub struct RunResp { pub exit: i32, pub signal: String, pub stdout: Vec<u8>, pub stderr: Vec<u8>, pub stdout_sha256: String, pub stderr_sha256: String, pub output: Vec<u8>, pub output_cid: String, pub argv: Vec<String>, pub tool: ToolId, pub ms: u64 }
            pub struct Flag { pub name: String, pub value: String }
            /// The allowlist and limits, as data.
            pub struct Policy { pub api: String, pub license: String, pub prefix: Vec<String>, pub flags: Vec<Flag>, pub positional: String, pub in_ext: Vec<String>, pub out_ext: Vec<String>, pub max_in_bytes: u64, pub max_out_bytes: u64, pub max_stdio_bytes: u64, pub timeout_ms: u64, pub mem_mb: u64, pub concurrency: u32, pub tool: ToolId }
            pub enum RunError { Invalid(String), Limited(String), Failed(String) }
        }
        ops {
            1 => HealthOp(()) -> Health, needs "wrap.jq:health";
            2 => Run(RunReq) -> Result<RunResp, RunError>, needs "wrap.jq:run", deadline_ms 30000;
            3 => Describe(()) -> Policy, needs "wrap.jq:describe";
        }
    }
}
