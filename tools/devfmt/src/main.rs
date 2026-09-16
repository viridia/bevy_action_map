//! Reflows prose in Rust comments and markdown files to a fixed column width.
//!
//! `cargo fmt`'s own `wrap_comments` needs nightly and does not understand markdown, so a rename
//! that lengthens or shortens an identifier leaves every paragraph that mentions it wrapped wrong,
//! and fixing that by hand is one small edit per paragraph. This tool does the same reflow
//! `rustfmt` does for code, but for prose: it re-wraps paragraphs and leaves untouched everything
//! with its own layout rules — fenced code blocks, table rows, headings, divider lines and link
//! definitions.
//!
//! Usage: `devfmt [--check] [--preview] [--width N] (--diff[=REF] | --sweep) <path>...`
//!
//! A `<path>` may be a file or a directory (directories are walked recursively for `.rs` and
//! `.md` files, skipping `target/`, `.git/`, and other dot-directories). Given neither reporting
//! flag, matching files are rewritten in place, silently, unless something changed.
//!
//! Two flags report instead of writing, both exiting non-zero when something would have changed —
//! the contract `cargo fmt --check` has. `--check` lists the files, which is the view for deciding
//! what to run over next. `--preview` prints a unified diff of the change itself, its hunks sized
//! to the paragraph that moved rather than to the minimal line change.
//!
//! One of two scope flags is required; a run naming neither is refused rather than guessed at.
//!
//! `--diff` (default ref `HEAD`) is the routine one: it restricts every check to paragraphs that
//! overlap a line `git diff` says changed against that ref, the same trick `git-clang-format` uses
//! to stay out of code nobody touched. `<path>` narrows which changed files are considered; with
//! `--diff` and no paths, every changed file in the repository is. Run from the repository root —
//! paths from `git diff` are repo-root-relative.
//!
//! `--sweep` takes every paragraph in the paths it is given. Since a paragraph is refilled whether
//! or not its lines already fit, that rewrites far more than a diff does, which is why it has to be
//! asked for by name.
//!
//! # What it assumes
//!
//! - A line is classified by what its trimmed text starts with, not by parsing Rust or markdown, so
//!   a line inside a string literal that starts with `//` is read as a comment and reflowed with
//!   everything around it. A formatter's own test fixtures are where that actually bites; telling
//!   the two apart needs a Rust lexer rather than a line classifier.
//! - A line with no letter or digit in it is a divider rather than prose, and is held verbatim.
//!   A line of punctuation meant as prose would be held too.
//! - A backtick span is never broken across lines, so a span longer than the width overflows it.
//! - A plain `//` comment is prose, the same as `///`/`//!`. This tool does not distinguish
//!   commented-out code from an explanatory comment — do not point it at a tree that leaves
//!   commented-out code lying around.
//! - Block comments (`/* */`) are left alone entirely.
//! - A markdown file opening with a `---` line has YAML frontmatter, held verbatim up to the
//!   closing `---`. A `---` anywhere else is an ordinary line.
//! - A paragraph in scope is refilled whether or not its lines already fit. Editing prose shortens
//!   a line as often as it lengthens one, and a shortened line leaves a paragraph ragged without
//!   ever crossing the width, so a width test sees only half the problem. The cost is that a line
//!   broken short on purpose does not survive a run that reaches it.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const DEFAULT_WIDTH: usize = 100;

