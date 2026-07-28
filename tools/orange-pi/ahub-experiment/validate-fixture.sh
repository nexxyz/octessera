#!/bin/sh
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
PYTHON=${PYTHON:-python3}

die() {
	printf '%s\n' "AHUB fixture: $*" >&2
	exit 1
}

command -v "$PYTHON" >/dev/null 2>&1 || die "python3 is required"
"$PYTHON" "$HERE/preflight.py" "$HERE"

for tool in dtc fdtoverlay fdtget; do
	command -v "$tool" >/dev/null 2>&1 || die "$tool is required"
done

TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/octessera-ahub-fixture.XXXXXX")
trap 'rm -rf "$TMP_DIR"' EXIT HUP INT TERM
BASE_DTB=$TMP_DIR/base.dtb
OVERLAY_DTBO=$TMP_DIR/overlay.dtbo
MERGED_DTB=$TMP_DIR/merged.dtb

dtc -@ -I dts -O dtb -o "$BASE_DTB" "$HERE/h618-fixture-base.dts"
dtc -@ -I dts -O dtb -o "$OVERLAY_DTBO" "$HERE/octessera-ahub0-pi123-overlay.dts"

expect_root_absent() {
	dtb=$1
	node=$2
	label=$3
	if fdtget -l "$dtb" / | grep -Fxq "$node"; then
		die "$label: unexpected root node"
	fi
}

for node in ahub1_plat ahub1_mach octessera_plat pcm5102a octessera-dac; do
	expect_root_absent "$BASE_DTB" "$node" "base $node"
done

fdtoverlay -i "$BASE_DTB" -o "$MERGED_DTB" "$OVERLAY_DTBO"

expect() {
	actual=$1
	expected=$2
	label=$3
	[ "$actual" = "$expected" ] || die "$label: expected '$expected', got '$actual'"
}

string_prop() {
	fdtget -t s "$1" "$2" "$3"
}

value_prop() {
	fdtget "$1" "$2" "$3"
}

expect_absent() {
	dtb=$1
	node=$2
	property=$3
	label=$4
	if fdtget "$dtb" "$node" "$property" >/dev/null 2>&1; then
		die "$label: unexpected property"
	fi
}

expect_property() {
	dtb=$1
	node=$2
	property=$3
	label=$4
	fdtget -p "$dtb" "$node" | grep -Fxq "$property" || die "$label: missing property"
}

expect_child() {
	dtb=$1
	node=$2
	child=$3
	label=$4
	fdtget -l "$dtb" "$node" | grep -Fxq "$child" || die "$label: missing child"
}

