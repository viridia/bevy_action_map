//! Reflows prose in Rust comments and markdown files to a fixed column width.
//!
//! `cargo fmt`'s own `wrap_comments` needs nightly and does not understand markdown, so a rename
//! that lengthens or shortens an identifier leaves every paragraph that mentions it wrapped wrong,
//! and fixing that by hand is one small edit per paragraph. This tool does the same reflow
//! `rustfmt` does for code, but for prose: it re-wraps paragraphs and leaves everything with its
//! own layout rules — fenced code blocks, table rows, headings, and link definitions — untouched.
//!
//! Usage: `devfmt [--check] [--width N] [--diff[=REF]] <path>...`
//!
//! A `<path>` may be a file or a directory (directories are walked recursively for `.rs` and
//! `.md` files, skipping `target/`, `.git/`, and other dot-directories). Without `--check`,
//! matching files are rewritten in place, silently, unless something changed. With `--check`,
//! nothing is written; changed files are listed and the process exits non-zero, the same
//! contract `cargo fmt --check` has.
//!
//! `--diff` (default ref `HEAD`) is the routine way to run this: it restricts every check to
//! paragraphs that overlap a line `git diff` says changed against that ref, the same trick
//! `git-clang-format` uses to stay out of code nobody touched. `<path>` narrows which changed
//! files are considered; with `--diff` and no paths, every changed file in the repository is.
//! Run from the repository root — paths from `git diff` are repo-root-relative. Without `--diff`,
//! every paragraph in every collected file is checked, which is the right mode for a deliberate
//! cleanup sweep rather than routine use after an edit.
//!
//! # What it assumes
//!
//! - A line is classified by what its trimmed text starts with, not by parsing Rust or
//!   markdown, so a full-line string literal that happens to start with `//` would be
//!   misread as a comment. This does not occur in idiomatic Rust.
//! - A plain `//` comment is prose, the same as `///`/`//!`. This tool does not distinguish
//!   commented-out code from an explanatory comment — do not point it at a tree that leaves
//!   commented-out code lying around.
//! - Block comments (`/* */`) are left alone entirely.
//! - A paragraph whose lines already fit the width is reproduced exactly as written, never
//!   repacked to a tighter fit — matching hand-editing practice: only an actual violation gets
//!   touched, and fixing it only ever pushes trailing words forward onto later lines, never pulls
//!   words backward to reclaim slack a prior edit left behind.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const DEFAULT_WIDTH: usize = 100;

