# use a debug build
flash $DEFMT_LOG="info":
    cargo build --bin dpia
    probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/debug/dpia
# use a release build
flash-rel $DEFMT_LOG="info":
    cargo build --bin dpia -r
    probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/dpia