expect "$(string_prop "$MERGED_DTB" /octessera-dac soundcard-mach,name)" "octessera-dac" "card name"
expect "$(string_prop "$MERGED_DTB" /octessera-dac compatible)" "allwinner,sunxi-snd-mach" "card compatible"
expect_root_absent "$MERGED_DTB" pcm5102a "merged pcm5102a"
expect_property "$MERGED_DTB" /octessera-dac soundcard-mach,playback-only "playback-only card"
expect_child "$MERGED_DTB" /octessera-dac soundcard-mach,codec "dummy codec child"
expect_absent "$MERGED_DTB" /octessera-dac/soundcard-mach,codec sound-dai "dummy codec sound-dai"
expect "$(value_prop "$MERGED_DTB" /octessera_plat status)" "okay" "AHUB0 platform status"
expect "$(value_prop "$MERGED_DTB" /octessera_plat apb_num)" "0" "AHUB0 APB number"
expect "$(value_prop "$MERGED_DTB" /octessera_plat tdm_num)" "0" "AHUB0 TDM number"
dma_values=$(fdtget -t x "$MERGED_DTB" /octessera_plat dmas)
IFS=' ' read -r dma_tx dma_req_tx dma_rx dma_req_rx <<EOF
$dma_values
EOF
expect "$dma_req_tx" "3" "AHUB0 TX DMA request"
expect "$dma_req_rx" "3" "AHUB0 RX DMA request"
expect "$dma_tx" "$dma_rx" "AHUB0 DMA controller"
pinctrl_values=$(fdtget -t x "$MERGED_DTB" /octessera_plat pinctrl-0)
IFS=' ' read -r pinctrl_i2s0 pinctrl_dout0 <<EOF
$pinctrl_values
EOF
expect "$pinctrl_i2s0" "$(fdtget -t x "$MERGED_DTB" /pinctrl@300b000/ahub0-pins phandle)" "AHUB0 I2S pinctrl ownership"
expect "$pinctrl_dout0" "$(fdtget -t x "$MERGED_DTB" /pinctrl@300b000/ahub0-dout0-pins phandle)" "AHUB0 DOUT0 pinctrl ownership"
expect "$(string_prop "$MERGED_DTB" /pinctrl@300b000/ahub0-pins function)" "i2s0" "I2S function"
expect "$(string_prop "$MERGED_DTB" /pinctrl@300b000/ahub0-pins pins)" "PI1 PI2" "I2S pins"
expect "$(string_prop "$MERGED_DTB" /pinctrl@300b000/ahub0-dout0-pins function)" "i2s0_dout0" "I2S DOUT0 function"
expect "$(string_prop "$MERGED_DTB" /pinctrl@300b000/ahub0-dout0-pins pins)" "PI3" "I2S DOUT0 pin"
expect "$(value_prop "$MERGED_DTB" /octessera-dac status)" "okay" "octessera-dac machine status"
cpu_phandle=$(value_prop "$MERGED_DTB" /octessera-dac/soundcard-mach,cpu phandle)
expect "$(value_prop "$MERGED_DTB" /octessera-dac/soundcard-mach,cpu sound-dai)" "$(value_prop "$MERGED_DTB" /octessera_plat phandle)" "octessera-dac CPU ownership"
expect "$(value_prop "$MERGED_DTB" /octessera-dac soundcard-mach,frame-master)" "$cpu_phandle" "octessera-dac frame master"
expect "$(value_prop "$MERGED_DTB" /octessera-dac soundcard-mach,bitclock-master)" "$cpu_phandle" "octessera-dac bitclock master"
expect_absent "$MERGED_DTB" /octessera-dac soundcard-mach,mclk-fs "octessera-dac mclk-fs"
expect_absent "$MERGED_DTB" /octessera-dac/soundcard-mach,cpu soundcard-mach,mclk-fs "octessera-dac CPU mclk-fs"
expect "$(value_prop "$MERGED_DTB" /soc/ahub1_plat status)" "okay" "preserved HDMI platform status"
expect "$(value_prop "$MERGED_DTB" /soc/ahub1_plat tdm_num)" "1" "preserved HDMI TDM number"
expect "$(string_prop "$MERGED_DTB" /soc/ahub1_mach soundcard-mach,name)" "HDMI" "preserved HDMI card"
expect "$(value_prop "$MERGED_DTB" /soc/ahub1_mach/soundcard-mach,cpu sound-dai)" "$(value_prop "$MERGED_DTB" /soc/ahub1_plat phandle)" "preserved HDMI CPU ownership"
expect "$(value_prop "$MERGED_DTB" /ahub_dam_mach status)" "disabled" "disabled AHUB DAM machine"
expect "$(string_prop "$MERGED_DTB" /main-encoder pins)" "PI0" "preserved PI0 encoder"

for node in ahub1_plat ahub1_mach; do
	expect_root_absent "$MERGED_DTB" "$node" "merged $node"
done

for node in /serial@5000000 /serial@5002000 /spi@5010000 /i2c@5003000 /hdmi@6000000 /codec@5096000 /main-encoder; do
	expect "$(value_prop "$MERGED_DTB" "$node" status)" "$(value_prop "$BASE_DTB" "$node" status)" "preserved $node status"
	expect "$(string_prop "$MERGED_DTB" "$node" compatible)" "$(string_prop "$BASE_DTB" "$node" compatible)" "preserved $node compatible"
done
expect "$(string_prop "$MERGED_DTB" /soc/ahub1_mach compatible)" "$(string_prop "$BASE_DTB" /soc/ahub1_mach compatible)" "preserved HDMI machine compatible"

base_children=$(fdtget -l "$BASE_DTB" / | LC_ALL=C sort)
merged_children=$(fdtget -l "$MERGED_DTB" / | LC_ALL=C sort)
expected_children=$(printf '%s\noctessera_plat\noctessera-dac\n' "$base_children" | LC_ALL=C sort)
expect "$merged_children" "$expected_children" "root child set"

printf '%s\n' 'compiled and merged AHUB fixture passed'
