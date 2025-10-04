# use a debug build
run: build
    probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/debug/dpia
# use a release build
run-rel: build-rel
    probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/dpia
# flash the cyw43 firmware
prepare-cyw43:
    probe-rs download cyw43-firmware/43439A0.bin --binary-format bin --chip RP235x --base-address 0x101b0000
    probe-rs download cyw43-firmware/43439A0_btfw.bin --binary-format bin --chip RP235x --base-address 0x101f0000
    probe-rs download cyw43-firmware/43439A0_clm.bin --binary-format bin --chip RP235x --base-address 0x101f8000
# use a debug build
flash: build
    picotool load --update --verify --execute -t elf target/thumbv8m.main-none-eabihf/debug/dpia
# use a release build
flash-rel: build-rel
    picotool load --update --verify --execute -t elf target/thumbv8m.main-none-eabihf/release/dpia
build:
    cargo build --bin dpia
build-rel:
    cargo build --bin dpia -r
