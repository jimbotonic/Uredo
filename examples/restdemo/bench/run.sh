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

echo "building, release, both sides"
( cd "$here" && cargo build --release -q )
"$root/compiler/target/release/uredo" build --release "$demo" >/dev/null
( cd "$demo/idiomatic" && cargo build --release -q )
# `uredo build` lowers into `target/uredo` and points cargo's target-dir at the package's own
# `target/`, so the binary lands here.
ure_bin="$demo/target/release/restdemo"
rs_bin="$demo/idiomatic/target/release/restdemo_idiomatic"
[ -x "$ure_bin" ] && [ -x "$rs_bin" ] || { echo "missing a binary" >&2; exit 1; }

# Both binaries bind 127.0.0.1:8080 and neither takes a port, so they are run one at a time.
# The generator is multi-threaded and so is the server; left alone they compete for the same
# cores and the run-to-run spread swamps anything being measured. Each gets half the machine.
half=$(( $(nproc) / 2 ))
srv_cpus="0-$(( half - 1 ))"
gen_cpus="$half-$(( half * 2 - 1 ))"
pin() { if command -v taskset >/dev/null; then taskset -c "$1" "${@:2}"; else "${@:2}"; fi; }

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

measure() {
    local bin="$1" label="$2" port addr
    port=$(free_port); addr="127.0.0.1:$port"
    RESTDEMO_ADDR="$addr" pin "$srv_cpus" "$bin" >/dev/null 2>&1 &
    local pid=$!
    sleep 0.5
    curl -sS --max-time 2 -X POST -d '{"name":"bench"}' "http://$addr/items" >/dev/null 2>&1 || true
    if ! identify "$addr" "$label"; then
        kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true
        exit 1
    fi
    pin "$gen_cpus" "$gen" "$addr" "$path" "$conns" 1 >/dev/null 2>&1 || true
    local out
    out=$(pin "$gen_cpus" "$gen" "$addr" "$path" "$conns" "$secs")
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    sleep 0.2
    printf '  %-22s %s\n' "$label" "$out"
    echo "$label $out" >> "$samples"
}

samples="$(mktemp)"
trap 'rm -f "$samples"' EXIT

echo
echo "path $path · $conns connections · ${secs}s · $reps repetitions"
echo "server on cpus $srv_cpus, generator on cpus $gen_cpus, of $(nproc)"
echo
echo "warming the machine, discarded"
measure "$ure_bin" "warm-up" >/dev/null
measure "$rs_bin"  "warm-up" >/dev/null
: > "$samples"

echo "control: the same binary twice, to see the noise floor"
for i in $(seq 1 "$reps"); do
    measure "$ure_bin" "uredo (control A$i)"
    measure "$ure_bin" "uredo (control B$i)"
done
echo
echo "comparison: ABBA, so a monotonic ramp cancels instead of favouring the later slot"
# ABBA, not ABAB. The machine ramps: across a session both sides climb monotonically as
# frequency and caches warm, and under plain alternation the second binary always gets the later
# and faster slot — which is a systematic bias, not noise, and it showed up as a 12% "difference"
# that was really the ordering.
for i in $(seq 1 "$reps"); do
    if [ $(( i % 2 )) -eq 1 ]; then
        measure "$ure_bin" "uredo      run $i"
        measure "$rs_bin"  "rust twin  run $i"
    else
        measure "$rs_bin"  "rust twin  run $i"
        measure "$ure_bin" "uredo      run $i"
    fi
done
echo
python3 - "$samples" <<'PY'
import sys, re, statistics
rows = [l.split() for l in open(sys.argv[1]) if l.strip()]
def rps(r): return float(r[r.index('rps') + 1])
ctl = [rps(r) for r in rows if 'control' in ' '.join(r)]
ure = [rps(r) for r in rows if r[0] == 'uredo' and 'control' not in ' '.join(r)]
rs  = [rps(r) for r in rows if r[0] == 'rust']
def line(name, v):
    print(f"  {name:<24} median {statistics.median(v):>7.0f}  min {min(v):>7.0f}  max {max(v):>7.0f}  "
          f"spread {100*(max(v)-min(v))/statistics.median(v):>5.1f}%")
print("summary, requests per second")
if ctl: line("control (same binary)", ctl)
if ure: line("uredo", ure)
if rs:  line("rust twin", rs)
if ure and rs:
    # The paired difference is the statistic ABBA earns: each pair is measured minutes apart from
    # the next, so comparing pools of medians re-imports the drift the ordering was meant to cancel.
    pairs = [100 * (a - b) / b for a, b in zip(ure, rs)]
    d = statistics.median(pairs)
    wins = sum(1 for x in pairs if x > 0)
    print(f"\n  uredo vs rust twin, median of per-pair differences: {d:+.1f}%")
    print(f"  per-pair range: {min(pairs):+.1f}% to {max(pairs):+.1f}%")
    print(f"  paired runs won by uredo:    {wins} of {len(pairs)}")
    if ctl:
        noise = 100 * (max(ctl) - min(ctl)) / statistics.median(ctl)
        print(f"  the same binary's own spread: {noise:.1f}%")
        print()
        print("  A difference smaller than that spread is noise." if abs(d) < noise
              else "  The difference exceeds the noise floor and is worth investigating.")
PY
