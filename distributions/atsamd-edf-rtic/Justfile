openocd:
	openocd -s scripts -f scripts/openocd.cfg

gdb:
	DEFMT_LOG=trace cargo b
	arm-none-eabi-gdb -x scripts/debug.gdb target/thumbv7em-none-eabihf/debug/edf-sw