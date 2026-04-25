set shell := ["nu", "-c"]

fw_target := "thumbv8m.main-none-eabihf"

_default:
    @just --list

# Run firmware binary on target board (build+flash via configured runner).
run bin="rp_touch":
    cargo run -p app --bin {{ bin }} --target {{ fw_target }} --release

# Run simulator.
sim bin="rp_touch":
    cargo run -p {{ bin }}_sim

# Run host GUI/desktop tool.
host:
    @cd tools/rp_touch_host; just run