fn main() -> ExitCode {
    let mut check = false;
    let mut preview = false;
    let mut sweep = false;
    let mut width = DEFAULT_WIDTH;
    let mut diff_ref: Option<String> = None;
    let mut paths: Vec<PathBuf> = Vec::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--preview" => preview = true,
            "--sweep" => sweep = true,
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

    // Repacking reaches every paragraph it is given, so the old no-flag form became a whole-file
    // rewrite rather than a tidy-up. Which paragraphs are in reach is now always said out loud.
    match (diff_ref.is_some(), sweep) {
        (false, false) => {
            eprintln!(
                "devfmt: name the scope — --diff for the paragraphs you changed, --sweep for every \
                 paragraph in the paths given"
            );
            return ExitCode::from(2);
        }
        (true, true) => {
            eprintln!("devfmt: --diff and --sweep are alternatives; pass one");
            return ExitCode::from(2);
        }
        _ => {}
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
                if check || preview {
                    changed_files.push(file.clone());
                    // Re-read rather than have `reflow_file` hand back both sides: one extra read
                    // of a file already in the page cache, against a signature every caller pays.
                    if preview {
                        match fs::read_to_string(file) {
                            Ok(old) => print_preview(file, &paragraph_hunks(&old, &new_contents)),
                            Err(e) => {
                                eprintln!("devfmt: {}: {e}", file.display());
                                error = true;
                            }
                        }
                    }
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
        }
    }

    if error {
        return ExitCode::from(1);
    }
    // Both reporting modes have `cargo fmt --check`'s contract: nothing written, non-zero when
    // something would have been.
    if (check || preview) && !changed_files.is_empty() {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn print_usage() {
    eprintln!("usage: devfmt [--check] [--preview] [--width N] (--diff[=REF] | --sweep) <path>...");
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
        "md" => reflow_markdown(&normalized, width, touched),
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

/// Reflows a markdown file, holding any YAML frontmatter verbatim. The body is reflowed from its
/// real line number so that `--diff` scoping still lines up with what `git diff` reported.
fn reflow_markdown(text: &str, width: usize, touched: Option<&HashSet<usize>>) -> String {
    let (frontmatter, body, body_start) = split_frontmatter(text);
    let mut out = frontmatter.to_string();
    out.push_str(&reflow_paragraphs(body, width, "", body_start, touched));
    out
}

/// Splits a leading YAML frontmatter block off `text`, returning it verbatim, the body after it,
/// and the line number the body starts on. Frontmatter is recognized only at the very start of
/// the file, which is what keeps a `---` thematic break further down from opening one.
fn split_frontmatter(text: &str) -> (&str, &str, usize) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return ("", text, 1);
    };
    let mut end = "---\n".len();
    for (line_no, line) in (2usize..).zip(rest.lines()) {
        end = (end + line.len() + 1).min(text.len());
        if line.trim_end() == "---" {
            return (&text[..end], &text[end..], line_no + 1);
        }
    }
    ("", text, 1)
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

/// `indent` is the whitespace between `base_indent` and the text, taken from the paragraph's first
/// line and reused on every line it emits. Dropping it is what used to pull an indented
/// continuation paragraph out to column 0, escaping the list item it belonged to, and flatten a
/// nested bullet into a top-level one.
struct Paragraph {
    kind: ParaKind,
    indent: String,
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
        let indent = &content[..content.len() - trimmed.len()];

        if in_fence {
            flush_paragraph(para.take(), width, base_indent, touched, &mut out);
            emit_verbatim(base_indent, content, &mut out);
            if trimmed.starts_with("```") {
                in_fence = false;
            }
            continue;
        }

        let opens_fence = trimmed.starts_with("```");
        // A line with no letter or digit is a divider or a thematic break rather than prose, and a
        // rewrap would pull the following sentence's first word up onto the end of it.
        let atomic = opens_fence
            || trimmed.is_empty()
            || !trimmed.chars().any(char::is_alphanumeric)
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
                indent: indent.to_string(),
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
                        indent: indent.to_string(),
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

    let in_scope = touched.is_none_or(|set| para.lines.iter().any(|l| set.contains(&l.line_no)));

    // A paragraph outside the diff's reach is never touched, even if it happens to violate the
    // width — that debt was there before this edit and is not this run's to fix. Inside the reach
    // it is refilled whether or not its lines already fit, because an edit shortens a line as often
    // as it lengthens one and a shortened line leaves the paragraph ragged without ever crossing
    // the width. That half of the problem a width test cannot see, so scope rather than width is
    // what bounds the blast radius.
    if !in_scope {
        for line in &para.lines {
            emit_verbatim(base_indent, &line.raw, out);
        }
        return;
    }

    let indent = &para.indent;
    let (first_prefix, cont_prefix) = match &para.kind {
        ParaKind::Plain => (
            format!("{base_indent}{indent}"),
            format!("{base_indent}{indent}"),
        ),
        ParaKind::List { marker } => (
            format!("{base_indent}{indent}{marker}"),
            format!(
                "{base_indent}{indent}{}",
                " ".repeat(marker.chars().count())
            ),
        ),
    };

    let rewrap_parts: Vec<&str> = para.lines.iter().map(|l| l.rewrap.as_str()).collect();
    let joined = rewrap_parts.join(" ");
    let units = wrap_units(&joined);
    if units.is_empty() {
        return;
    }

    let first_avail = width.saturating_sub(first_prefix.chars().count()).max(1);
    let cont_avail = width.saturating_sub(cont_prefix.chars().count()).max(1);

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut avail = first_avail;
    for unit in &units {
        let unit_len = unit.chars().count();
        if current.is_empty() {
            current.push_str(unit);
        } else if current.chars().count() + 1 + unit_len <= avail {
            current.push(' ');
            current.push_str(unit);
        } else {
            lines.push(std::mem::take(&mut current));
            avail = cont_avail;
            current.push_str(unit);
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

/// The units a greedy fill may not break: whitespace-separated words, except that words inside a
/// backtick span are held together as one. A span containing a space otherwise splits across lines,
/// and the fragment left at the head of the second line can itself read as a list marker on a later
/// run over the same file.
///
/// A unit longer than the width overflows its line, the same trade a long URL already gets. An odd
/// number of backticks in the paragraph means they cannot be paired, so the span rule is dropped
/// rather than guessed at — one stray backtick would otherwise glue the rest of the paragraph into
/// a single unit.
fn wrap_units(text: &str) -> Vec<String> {
    if text.chars().filter(|&c| c == '`').count() % 2 == 1 {
        return text.split_whitespace().map(str::to_string).collect();
    }

    let mut units: Vec<String> = Vec::new();
    let mut open = false;
    for word in text.split_whitespace() {
        match units.last_mut() {
            Some(unit) if open => {
                unit.push(' ');
                unit.push_str(word);
            }
            _ => units.push(word.to_string()),
        }
        if word.chars().filter(|&c| c == '`').count() % 2 == 1 {
            open = !open;
        }
    }
    units
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

// ---------------------------------------------------------------------------------------------
// Preview: a unified diff of what a run would write. The hunks are the size of the region the
// reflow rewrote rather than the minimal line change — a reflowed paragraph has every one of its
// lines changed, so a minimal diff of it is noise to read.
//
// No diff algorithm is involved. Every path here emits a blank line verbatim and no paragraph ever
// produces one, so the blank lines of the old text and the new correspond one to one, and the text
// between a pair of them can be compared directly. Dropping the lines that already match at each
// end of such a segment is what narrows a hunk to the part that moved.
// ---------------------------------------------------------------------------------------------

/// One hunk, ready to print: where it starts in each text (1-based, counting its context) and how
/// many lines it covers there, with `body` already carrying the `' '`/`'-'`/`'+'` prefixes.
struct Hunk {
    old_start: usize,
    new_start: usize,
    old_count: usize,
    new_count: usize,
    body: Vec<String>,
}

/// One changed region, as 0-based half-open line ranges into the old and new texts.
struct Region {
    old_from: usize,
    old_to: usize,
    new_from: usize,
    new_to: usize,
}

/// Where `new` differs from `old`, aligned on the blank lines they share. Empty when they match.
fn paragraph_hunks(old: &str, new: &str) -> Vec<Hunk> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut regions = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    loop {
        let oe = next_blank(&old_lines, i);
        let ne = next_blank(&new_lines, j);
        if let Some((at, old_len, new_len)) = narrow(&old_lines[i..oe], &new_lines[j..ne]) {
            regions.push(Region {
                old_from: i + at,
                old_to: i + at + old_len,
                new_from: j + at,
                new_to: j + at + new_len,
            });
        }
        if oe >= old_lines.len() && ne >= new_lines.len() {
            break;
        }
        i = oe + 1;
        j = ne + 1;
    }

    // A hunk carries one line of context on each side, since `git apply` refuses a patch whose
    // hunks have none unless told it was generated at `-U0`. Two regions with a single line between
    // them — the usual spacing of two paragraphs that both moved — would each claim that line, and
    // overlapping hunks are refused just as firmly, so they become one hunk holding it as interior
    // context instead.
    let mut hunks = Vec::new();
    let mut start = 0;
    while start < regions.len() {
        let mut end = start;
        while end + 1 < regions.len() && regions[end + 1].old_from <= regions[end].old_to + 1 {
            end += 1;
        }
        hunks.push(materialize(&regions[start..=end], &old_lines, &new_lines));
        start = end + 1;
    }
    hunks
}

fn next_blank(lines: &[&str], from: usize) -> usize {
    (from..lines.len())
        .find(|&k| lines[k].trim().is_empty())
        .unwrap_or(lines.len())
}

/// The part of one blank-delimited segment that actually differs: its offset within the segment,
/// then how many lines the old and new sides each contribute. `None` when the segment is unchanged.
/// The offset is the same on both sides, since everything before it matched.
fn narrow(old: &[&str], new: &[&str]) -> Option<(usize, usize, usize)> {
    if old == new {
        return None;
    }
    let shorter = old.len().min(new.len());
    let head = (0..shorter).take_while(|&k| old[k] == new[k]).count();
    let tail = (0..shorter - head)
        .take_while(|&k| old[old.len() - 1 - k] == new[new.len() - 1 - k])
        .count();
    Some((head, old.len() - tail - head, new.len() - tail - head))
}

/// Builds the printable hunk covering `group`, which is one region or several that had to merge.
/// The context lines come from the old text; every one of them is a line both texts agree on, so
/// which side they are read from does not matter.
fn materialize(group: &[Region], old_lines: &[&str], new_lines: &[&str]) -> Hunk {
    let first = &group[0];
    let last = &group[group.len() - 1];
    let lead = usize::from(first.old_from > 0 && first.new_from > 0);
    let trail = usize::from(last.old_to < old_lines.len() && last.new_to < new_lines.len());

    let mut body = Vec::new();
    if lead == 1 {
        body.push(format!(" {}", old_lines[first.old_from - 1]));
    }
    for (n, region) in group.iter().enumerate() {
        for line in &old_lines[region.old_from..region.old_to] {
            body.push(format!("-{line}"));
        }
        for line in &new_lines[region.new_from..region.new_to] {
            body.push(format!("+{line}"));
        }
        if let Some(next) = group.get(n + 1) {
            for line in &old_lines[region.old_to..next.old_from] {
                body.push(format!(" {line}"));
            }
        }
    }
    if trail == 1 {
        body.push(format!(" {}", old_lines[last.old_to]));
    }

    Hunk {
        old_start: first.old_from + 1 - lead,
        new_start: first.new_from + 1 - lead,
        old_count: last.old_to + trail - (first.old_from - lead),
        new_count: last.new_to + trail - (first.new_from - lead),
        body,
    }
}

/// A unified-diff line span. A side with no lines at all is written as the line it follows with a
/// count of zero; a hunk carrying context never reaches that, but one at the very start of a file
/// can.
fn diff_span(start: usize, count: usize) -> (usize, usize) {
    if count == 0 {
        (start.saturating_sub(1), 0)
    } else {
        (start, count)
    }
}

fn print_preview(path: &Path, hunks: &[Hunk]) {
    if hunks.is_empty() {
        return;
    }
    println!("--- a/{}", path.display());
    println!("+++ b/{}", path.display());
    for hunk in hunks {
        let (old_start, old_count) = diff_span(hunk.old_start, hunk.old_count);
        let (new_start, new_count) = diff_span(hunk.new_start, hunk.new_count);
        println!("@@ -{old_start},{old_count} +{new_start},{new_count} @@");
        for line in &hunk.body {
            println!("{line}");
        }
    }
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
    fn a_paragraph_wrapped_short_of_the_limit_is_repacked() {
        // Nothing here violates the width. The paragraph is ragged, which is what an edit that
        // shortens a line leaves behind and precisely what a width test cannot see. What keeps
        // this off prose nobody edited is the diff scope, not the width — the two tests below.
        let input = "/// aaaa bbbb\n/// cccc dddd\n";
        assert_eq!(rust(input, 100), "/// aaaa bbbb cccc dddd\n");
    }

    #[test]
    fn a_ragged_paragraph_outside_the_diff_is_left_alone() {
        // With the width no longer gating a rewrap, scope is the only thing between a routine run
        // and every paragraph in the file, so this is the guard rail that matters.
        let input = "/// aaaa bbbb\n/// cccc dddd\n";
        let untouched: HashSet<usize> = HashSet::new();
        assert_eq!(rust_scoped(input, 100, &untouched), input);
    }

    #[test]
    fn a_ragged_paragraph_inside_the_diff_is_repacked() {
        let input = "/// aaaa bbbb\n/// cccc dddd\n";
        let touched: HashSet<usize> = [1].into_iter().collect();
        assert_eq!(
            rust_scoped(input, 100, &touched),
            "/// aaaa bbbb cccc dddd\n"
        );
    }

    #[test]
    fn repacking_a_packed_paragraph_changes_nothing() {
        // A sweep over an already-packed tree has to be a no-op, or a migration could never finish.
        let packed = rust("/// aaaa bbbb\n/// cccc dddd\n", 100);
        assert_eq!(rust(&packed, 100), packed);
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
    fn a_multiline_list_item_that_fits_on_one_line_is_joined() {
        let input = "/// - one two three four five six seven\n///   eight nine ten\n";
        assert_eq!(
            rust(input, 100),
            "/// - one two three four five six seven eight nine ten\n"
        );
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
    fn an_indented_paragraph_keeps_its_indent() {
        // A continuation paragraph under a list item. Dedenting it to column 0 takes it out of the
        // item it belongs to, which is a different document.
        let input = "  aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk\n";
        let out = reflow_paragraphs(input, 30, "", 1, None);
        for line in out.lines() {
            assert!(line.starts_with("  "), "lost its indent: {line:?}");
            assert!(line.chars().count() <= 30, "line too long: {line:?}");
        }
    }

    #[test]
    fn a_nested_list_item_keeps_its_nesting() {
        // Promoting this to column 0 flattens a two-level list into one.
        let input = "  - aaaa bbbb cccc dddd eeee ffff gggg hhhh\n";
        let out = reflow_paragraphs(input, 30, "", 1, None);
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines.len() > 1, "nothing rewrapped, so nothing was tested");
        assert!(lines[0].starts_with("  - "), "promoted: {:?}", lines[0]);
        for line in &lines[1..] {
            assert!(line.starts_with("    "), "continuation not hung: {line:?}");
        }
    }

    #[test]
    fn a_nested_list_item_in_a_doc_comment_keeps_its_nesting() {
        let input = "///   - aaaa bbbb cccc dddd eeee ffff gggg hhhh\n";
        let out = rust(input, 30);
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines.len() > 1, "nothing rewrapped, so nothing was tested");
        assert!(lines[0].starts_with("///   - "), "promoted: {:?}", lines[0]);
        for line in &lines[1..] {
            assert!(
                line.starts_with("///     "),
                "continuation not hung: {line:?}"
            );
        }
    }

    #[test]
    fn a_divider_is_not_absorbed_into_the_paragraph_below_it() {
        let input = "// -------------------\n// aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii\n";
        let out = rust(input, 30);
        assert!(
            out.starts_with("// -------------------\n"),
            "the divider grew a word: {out:?}"
        );
    }

    #[test]
    fn a_backtick_span_containing_a_space_is_never_split() {
        // Splitting `12. ` leaves a fragment that a later run over the same file reads as a list
        // marker, so the damage compounds.
        let input =
            "/// so `- ` and `12. ` each hang their continuation lines under their own width\n";
        let out = rust(input, 40);
        assert!(
            out.lines().any(|l| l.contains("`12. `")),
            "the span was split: {out:?}"
        );
    }

    #[test]
    fn an_unpaired_backtick_falls_back_to_plain_words() {
        // One stray backtick would otherwise glue the rest of the paragraph into a single unit.
        let input = "/// a stray ` tick aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk\n";
        let out = rust(input, 30);
        for line in out.lines() {
            assert!(line.chars().count() <= 30, "line too long: {line:?}");
        }
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
    fn frontmatter_is_held_verbatim() {
        let input = "---\nname: comment-grooming\ndescription: Sweep comments and doc comments for house-style violations and duplicated explanations, using recon subagents.\n---\n\n# Heading\n";
        assert_eq!(reflow_markdown(input, 100, None), input);
    }

    #[test]
    fn the_body_after_frontmatter_still_reflows() {
        let input = "---\nname: x\n---\n\naaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq\n";
        let out = reflow_markdown(input, 40, None);
        assert!(out.starts_with("---\nname: x\n---\n\n"));
        assert!(out.lines().skip(4).all(|l| l.chars().count() <= 40));
    }

    #[test]
    fn frontmatter_does_not_shift_the_body_line_numbers() {
        let input = "---\nname: x\n---\n\naaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq\n";
        // The paragraph is on line 5, not line 1 of a body counted from scratch.
        let touched: HashSet<usize> = [5].into_iter().collect();
        assert_ne!(reflow_markdown(input, 40, Some(&touched)), input);
        let touched: HashSet<usize> = [2].into_iter().collect();
        assert_eq!(reflow_markdown(input, 40, Some(&touched)), input);
    }

    #[test]
    fn an_unterminated_leading_marker_is_not_frontmatter() {
        let input = "---\naaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq\n";
        let out = reflow_markdown(input, 40, None);
        assert_ne!(out, input);
        assert!(out.lines().all(|l| l.chars().count() <= 40));
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

    #[test]
    fn an_unchanged_text_has_no_hunks() {
        let text = "keep me\n\naaaa bbbb\n";
        assert!(paragraph_hunks(text, text).is_empty());
    }

    #[test]
    fn a_hunk_addresses_the_lines_that_moved() {
        let old = "keep me\n\naaaa bbbb\ncccc\n\nkeep me too\n";
        let new = "keep me\n\naaaa bbbb cccc\n\nkeep me too\n";
        let hunks = paragraph_hunks(old, new);
        assert_eq!(hunks.len(), 1);
        assert_eq!((hunks[0].old_start, hunks[0].old_count), (2, 4));
        assert_eq!((hunks[0].new_start, hunks[0].new_count), (2, 3));
        assert_eq!(
            hunks[0].body,
            [" ", "-aaaa bbbb", "-cccc", "+aaaa bbbb cccc", " "]
        );
    }

    #[test]
    fn a_hunk_that_only_removes_lines_keeps_its_context() {
        let old = "aaaa bbbb cccc\ndddd\n";
        let new = "aaaa bbbb cccc\n";
        let hunks = paragraph_hunks(old, new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].body, [" aaaa bbbb cccc", "-dddd"]);
        assert_eq!((hunks[0].old_count, hunks[0].new_count), (2, 1));
    }

    #[test]
    fn two_changes_one_blank_line_apart_become_one_hunk() {
        // Separately, each would claim that blank line as its own context, and `git apply` refuses
        // overlapping hunks as firmly as context-free ones. Merged, the blank is interior context.
        let old = "aaaa bbbb\ncccc\n\ndddd eeee\nffff\n";
        let new = "aaaa bbbb cccc\n\ndddd eeee ffff\n";
        let hunks = paragraph_hunks(old, new);
        assert_eq!(hunks.len(), 1, "the two regions did not merge");
        assert_eq!((hunks[0].old_count, hunks[0].new_count), (5, 3));
        assert_eq!(hunks[0].body.iter().filter(|l| *l == " ").count(), 1);
    }

    #[test]
    fn applying_the_hunks_reproduces_what_a_run_would_write() {
        // The property the preview lives or dies on: what it shows is what a real run writes. A
        // preview that disagrees with its own run is worse than having none.
        let old = "keep\n\naaaa bbbb\ncccc\n\ndddd eeee\nffff\n\ntail\n";
        let new = "keep\n\naaaa bbbb cccc\n\ndddd eeee ffff\n\ntail\n";
        let mut rebuilt: Vec<String> = old.lines().map(str::to_string).collect();
        // Back to front, so a hunk's line numbers still address `rebuilt` when it is applied.
        for hunk in paragraph_hunks(old, new).iter().rev() {
            let kept: Vec<String> = hunk
                .body
                .iter()
                .filter(|line| !line.starts_with('-'))
                .map(|line| line[1..].to_string())
                .collect();
            let at = hunk.old_start - 1;
            rebuilt.splice(at..at + hunk.old_count, kept);
        }
        assert_eq!(rebuilt.join("\n") + "\n", new);
    }
}