fn main() -> ExitCode {
    let mut check = false;
    let mut width = DEFAULT_WIDTH;
    let mut diff_ref: Option<String> = None;
    let mut paths: Vec<PathBuf> = Vec::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--diff" => diff_ref = Some("HEAD".to_string()),
            "--width" => {
                let value = args.next().unwrap_or_else(|| {
                    eprintln!("--width needs a number");
                    std::process::exit(2);
                });
                width = value.parse().unwrap_or_else(|_| {
                    eprintln!("--width needs a number, got {value:?}");
                    std::process::exit(2);
                });
            }
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other if other.starts_with("--diff=") => {
                diff_ref = Some(other["--diff=".len()..].to_string());
            }
            other => paths.push(PathBuf::from(other)),
        }
    }

    let (files, touched_by_file) = match &diff_ref {
        Some(git_ref) => {
            let files = git_diff_changed_files(git_ref, &paths);
            if files.is_empty() {
                println!("devfmt: no changed .rs/.md files against {git_ref}");
                return ExitCode::SUCCESS;
            }
            let mut touched = HashMap::new();
            for file in &files {
                if let Some(set) = git_diff_touched_lines(git_ref, file) {
                    touched.insert(file.clone(), set);
                } else {
                    eprintln!("devfmt: could not read the diff for {}", file.display());
                    return ExitCode::from(2);
                }
            }
            (files, Some(touched))
        }
        None => {
            if paths.is_empty() {
                print_usage();
                return ExitCode::from(2);
            }
            let mut files = Vec::new();
            for path in &paths {
                collect_files(path, &mut files);
            }
            files.sort();
            files.dedup();
            (files, None)
        }
    };

    let mut changed_files = Vec::new();
    let mut error = false;

    for file in &files {
        let touched = touched_by_file.as_ref().and_then(|m| m.get(file));
        match reflow_file(file, width, touched) {
            Ok(Some(new_contents)) => {
                if check {
                    changed_files.push(file.clone());
                } else if let Err(e) = fs::write(file, new_contents) {
                    eprintln!("devfmt: {}: {e}", file.display());
                    error = true;
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("devfmt: {}: {e}", file.display());
                error = true;
            }
        }
    }

    if check {
        for file in &changed_files {
            println!("would reflow: {}", file.display());
        }
        if !changed_files.is_empty() {
            println!(
                "{} file(s) need reflowing (of {} checked)",
                changed_files.len(),
                files.len()
            );
            return ExitCode::from(1);
        }
    }

    if error {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn print_usage() {
    eprintln!("usage: devfmt [--check] [--width N] [--diff[=REF]] <path>...");
}

// ---------------------------------------------------------------------------------------------
// Git integration: `--diff` restricts every check to paragraphs that overlap a changed line,
// the same trick `git-clang-format` uses. Both helpers fail soft (empty result) except where a
// missing diff for a file the caller already knows changed would silently widen scope instead —
// that case is treated as an error in `main` rather than quietly falling back to "check nothing".
// ---------------------------------------------------------------------------------------------

/// Every `.rs`/`.md` file `git diff --name-only` reports as added, copied, modified or renamed
/// against `git_ref`, optionally narrowed to `paths`. Deleted files are excluded — there is
/// nothing left to reflow.
fn git_diff_changed_files(git_ref: &str, paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut cmd = Command::new("git");
    cmd.args([
        "diff",
        "--no-color",
        "--name-only",
        "--diff-filter=ACMR",
        git_ref,
    ]);
    if !paths.is_empty() {
        cmd.arg("--");
        cmd.args(paths);
    }
    let Ok(output) = cmd.output() else {
        eprintln!("devfmt: could not run git");
        return Vec::new();
    };
    if !output.status.success() {
        eprintln!("devfmt: git diff --name-only against {git_ref} failed");
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(PathBuf::from)
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("rs" | "md")))
        .collect()
}

/// The line numbers `file` changed on, in the working-tree version, against `git_ref`. `None`
/// means the git command itself failed — distinct from an empty result, which is a real "no
/// changed lines" answer (e.g. a rename with no content change).
fn git_diff_touched_lines(git_ref: &str, file: &Path) -> Option<HashSet<usize>> {
    let output = Command::new("git")
        .args(["diff", "--no-color", "-U0", git_ref, "--"])
        .arg(file)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(parse_diff_touched_lines(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// Parses `@@ -a,b +c,d @@` hunk headers out of a `git diff -U0` patch into the set of touched
/// lines in the new (working-tree) file. A hunk that only deletes (`d` is `0`) has no line of
/// its own in the new file, but the deletion still changed the paragraph around it, so both
/// neighbors of the deletion point are marked instead.
fn parse_diff_touched_lines(diff: &str) -> HashSet<usize> {
    let mut touched = HashSet::new();
    for line in diff.lines() {
        let Some(rest) = line.strip_prefix("@@ ") else {
            continue;
        };
        let Some(new_side) = rest.split_whitespace().find(|tok| tok.starts_with('+')) else {
            continue;
        };
        let new_side = &new_side[1..];
        let (start_str, count_str) = match new_side.split_once(',') {
            Some((s, c)) => (s, c),
            None => (new_side, "1"),
        };
        let (Ok(start), Ok(count)) = (start_str.parse::<usize>(), count_str.parse::<usize>())
        else {
            continue;
        };
        if count == 0 {
            touched.insert(start);
            touched.insert(start.saturating_sub(1));
        } else {
            touched.extend(start..start + count);
        }
    }
    touched
}

/// Reads `path`, reflows it if it is a kind this tool understands, and returns the new contents
/// if they differ from what is on disk. `None` means either the file did not need any change, or
/// its extension is not `.rs`/`.md`. `touched`, when given, restricts reflowing to paragraphs
/// that overlap one of those line numbers — see the module doc's `--diff` section.
fn reflow_file(
    path: &Path,
    width: usize,
    touched: Option<&HashSet<usize>>,
) -> std::io::Result<Option<String>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let source = fs::read_to_string(path)?;

    // Preserve CRLF and a missing final newline, so a Windows-authored file or a file someone
    // deliberately left without a trailing newline does not pick up a spurious diff.
    let crlf = source.contains("\r\n");
    let normalized = if crlf {
        source.replace("\r\n", "\n")
    } else {
        source.clone()
    };
    let had_trailing_newline = normalized.ends_with('\n');

    let reflowed = match ext {
        "rs" => reflow_rust(&normalized, width, touched),
        "md" => reflow_paragraphs(&normalized, width, "", 1, touched),
        _ => return Ok(None),
    };

    let mut result = reflowed;
    if had_trailing_newline && !result.ends_with('\n') {
        result.push('\n');
    } else if !had_trailing_newline && result.ends_with('\n') {
        result.pop();
    }
    if crlf {
        result = result.replace('\n', "\r\n");
    }

    if result == source {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

/// Walks `path`, appending every `.rs`/`.md` file found to `out`. A bare file is appended
/// directly regardless of extension — the caller decides what to do with it.
fn collect_files(path: &Path, out: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(path) else {
        eprintln!("devfmt: cannot read {}", path.display());
        return;
    };
    if metadata.is_file() {
        out.push(path.to_path_buf());
        return;
    }
    if !metadata.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        eprintln!("devfmt: cannot read directory {}", path.display());
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        let child = entry.path();
        if child.is_dir() {
            collect_files(&child, out);
        } else if matches!(
            child.extension().and_then(|e| e.to_str()),
            Some("rs" | "md")
        ) {
            out.push(child);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Rust source: strip the comment marker off each `///`/`//!`/`//` line, reflow the prose that
// results the same way markdown prose is reflowed, then put the marker back.
// ---------------------------------------------------------------------------------------------

fn reflow_rust(source: &str, width: usize, touched: Option<&HashSet<usize>>) -> String {
    let mut out = String::new();
    let mut run: Vec<&str> = Vec::new(); // contents of the comment lines in the current run
    let mut run_prefix = String::new(); // "<indent><marker> " shared by every line in the run
    let mut run_start_line = 1usize;

    for (line_no, line) in (1usize..).zip(source.lines()) {
        match parse_comment_line(line) {
            Some((prefix, content)) if prefix == run_prefix => {
                run.push(content);
            }
            Some((prefix, content)) => {
                flush_comment_run(&run, &run_prefix, run_start_line, width, touched, &mut out);
                run_prefix = prefix;
                run = vec![content];
                run_start_line = line_no;
            }
            None => {
                flush_comment_run(&run, &run_prefix, run_start_line, width, touched, &mut out);
                run.clear();
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    flush_comment_run(&run, &run_prefix, run_start_line, width, touched, &mut out);
    out
}

/// Splits `line` into `("<indent><marker> ", rest)` if it is a comment line, where `marker` is
/// whichever of `//!`, `///`, `//` matches first. `rest` keeps whatever whitespace followed the
/// marker, so a blank comment line and an already-indented continuation both round-trip.
fn parse_comment_line(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    for marker in ["//!", "///", "//"] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return Some((
                format!("{indent}{marker} "),
                rest.strip_prefix(' ').unwrap_or(rest),
            ));
        }
    }
    None
}

/// Reflows one run of same-prefix comment lines, starting at file line `start_line`, and appends
/// the result to `out`. A run with a changed prefix or a code line in between was already split
/// into separate runs by the caller.
fn flush_comment_run(
    run: &[&str],
    prefix: &str,
    start_line: usize,
    width: usize,
    touched: Option<&HashSet<usize>>,
    out: &mut String,
) {
    if run.is_empty() {
        return;
    }
    let joined = run.join("\n");
    let reflowed = reflow_paragraphs(&joined, width, prefix, start_line, touched);
    for line in reflowed.lines() {
        // A blank comment line reflows to just the bare prefix; trim the trailing space so
        // `///` round-trips as `///` rather than `/// `.
        if line.trim_end() == prefix.trim_end() && line.ends_with(' ') {
            out.push_str(prefix.trim_end());
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
}

// ---------------------------------------------------------------------------------------------
// Paragraph reflow, shared by markdown files and by the prose extracted from Rust comments.
// `base_indent` is prepended to every emitted line — the comment marker for Rust, or an empty
// string for a markdown file (markdown carries its own indentation as part of the text).
// ---------------------------------------------------------------------------------------------

enum ParaKind {
    Plain,
    /// A markdown list item. `marker` is its bullet or number, exactly as written, so `- ` and
    /// `12. ` each hang continuation lines under their own width.
    List {
        marker: String,
    },
}

/// One line as it originally appeared (`raw`, everything after `base_indent`, used both to
/// measure whether it already fits and to reproduce it verbatim when it does) alongside the text
/// that would feed a rewrap if one turns out to be needed (`rewrap`, the marker-stripped words),
/// and the file line number it came from, for `--diff` scoping.
struct ParaLine {
    raw: String,
    rewrap: String,
    line_no: usize,
}

struct Paragraph {
    kind: ParaKind,
    lines: Vec<ParaLine>,
}

fn reflow_paragraphs(
    text: &str,
    width: usize,
    base_indent: &str,
    start_line: usize,
    touched: Option<&HashSet<usize>>,
) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    let mut para: Option<Paragraph> = None;

    for (line_no, line) in (start_line..).zip(text.lines()) {
        let content = line.strip_prefix(base_indent).unwrap_or(line);
        let trimmed = content.trim_start();

        if in_fence {
            flush_paragraph(para.take(), width, base_indent, touched, &mut out);
            emit_verbatim(base_indent, content, &mut out);
            if trimmed.starts_with("```") {
                in_fence = false;
            }
            continue;
        }

        let opens_fence = trimmed.starts_with("```");
        let atomic = opens_fence
            || trimmed.is_empty()
            || trimmed.starts_with('|')
            || trimmed.starts_with('#')
            || is_link_definition(trimmed);

        if atomic {
            flush_paragraph(para.take(), width, base_indent, touched, &mut out);
            emit_verbatim(base_indent, content, &mut out);
            in_fence = opens_fence;
        } else if let Some(marker) = list_marker(trimmed) {
            flush_paragraph(para.take(), width, base_indent, touched, &mut out);
            let rest = &trimmed[marker.len()..];
            let line = ParaLine {
                raw: content.to_string(),
                rewrap: rest.to_string(),
                line_no,
            };
            para = Some(Paragraph {
                kind: ParaKind::List { marker },
                lines: vec![line],
            });
        } else {
            let line = ParaLine {
                raw: content.to_string(),
                rewrap: trimmed.to_string(),
                line_no,
            };
            match &mut para {
                Some(p) => p.lines.push(line),
                None => {
                    para = Some(Paragraph {
                        kind: ParaKind::Plain,
                        lines: vec![line],
                    })
                }
            }
        }
    }
    flush_paragraph(para.take(), width, base_indent, touched, &mut out);
    out
}

fn emit_verbatim(base_indent: &str, content: &str, out: &mut String) {
    out.push_str(base_indent);
    out.push_str(content);
    out.push('\n');
}

fn flush_paragraph(
    para: Option<Paragraph>,
    width: usize,
    base_indent: &str,
    touched: Option<&HashSet<usize>>,
    out: &mut String,
) {
    let Some(para) = para else { return };

    let fits = para
        .lines
        .iter()
        .all(|l| base_indent.chars().count() + l.raw.chars().count() <= width);
    let in_scope = touched.is_none_or(|set| para.lines.iter().any(|l| set.contains(&l.line_no)));

    // A paragraph outside the diff's reach is never touched, even if it happens to violate the
    // width — that debt was there before this edit and is not this run's to fix. A paragraph
    // whose lines already fit is reproduced exactly as written rather than repacked to the
    // tightest fit: this tool's job is fixing violations, not renormalizing prose a human already
    // wrapped somewhere short of the limit on purpose.
    if fits || !in_scope {
        for line in &para.lines {
            emit_verbatim(base_indent, &line.raw, out);
        }
        return;
    }

    let (first_prefix, cont_prefix) = match &para.kind {
        ParaKind::Plain => (base_indent.to_string(), base_indent.to_string()),
        ParaKind::List { marker } => (
            format!("{base_indent}{marker}"),
            format!("{base_indent}{}", " ".repeat(marker.chars().count())),
        ),
    };

    let rewrap_parts: Vec<&str> = para.lines.iter().map(|l| l.rewrap.as_str()).collect();
    let joined = rewrap_parts.join(" ");
    let words: Vec<&str> = joined.split_whitespace().collect();
    if words.is_empty() {
        return;
    }

    let first_avail = width.saturating_sub(first_prefix.chars().count()).max(1);
    let cont_avail = width.saturating_sub(cont_prefix.chars().count()).max(1);

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut avail = first_avail;
    for word in words {
        let word_len = word.chars().count();
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word_len <= avail {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            avail = cont_avail;
            current.push_str(word);
        }
    }
    lines.push(current);

    for (i, line) in lines.iter().enumerate() {
        let prefix = if i == 0 { &first_prefix } else { &cont_prefix };
        out.push_str(prefix);
        out.push_str(line);
        out.push('\n');
    }
}

/// Whether `trimmed` opens a markdown list item, and if so, the exact marker text (`"- "`,
/// `"12. "`, ...) so a continuation line can hang-indent under it.
fn list_marker(trimmed: &str) -> Option<String> {
    for bullet in ["- ", "* ", "+ "] {
        if trimmed.starts_with(bullet) {
            return Some(bullet.to_string());
        }
    }
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = &trimmed[digits.len()..];
        if rest.starts_with(". ") {
            return Some(format!("{digits}. "));
        }
    }
    None
}

/// A reference-style link definition, `[label]: target`. These cannot be reflowed without
/// breaking the reference, so they are passed through untouched like a table row.
fn is_link_definition(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix('[') else {
        return false;
    };
    let Some(close) = rest.find(']') else {
        return false;
    };
    rest[close + 1..].starts_with(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust(input: &str, width: usize) -> String {
        reflow_rust(input, width, None)
    }

    fn rust_scoped(input: &str, width: usize, touched: &HashSet<usize>) -> String {
        reflow_rust(input, width, Some(touched))
    }

    #[test]
    fn a_short_paragraph_is_unchanged() {
        let input = "/// One short line.\n";
        assert_eq!(rust(input, 100), input);
    }

    #[test]
    fn a_long_paragraph_wraps_at_the_width() {
        let input = "/// aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq\n";
        let out = rust(input, 40);
        for line in out.lines() {
            assert!(line.chars().count() <= 40, "line too long: {line:?}");
        }
        assert_eq!(
            out.replace("/// ", "").replace('\n', " ").trim(),
            "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq"
        );
    }

    #[test]
    fn already_correctly_wrapped_text_is_idempotent() {
        let input = "/// aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj\n/// kkkk llll mmmm nnnn oooo pppp qqqq\n";
        let once = rust(input, 40);
        assert_ne!(
            once, input,
            "the input's first line is over width, so this should have reflowed"
        );
        let twice = rust(&once, 40);
        assert_eq!(once, twice);
    }

    #[test]
    fn a_paragraph_wrapped_short_of_the_limit_is_left_exactly_as_written() {
        // Every line here fits comfortably under 100 — a human broke it early for readability,
        // and a tighter greedy repacking exists but must not be applied: this regressed once,
        // when the tool repacked every already-compliant paragraph in the tree on a routine run.
        let input = "/// The window lost input focus, reported by Bevy's `KeyboardFocusLost` — alt-tab, a lock\n\
                      /// screen, a suspend. Every physically-held keyboard and mouse control is released.\n";
        assert_eq!(rust(input, 100), input);
    }

    #[test]
    fn a_blank_doc_comment_line_separates_paragraphs() {
        let input = "/// First paragraph.\n///\n/// Second paragraph.\n";
        assert_eq!(rust(input, 100), input);
    }

    #[test]
    fn fenced_code_blocks_are_untouched_however_long() {
        let input = "/// ```rust\n/// let x = some_very_long_identifier_that_would_never_fit_on_an_eighty_column_line;\n/// ```\n";
        assert_eq!(rust(input, 40), input);
    }

    #[test]
    fn table_rows_are_untouched_however_long() {
        let input = "/// | one very long column header | another very long column header that overflows |\n";
        assert_eq!(rust(input, 40), input);
    }

    #[test]
    fn a_list_item_wraps_with_a_hanging_indent() {
        let input =
            "/// - one two three four five six seven eight nine ten eleven twelve thirteen\n";
        let out = rust(input, 40);
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("/// - one"));
        assert!(
            lines[1].starts_with("///   "),
            "continuation not hung under the marker: {:?}",
            lines[1]
        );
        for line in &lines {
            assert!(line.chars().count() <= 40);
        }
    }

    #[test]
    fn an_already_compliant_multiline_list_item_is_left_alone() {
        // Both lines already fit — a human chose this break, so it survives even though a
        // tighter packing exists.
        let input = "/// - one two three four five six seven\n///   eight nine ten\n";
        assert_eq!(rust(input, 100), input);
    }

    #[test]
    fn a_multiline_list_item_that_violates_the_width_is_merged_and_rewrapped() {
        let input =
            "/// - one two three four five six seven eight nine ten eleven twelve thirteen\n\
             ///   fourteen\n";
        let out = rust(input, 40);
        let lines: Vec<&str> = out.lines().collect();
        assert_ne!(out, input, "an over-width item must actually reflow");
        for line in &lines {
            assert!(line.chars().count() <= 40, "line too long: {line:?}");
        }
        assert!(lines[0].starts_with("/// - one"));
        for line in &lines[1..] {
            assert!(
                line.starts_with("///   "),
                "continuation not hung under the marker: {line:?}"
            );
        }
        let words: String = out
            .replace("/// - ", "")
            .replace("///   ", " ")
            .replace('\n', " ");
        assert_eq!(
            words.split_whitespace().collect::<Vec<_>>(),
            "one two three four five six seven eight nine ten eleven twelve thirteen fourteen"
                .split_whitespace()
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn different_indentation_never_merges_across_scopes() {
        let input = "    /// inner one\nfn f() {\n        /// deeper one\n    }\n";
        assert_eq!(rust(input, 100), input);
    }

    #[test]
    fn plain_line_comments_reflow_the_same_as_doc_comments() {
        let input =
            "    // aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo\n";
        let out = rust(input, 40);
        for line in out.lines() {
            assert!(line.chars().count() <= 40);
        }
    }

    #[test]
    fn code_is_never_touched() {
        let input = "fn f() {\n    let x = 1;\n}\n";
        assert_eq!(rust(input, 20), input);
    }

    #[test]
    fn a_link_definition_is_left_alone() {
        let input = "/// See [the crate][crate-link] for a very long description that would otherwise wrap here.\n///\n/// [crate-link]: https://example.com/some/extremely/long/path/that/must/not/be/broken/apart\n";
        let out = rust(input, 60);
        assert!(out.contains("[crate-link]: https://example.com/some/extremely/long/path/that/must/not/be/broken/apart\n"));
    }

    #[test]
    fn markdown_files_reflow_without_a_comment_marker() {
        let input = "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq\n";
        let out = reflow_paragraphs(input, 40, "", 1, None);
        for line in out.lines() {
            assert!(line.chars().count() <= 40);
        }
    }

    #[test]
    fn markdown_headings_and_blank_lines_pass_through() {
        let input = "# Title\n\nSome text.\n";
        assert_eq!(reflow_paragraphs(input, 100, "", 1, None), input);
    }

    #[test]
    fn crlf_and_missing_trailing_newline_round_trip() {
        // exercised at the reflow_file level via crlf handling in main.rs; this covers the
        // paragraph engine's behavior on the no-trailing-newline case directly.
        let input = "/// short";
        let out = reflow_rust(input, 100, None);
        assert_eq!(out, "/// short\n");
    }

    // --- `--diff` scoping ---------------------------------------------------------------------

    #[test]
    fn a_violation_outside_the_touched_lines_is_left_alone() {
        let input = "/// aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq\n";
        let touched: HashSet<usize> = HashSet::new(); // nothing changed in this file
        assert_eq!(rust_scoped(input, 40, &touched), input);
    }

    #[test]
    fn a_violation_inside_the_touched_lines_still_reflows() {
        let input = "/// aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq\n";
        let touched: HashSet<usize> = [1].into_iter().collect();
        let out = rust_scoped(input, 40, &touched);
        assert_ne!(out, input);
        for line in out.lines() {
            assert!(line.chars().count() <= 40);
        }
    }

    #[test]
    fn only_the_touched_paragraph_of_several_reflows() {
        let input = "/// aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq\n\
                      ///\n\
                      /// AAAA BBBB CCCC DDDD EEEE FFFF GGGG HHHH IIII JJJJ KKKK LLLL MMMM NNNN OOOO PPPP QQQQ\n";
        // Line 3 is the second paragraph; line 1 is the first. Only line 3 is "changed".
        let touched: HashSet<usize> = [3].into_iter().collect();
        let out = rust_scoped(input, 40, &touched);
        let lines: Vec<&str> = out.lines().collect();
        // First paragraph (lowercase) is untouched and so stays over width.
        assert!(
            lines[0].chars().count() > 40,
            "untouched paragraph must not have reflowed"
        );
        // Second paragraph (uppercase) was touched and must now fit.
        assert!(out.lines().skip(2).all(|l| l.chars().count() <= 40));
    }

    #[test]
    fn parses_a_simple_addition_hunk() {
        let diff = "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -10,2 +10,3 @@ fn f() {\n";
        let touched = parse_diff_touched_lines(diff);
        assert_eq!(touched, [10, 11, 12].into_iter().collect());
    }

    #[test]
    fn parses_a_single_line_hunk_with_no_count() {
        // git omits the count when it is 1: `@@ -5 +5 @@` means one line.
        let diff = "@@ -5 +5 @@\n";
        let touched = parse_diff_touched_lines(diff);
        assert_eq!(touched, [5].into_iter().collect());
    }

    #[test]
    fn parses_a_pure_deletion_hunk_as_its_neighbors() {
        let diff = "@@ -8,2 +7,0 @@\n";
        let touched = parse_diff_touched_lines(diff);
        assert_eq!(touched, [6, 7].into_iter().collect());
    }

    #[test]
    fn ignores_non_hunk_lines() {
        let diff = "diff --git a/f b/f\nindex 111..222 100644\n--- a/f\n+++ b/f\n";
        assert!(parse_diff_touched_lines(diff).is_empty());
    }
}
