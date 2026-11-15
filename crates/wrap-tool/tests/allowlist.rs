//! The allowlist: what reaches the program, and what is refused before it starts.
use wrap_tool::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn png() -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/ducks.png")).unwrap()
}

#[test]
fn convert_builds_argv_with_coder_prefix() {
    let p = Policy::load("convert").unwrap();
    let pl = plan(&p, &s(&["{in}", "-resize", "50%", "{out}"]), &png(), "png", "webp").unwrap();
    assert_eq!(pl.argv, s(&["convert", "png:in.png", "-resize", "50%", "out.webp"]));
    assert_eq!(pl.out_file.as_deref(), Some("out.webp"));
}

#[test]
fn convert_refuses_what_is_not_listed() {
    let p = Policy::load("convert").unwrap();
    for bad in [
        s(&["{in}", "-write", "/tmp/x.png", "{out}"]),     // writes anywhere
        s(&["{in}", "-resize", "50%;rm", "{out}"]),        // value outside its kind
        s(&["{in}", "-quality", "900", "{out}"]),          // out of range
        s(&["msl:/etc/passwd", "{out}"]),                 // a coder path
        s(&["{in}", "{in}", "{out}"]),                    // input twice
        s(&["{in}", "-resize", "50%"]),                   // no output
    ] {
        assert!(matches!(plan(&p, &bad, &png(), "png", "png"), Err(WrapError::Invalid(_))), "{bad:?}");
    }
    // the bytes must be what the extension says
    assert!(matches!(plan(&p, &s(&["{in}", "{out}"]), b"<svg/>", "png", "png"), Err(WrapError::Invalid(_))));
    assert!(matches!(plan(&p, &s(&["{in}", "{out}"]), &png(), "gif", "png"), Err(WrapError::Invalid(_))));
    assert!(matches!(plan(&p, &s(&["{in}", "{out}"]), &png(), "png", "svg"), Err(WrapError::Invalid(_))));
}

#[test]
fn size_limit_is_checked_before_anything_runs() {
    let p = Policy::load("convert").unwrap();
    let mut big = png();
    big.resize(p.max_in_bytes as usize + 1, 0);
    assert!(matches!(plan(&p, &s(&["{in}", "{out}"]), &big, "png", "png"), Err(WrapError::Limited(_))));
}

#[test]
fn pandoc_is_always_sandboxed_and_writes_only_out() {
    let p = Policy::load("pandoc").unwrap();
    let pl = plan(&p, &s(&["{in}", "-f", "gfm", "-t", "html", "-o", "{out}"]), b"# hi", "md", "html").unwrap();
    assert_eq!(pl.argv[..2], s(&["pandoc", "--sandbox"]));
    assert!(plan(&p, &s(&["{in}", "-o", "/etc/x"]), b"# hi", "md", "").is_err());
    assert!(plan(&p, &s(&["{in}", "--lua-filter", "x.lua"]), b"# hi", "md", "").is_err());
    assert!(plan(&p, &s(&["{in}", "-t", "pdf"]), b"# hi", "md", "").is_err());
}

#[test]
fn jq_filter_cannot_read_files() {
    let p = Policy::load("jq").unwrap();
    assert!(plan(&p, &s(&["-c", ".a", "{in}"]), b"{}", "json", "").is_ok());
    for f in ["import \"/home/x/secret\" as $s; $s", "include \"x\"; .", "input_filename"] {
        assert!(plan(&p, &s(&[f, "{in}"]), b"{}", "json", "").is_err(), "{f}");
    }
    assert!(plan(&p, &s(&["--rawfile", "a", "/etc/passwd", "{in}"]), b"{}", "json", "").is_err());
}

#[test]
fn exec_kills_a_runaway_program() {
    // jq looping forever is stopped at the policy's wall clock, not left running
    let mut p = Policy::load("jq").unwrap();
    let Some(exe) = find_exe(&p) else { return };
    p.timeout_ms = 300;
    let pl = plan(&p, &s(&["[range(0; infinite)] | length", "{in}"]), b"{}", "json", "").unwrap();
    let t = std::time::Instant::now();
    assert!(matches!(exec(&p, &exe, &pl, b"{}", true), Err(WrapError::Limited(_))));
    assert!(t.elapsed().as_millis() < 3000);
}

#[test]
fn limited_and_plain_runs_agree() {
    let p = Policy::load("convert").unwrap();
    let Some(exe) = find_exe(&p) else { return };
    let pl = plan(&p, &s(&["{in}", "-resize", "50%", "-strip", "{out}"]), &png(), "png", "png").unwrap();
    let a = exec(&p, &exe, &pl, &png(), true).unwrap();
    let b = exec(&p, &exe, &pl, &png(), false).unwrap();
    assert_eq!((a.exit, sha256_hex(&a.stdout), sha256_hex(&a.stderr), a.output_cid()), (b.exit, sha256_hex(&b.stdout), sha256_hex(&b.stderr), b.output_cid()));
    assert!(a.output_cid().starts_with("sha256:"));
}
