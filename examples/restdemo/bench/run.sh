#!/usr/bin/env bash
# Compare the Uredo service with its idiomatic Rust twin, on this machine, under one load
# generator, and report a distribution rather than a number.
#
# The expected result is **parity**: Uredo has no runtime, §36 retired the runtime budget because
# generated Rust performs like hand-written Rust, and the two programs are the same design. A gap
# either way is a finding about the compiler, not a feature of the language.
#
# Which is why this alternates A/B/A/B and runs **A against A** first. If two runs of the *same*
# binary differ as much as the two binaries differ, there is no signal, and that control is the
# only thing that makes a null result reportable.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
demo="$(dirname "$here")"
root="$(cd "$demo/../.." && pwd)"
path="${BENCH_PATH:-/items/1}"
conns="${BENCH_CONNS:-32}"
secs="${BENCH_SECS:-5}"
reps="${BENCH_REPS:-3}"
gen="$here/target/release/restdemo_bench"

echo "building, release, every side"
( cd "$here" && cargo build --release -q )
"$root/compiler/target/release/uredo" build --release "$demo" >/dev/null
( cd "$demo/idiomatic" && cargo build --release -q )
( cd "$demo/axum" && cargo build --release -q )
# `uredo build` lowers into `target/uredo` and points cargo's target-dir at the package's own
# `target/`, so the binary lands here.
ure_bin="$demo/target/release/restdemo"
rs_bin="$demo/idiomatic/target/release/restdemo_idiomatic"
ax_bin="$demo/axum/target/release/restdemo_axum"

# Which two binaries are being compared. The default is the pair this script was written for —
# Uredo against its hand-written Rust twin — but the framework comparison needs axum in one of
# the slots, and running it through a second copy of this script would be running it through a
# second set of controls. One tag word each, because the summary groups on it.
a_bin="${BENCH_A_BIN:-$ure_bin}"; a_tag="${BENCH_A_TAG:-uredo}"
b_bin="${BENCH_B_BIN:-$rs_bin}";  b_tag="${BENCH_B_TAG:-rust}"
[ -x "$a_bin" ] && [ -x "$b_bin" ] || { echo "missing a binary: $a_bin / $b_bin" >&2; exit 1; }

# Both binaries bind 127.0.0.1:8080 and neither takes a port, so they are run one at a time.
# The generator is multi-threaded and so is the server; left alone they compete for the same
# cores and the run-to-run spread swamps anything being measured. Each gets half the machine.
half=$(( $(nproc) / 2 ))
srv_cpus="0-$(( half - 1 ))"
gen_cpus="$half-$(( half * 2 - 1 ))"
pin() { if [ -n "$taskset_bin" ]; then "$taskset_bin" -c "$1" "${@:2}"; else "${@:2}"; fi; }
taskset_bin="$(command -v taskset || true)"

# Nothing from a previous run may still be holding cores. This is not hygiene, it is the
# measurement: `pin` used to be called with `&` and its `$!` recorded as the server's pid, but
# `$!` on a shell *function* is the subshell bash forks to run it, not the server that subshell
# then starts. Every `kill` in this script killed the wrapper. Servers accumulated across runs
# and across days — seventy of them were found alive, some twenty-six hours old — and the noise
# floor rose run by run until it was larger than anything being measured.
leftovers="$(pgrep -f "$demo/(target|idiomatic/target|axum/target)/release/restdemo" || true)"
if [ -n "$leftovers" ]; then
    echo "ABORT: restdemo servers from an earlier run are still alive:" >&2
    ps -o pid=,etimes=,args= -p $(echo "$leftovers" | tr "\n" " ") >&2
    echo "kill them before measuring; they hold cores and inflate the noise floor" >&2
    exit 1
fi

# A port nothing else is using. The first version of this script assumed 8080 was free. It was
# held by an unrelated admin service, both servers failed to bind, and the benchmark measured
# that service twice and reported the result as parity. Hence free_port, and hence identify.
free_port() {
    python3 -c "import socket; s=socket.socket(); s.bind(('127.0.0.1',0)); print(s.getsockname()[1]); s.close()"
}

# The control that was missing: prove the thing answering is the thing under test, before timing
# a single request through it.
identify() {
    local addr="$1" want="$2" health seeded
    health=$(curl -sS --max-time 2 "http://$addr/health" 2>/dev/null || true)
    case "$health" in
        *'"status":"ok"'*) ;;
        *) echo "  ABORT: nothing recognisable on $addr for $want (got: ${health:0:60})" >&2; return 1 ;;
    esac
    seeded=$(curl -sS --max-time 2 -o /dev/null -w '%{http_code}' "http://$addr$path" 2>/dev/null || true)
    [ "$seeded" = "200" ] || { echo "  ABORT: $path returned $seeded on $want; the benchmark would time an error path" >&2; return 1; }
}

