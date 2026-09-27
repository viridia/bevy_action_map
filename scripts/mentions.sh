#!/usr/bin/env bash
# Every mention of a pattern in the tree, with the exclusions a sweep always wants, so a sweep for a
# withdrawn name or a moved decision is the same call each time.
#
# `git grep --untracked` honours `.gitignore`, which covers `target/` in every workspace, and still
# sees files not yet added. `archive/` is excluded here: nothing in flight reasons from it.
#
# Output is `file:line:text`, a blank line between files, each line cut at WIDTH characters so one
# long table row cannot flood the transcript. The count goes to stderr.
#
# Usage: scripts/mentions.sh [git-grep options] <pattern>
#   The pattern is a Perl-compatible regular expression. Options such as -i or -w pass through.
#   Exits 1 when there is no match, as grep does.

set -uo pipefail
cd "$(dirname "$0")/.."

WIDTH=160

if [[ $# -lt 1 ]]; then
    echo "usage: $0 [git-grep options] <pattern>" >&2
    exit 2
fi

pattern="${!#}"
opts=("${@:1:$#-1}")

# Perl syntax rather than -E: macOS's POSIX regex gives `\b` no meaning, and matches nothing.
out="$(git grep --untracked -I -n -P --break --color=never "${opts[@]+"${opts[@]}"}" \
    -e "${pattern}" -- . ':!archive/')"
status=$?
if [[ ${status} -ne 0 ]]; then
    exit "${status}"
fi

# A UTF-8 locale makes `cut -c` count characters, so an em-dash is never split.
printf '%s\n' "${out}" | LC_ALL=en_US.UTF-8 cut -c "1-${WIDTH}"

lines=$(printf '%s\n' "${out}" | grep -c .)
files=$(printf '%s\n' "${out}" | grep . | cut -d: -f1 | sort -u | wc -l | tr -d ' ')
echo "${lines} lines in ${files} files" >&2
