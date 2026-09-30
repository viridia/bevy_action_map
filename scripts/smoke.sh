#!/usr/bin/env bash
# Launches each named example for a bounded time and fails it on a panic, an `ERROR` line, an
# unexpected `WARN` line, or an early exit. This is the launch CLAUDE.md asks of every example a
# chunk touches, and a clean run prints one line.
#
# Usage: scripts/smoke.sh [--secs N] <example>...
#   --secs N  how long each example runs once built, overriding `secs_for`.
#
# Every windowed example logs all it will within about a second of starting (measured September
# 2026), so the default run is short, and longer only for an example with timed behaviour of its
# own, where a timer that goes wrong would panic partway through.
#
# The build happens before the clock starts, so a cold build cannot eat the run. macOS has no
# `timeout`, so the bound is kept here: the example runs in its own process group, and the whole
# group is signalled, since killing `cargo run` alone would leave the game running.

set -uo pipefail
cd "$(dirname "$0")/.."

secs=""
declare -a examples=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --secs)
            secs="${2:?--secs needs a value}"
            shift 2
            ;;
        -*)
            echo "usage: $0 [--secs N] <example>..." >&2
            exit 2
            ;;
        *)
            examples+=("$1")
            shift
            ;;
    esac
done
if [[ ${#examples[@]} -eq 0 ]]; then
    echo "usage: $0 [--secs N] <example>..." >&2
    exit 2
fi

# The features an example needs beyond the defaults. Cargo refuses to build one without them, so an
# example missing here fails loudly rather than silently.
features_for() {
    case "$1" in
        disasteroids | split_friction) echo "--features serialize" ;;
        *) echo "" ;;
    esac
}

secs_for() {
    case "$1" in
        pong_countdown | pong_robot | disasteroids) echo 30 ;;
        *) echo 10 ;;
    esac
}

# Examples that print and exit rather than open a window, for which exiting cleanly is passing.
one_shot() {
    case "$1" in
        diagnostics) return 0 ;;
        *) return 1 ;;
    esac
}

# `WARN` lines known to be harmless, as `<example>|<pattern>`, the pattern matching the `WARN` line
# itself. One that does not appear is reported as stale, which is how a Bevy bump that fixes it
# shows up; it is a notice rather than a failure, since a warning can be intermittent.
known_warnings=(
    # Bevy's, bevy#25936: `mesh2d::bindings` unresolved, on the next line. Seen on some runs only.
    "split_friction|sprite_material.wesl\` has an unresolved import"
)

log_dir=$(mktemp -d "${TMPDIR:-/tmp}/smoke.XXXXXX")
failed=0

set -m # each background job gets its own process group
for example in "${examples[@]}"; do
    log="${log_dir}/${example}.log"
    # shellcheck disable=SC2046 # the feature flags are meant to split
    if ! build=$(cargo build --quiet --example "${example}" $(features_for "${example}") 2>&1); then
        printf '%s\n' "${build}"
        echo "FAILED (build): ${example}"
        failed=1
        continue
    fi

    # shellcheck disable=SC2046
    NO_COLOR=1 cargo run --quiet --example "${example}" $(features_for "${example}") \
        >"${log}" 2>&1 &
    pid=$!

    run_secs="${secs:-$(secs_for "${example}")}"
    elapsed=0
    while kill -0 "${pid}" 2>/dev/null && [[ ${elapsed} -lt ${run_secs} ]]; do
        sleep 1
        elapsed=$((elapsed + 1))
    done

    early=""
    if kill -0 "${pid}" 2>/dev/null; then
        kill -TERM -- "-${pid}" 2>/dev/null
        wait "${pid}" 2>/dev/null
    else
        wait "${pid}"
        status=$?
        if ! one_shot "${example}" || [[ ${status} -ne 0 ]]; then
            early="exited after ${elapsed}s with status ${status}"
        fi
    fi

    # Known warnings for this example, and which of them turned up.
    declare -a patterns=()
    for entry in "${known_warnings[@]}"; do
        [[ "${entry%%|*}" == "${example}" ]] && patterns+=("${entry#*|}")
    done
    problems=$(grep -E 'panicked|ERROR|WARN' "${log}" || true)
    for pattern in "${patterns[@]+"${patterns[@]}"}"; do
        if grep -q -- "${pattern}" <<<"${problems}"; then
            problems=$(grep -v -- "${pattern}" <<<"${problems}" || true)
        else
            echo "stale: ${example} no longer logs the known warning '${pattern}'"
        fi
    done
    unset patterns

    if [[ -n "${early}" || -n "${problems}" ]]; then
        [[ -n "${problems}" ]] && printf '%s\n' "${problems}"
        [[ -n "${early}" ]] && echo "${early}"
        echo "FAILED: ${example} (log: ${log})"
        failed=1
    else
        echo "ok: ${example} ran ${elapsed}s"
    fi
done

exit "${failed}"
