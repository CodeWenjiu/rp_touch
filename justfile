set shell := ["powershell.exe", "-NoProfile", "-Command"]

fw_target := "thumbv8m.main-none-eabihf"
host_target := "x86_64-pc-windows-msvc"

_default:
    @just --list

# Run firmware binary on target board (build+flash via configured runner).
run bin="rp_touch":
    cargo run -p app --bin {{ bin }} --target {{ fw_target }} --release

# Run host simulator (workspace member: <bin>_sim package).
sim bin="rp_touch":
    cargo run -p {{ bin }}_sim --target {{ host_target }}
