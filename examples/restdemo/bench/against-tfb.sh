#!/usr/bin/env bash
# Build and run a TechEmpower front-runner locally, so the comparison is on this machine under
# this generator rather than between two numbers taken under different conditions.
#
# TechEmpower's own project was archived in 2026 with Round 23 as its last, so its published
# figures are now historical and a fresh number can only be taken locally.
#
# What differs from the upstream harness, and nothing else: the database host, because
# `tfb-database` is a name that exists only inside it, and the listen address.
set -euo pipefail

work="${1:-/tmp/tfb-local}"
fw="${FRAMEWORK:-may-minihttp}"
mkdir -p "$work"

echo "1. the benchmark database, with TechEmpower's own schema"
docker rm -f tfb-pg >/dev/null 2>&1 || true
docker run -d --name tfb-pg -e POSTGRES_PASSWORD=benchmarkdbpass -p 15432:5432 postgres:16-alpine >/dev/null
for _ in $(seq 1 30); do docker exec tfb-pg pg_isready -q 2>/dev/null && break; sleep 1; done
curl -sSL -o "$work/create.sql" \
  https://raw.githubusercontent.com/TechEmpower/FrameworkBenchmarks/master/toolset/databases/postgres/create-postgres.sql
docker exec -u postgres tfb-pg psql -q -c "CREATE USER benchmarkdbuser WITH PASSWORD 'benchmarkdbpass' SUPERUSER;"
docker exec -u postgres tfb-pg psql -q -c "CREATE DATABASE hello_world OWNER benchmarkdbuser;"
docker cp "$work/create.sql" tfb-pg:/tmp/create.sql >/dev/null
docker exec -u postgres tfb-pg psql -q -d hello_world -f /tmp/create.sql

echo "2. the framework's own source, from the archived repository"
if [ ! -d "$work/$fw" ]; then
    curl -sSL -o "$work/tfb.tgz" https://github.com/TechEmpower/FrameworkBenchmarks/archive/refs/heads/master.tar.gz
    tar xzf "$work/tfb.tgz" -C "$work" --wildcards "*/frameworks/Rust/$fw/*"
    mv "$work"/FrameworkBenchmarks-master/frameworks/Rust/"$fw" "$work/$fw"
    rm -rf "$work"/FrameworkBenchmarks-master "$work/tfb.tgz"
fi

echo "3. repoint the database host and the listen address, and build"
sed -i 's|@tfb-database/|@127.0.0.1:15432/|' "$work/$fw/src/main.rs"
sed -i 's|server.start("0.0.0.0:8080")|server.start(::std::env::var("TFB_ADDR").unwrap_or_else(\|_\| "0.0.0.0:8080".to_string()))|' "$work/$fw/src/main.rs"
( cd "$work/$fw" && cargo build --release -q )

echo
echo "built: $work/$fw/target/release/$fw"
echo "run it with TFB_ADDR=127.0.0.1:PORT and drive it with bench/target/release/restdemo_bench"
echo "stop the database with: docker rm -f tfb-pg"
