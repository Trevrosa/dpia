# use a debug build
run: build
    probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/debug/dpia
# use a release build
run-rel: build-rel
    probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/dpia
# use a debug build
flash $DEFMT_LOG="info": build
    picotool load --update --verify --execute -t elf target/thumbv8m.main-none-eabihf/debug/dpia
# use a release build
flash-rel $DEFMT_LOG="info": build-rel
    picotool load --update --verify --execute -t elf target/thumbv8m.main-none-eabihf/release/dpia
build:
    cargo build --bin dpia
build-rel:
    cargo build --bin dpia -r
