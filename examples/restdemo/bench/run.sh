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

measure() {
    local bin="$1" label="$2"
    pin "$srv_cpus" "$bin" >/dev/null 2>&1 &
    local pid=$!
    sleep 0.4
    curl -sS -X POST -d '{"name":"bench"}' http://127.0.0.1:8080/items >/dev/null 2>&1 || true
    pin "$gen_cpus" "$gen" 127.0.0.1:8080 "$path" "$conns" 1 >/dev/null 2>&1 || true  # warm-up
    local out
    out=$(pin "$gen_cpus" "$gen" 127.0.0.1:8080 "$path" "$conns" "$secs")
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
echo "control: the same binary twice, to see the noise floor"
for i in $(seq 1 "$reps"); do
    measure "$ure_bin" "uredo (control A$i)"
    measure "$ure_bin" "uredo (control B$i)"
done
echo
echo "comparison: alternating, so drift falls on both sides equally"
for i in $(seq 1 "$reps"); do
    measure "$ure_bin" "uredo      run $i"
    measure "$rs_bin"  "rust twin  run $i"
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
    d = 100 * (statistics.median(ure) - statistics.median(rs)) / statistics.median(rs)
    wins = sum(1 for a, b in zip(ure, rs) if a > b)
    print(f"\n  uredo vs rust twin, medians: {d:+.1f}%")
    print(f"  paired runs won by uredo:    {wins} of {len(ure)}")
    if ctl:
        noise = 100 * (max(ctl) - min(ctl)) / statistics.median(ctl)
        print(f"  the same binary's own spread: {noise:.1f}%")
        print()
        print("  A difference smaller than that spread is noise." if abs(d) < noise
              else "  The difference exceeds the noise floor and is worth investigating.")
PY
