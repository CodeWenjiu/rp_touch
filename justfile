set shell := ["nu", "-c"]

_default:
    @just --list

# Run firmware binary on target board (build+flash via configured runner).
run bin="rp_touch":
    cargo run -p app --bin {{ bin }} --release

# Run simulator.
sim bin="rp_touch":
    cargo run -p {{ bin }}_sim

# Run host GUI/desktop tool.
host:
    cargo run -p rp_touch_host