# Kill a server and prove it died. The previous version asked politely and never looked.
stop() {
    local pid="$1" label="$2"
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.1
    done
    kill -9 "$pid" 2>/dev/null || true
    sleep 0.2
    if kill -0 "$pid" 2>/dev/null; then
        echo "ABORT: the server for $label ($pid) will not die; every later sample would be measured beside it" >&2
        exit 1
    fi
}

measure() {
    local bin="$1" label="$2" port addr
    port=$(free_port); addr="127.0.0.1:$port"
    # Started as a simple command rather than through `pin`, so that `$!` is the server itself:
    # bash execs the last command of a background simple command in place, but forks a subshell
    # for a function, and a pid that names the subshell cannot kill what the subshell started.
    local pid
    if [ -n "$taskset_bin" ]; then
        RESTDEMO_ADDR="$addr" "$taskset_bin" -c "$srv_cpus" "$bin" >/dev/null 2>&1 &
    else
        RESTDEMO_ADDR="$addr" "$bin" >/dev/null 2>&1 &
    fi
    pid=$!
    sleep 0.5
    curl -sS --max-time 2 -X POST -d '{"name":"bench"}' "http://$addr/items" >/dev/null 2>&1 || true
    if ! identify "$addr" "$label"; then
        stop "$pid" "$label"
        exit 1
    fi
    pin "$gen_cpus" "$gen" "$addr" "$path" "$conns" 1 >/dev/null 2>&1 || true
    local out
    out=$(pin "$gen_cpus" "$gen" "$addr" "$path" "$conns" "$secs")
    stop "$pid" "$label"
    printf '  %-22s %s\n' "$label" "$out"
    echo "$label $out" >> "$samples"
}

samples="$(mktemp)"
trap 'rm -f "$samples"' EXIT

echo
echo "path $path · $conns connections · ${secs}s · $reps repetitions"
echo "server on cpus $srv_cpus, generator on cpus $gen_cpus, of $(nproc)"
echo
echo "comparing $a_tag against $b_tag"
echo
echo "warming the machine, discarded"
measure "$a_bin" "warm-up" >/dev/null
measure "$b_bin" "warm-up" >/dev/null
: > "$samples"

echo "control: the same binary twice, to see the noise floor"
for i in $(seq 1 "$reps"); do
    measure "$a_bin" "$a_tag (control A$i)"
    measure "$a_bin" "$a_tag (control B$i)"
done
echo
echo "comparison: ABBA, so a monotonic ramp cancels instead of favouring the later slot"
# ABBA, not ABAB. The machine ramps: across a session both sides climb monotonically as
# frequency and caches warm, and under plain alternation the second binary always gets the later
# and faster slot — which is a systematic bias, not noise, and it showed up as a 12% "difference"
# that was really the ordering.
for i in $(seq 1 "$reps"); do
    if [ $(( i % 2 )) -eq 1 ]; then
        measure "$a_bin" "$a_tag  run $i"
        measure "$b_bin" "$b_tag  run $i"
    else
        measure "$b_bin" "$b_tag  run $i"
        measure "$a_bin" "$a_tag  run $i"
    fi
done
echo
python3 - "$samples" "$a_tag" "$b_tag" <<'PY'
import sys, re, statistics
rows = [l.split() for l in open(sys.argv[1]) if l.strip()]
def rps(r): return float(r[r.index('rps') + 1])
a_tag, b_tag = sys.argv[2], sys.argv[3]
ctl = [rps(r) for r in rows if 'control' in ' '.join(r)]
ure = [rps(r) for r in rows if r[0] == a_tag and 'control' not in ' '.join(r)]
rs  = [rps(r) for r in rows if r[0] == b_tag]
def line(name, v):
    print(f"  {name:<24} median {statistics.median(v):>7.0f}  min {min(v):>7.0f}  max {max(v):>7.0f}  "
          f"spread {100*(max(v)-min(v))/statistics.median(v):>5.1f}%")
print("summary, requests per second")
if ctl: line("control (same binary)", ctl)
if ure: line(a_tag, ure)
if rs:  line(b_tag, rs)
if ure and rs:
    # The paired difference is the statistic ABBA earns: each pair is measured minutes apart from
    # the next, so comparing pools of medians re-imports the drift the ordering was meant to cancel.
    pairs = [100 * (a - b) / b for a, b in zip(ure, rs)]
    d = statistics.median(pairs)
    wins = sum(1 for x in pairs if x > 0)
    print(f"\n  {a_tag} vs {b_tag}, median of per-pair differences: {d:+.1f}%")
    print(f"  per-pair range: {min(pairs):+.1f}% to {max(pairs):+.1f}%")
    print(f"  paired runs won by {a_tag}: {wins} of {len(pairs)}")
    if ctl:
        noise = 100 * (max(ctl) - min(ctl)) / statistics.median(ctl)
        print(f"  the same binary's own spread: {noise:.1f}%")
        print()
        print("  A difference smaller than that spread is noise." if abs(d) < noise
              else "  The difference exceeds the noise floor and is worth investigating.")
PY
