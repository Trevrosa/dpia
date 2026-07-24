# use a debug build
run *ARGS: (build ARGS)
    probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/debug/dpia
# use a release build
run-rel *ARGS: (build-rel ARGS)
    probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/dpia
# reset, then run with a debug build, assuming it's already flashed
attach:
    probe-rs reset --chip RP235x
    probe-rs attach --chip RP235x target/thumbv8m.main-none-eabihf/debug/dpia
# reset, then run with a release build, assuming it's already flashed
attach-rel:
    probe-rs reset --chip RP235x
    probe-rs attach --chip RP235x target/thumbv8m.main-none-eabihf/release/dpia
# run with a debug build, assuming it's already flashed
reattach:
    probe-rs attach --chip RP235x target/thumbv8m.main-none-eabihf/debug/dpia
# run with a release build, assuming it's already flashed
reattach-rel:
    probe-rs attach --chip RP235x target/thumbv8m.main-none-eabihf/release/dpia
# flash the cyw43 firmware
prepare-cyw43:
    probe-rs download cyw43-firmware/43439A0.bin --binary-format bin --chip RP235x --base-address 0x101b0000 --preverify
    probe-rs download cyw43-firmware/43439A0_btfw.bin --binary-format bin --chip RP235x --base-address 0x101f0000 --preverify
    probe-rs download cyw43-firmware/43439A0_clm.bin --binary-format bin --chip RP235x --base-address 0x101f2000 --preverify
    probe-rs download cyw43-firmware/nvram_rp2040.bin --binary-format bin --chip RP235x --base-address 0x101f4000 --preverify
# verify the cyw43 firmware
verify-cyw43:
    probe-rs verify cyw43-firmware/43439A0.bin --binary-format bin --chip RP235x --base-address 0x101b0000
    probe-rs verify cyw43-firmware/43439A0_btfw.bin --binary-format bin --chip RP235x --base-address 0x101f0000
    probe-rs verify cyw43-firmware/43439A0_clm.bin --binary-format bin --chip RP235x --base-address 0x101f2000
    probe-rs verify cyw43-firmware/nvram_rp2040.bin --binary-format bin --chip RP235x --base-address 0x101f4000
# use a debug build
flash *ARGS: (build ARGS)
    picotool load --update --verify --execute -t elf target/thumbv8m.main-none-eabihf/debug/dpia
# use a release build
flash-rel *ARGS: (build-rel ARGS)
    picotool load --update --verify --execute -t elf target/thumbv8m.main-none-eabihf/release/dpia
build *ARGS:
    cargo build {{ARGS}}
build-rel *ARGS:
    cargo build -r {{ARGS}}