#!/bin/sh
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
PYTHON=${PYTHON:-python3}
SSH_BIN=${SSH_BIN:-ssh}
SCP_BIN=${SCP_BIN:-scp}
SSH_BATCH_MODE=${SSH_BATCH_MODE:-yes}
ACTION=${1:-}
TARGET=
DTB_PATH=
DTB_DIR=
OVERLAY_DIR=
OVERLAY_PATH=
BACKUP_ID=
CONFIRM=0

die() {
	printf '%s\n' "AHUB deploy: $*" >&2
	exit 1
}

usage() {
	cat >&2 <<'EOF'
usage:
  deploy-rollback.sh preflight --target octessera@192.168.0.217 --dtb /boot/dtb-<kernel>/allwinner/sun50i-h618-orangepi-zero2w.dtb
  deploy-rollback.sh deploy --yes --target octessera@192.168.0.217 --dtb /boot/dtb-<kernel>/allwinner/sun50i-h618-orangepi-zero2w.dtb
  deploy-rollback.sh rollback --yes --target octessera@192.168.0.217 --dtb /boot/dtb-<kernel>/allwinner/sun50i-h618-orangepi-zero2w.dtb --backup-id ID
EOF
	exit 2
}

case "$SSH_BATCH_MODE" in
	yes|no) ;;
	*) die "SSH_BATCH_MODE must be exactly yes or no" ;;
esac

[ -n "$ACTION" ] || usage
shift
while [ "$#" -gt 0 ]; do
	case "$1" in
		--target)
			[ "$#" -ge 2 ] || die "--target needs a value"
			TARGET=$2
			shift 2
			;;
		--dtb)
			[ "$#" -ge 2 ] || die "--dtb needs a value"
			DTB_PATH=$2
			shift 2
			;;
		--backup-id)
			[ "$#" -ge 2 ] || die "--backup-id needs a value"
			BACKUP_ID=$2
			shift 2
			;;
		--yes)
			CONFIRM=1
			shift
			;;
		*) usage ;;
	esac
done

[ -n "$TARGET" ] || die "an explicit --target is required"
[ -n "$DTB_PATH" ] || die "an explicit --dtb is required"
case "$DTB_PATH" in
	/boot/dtb/allwinner/sun50i-h618-orangepi-zero2w.dtb) ;;
	/boot/dtb-*/allwinner/sun50i-h618-orangepi-zero2w.dtb)
		DTB_TREE=${DTB_PATH#/boot/}
		DTB_TREE=${DTB_TREE%%/allwinner/*}
		case "$DTB_TREE" in
			dtb-[0-9A-Za-z]*) ;;
			*) die "--dtb must use a versioned Armbian DTB tree" ;;
			esac
		case "$DTB_TREE" in
			*/*) die "--dtb kernel tree must be one path component" ;;
		esac
		;;
	*) die "--dtb must end in /allwinner/sun50i-h618-orangepi-zero2w.dtb" ;;
esac
case "$DTB_PATH" in
	*[!A-Za-z0-9._/-]*) die "--dtb contains unsafe characters" ;;
esac
DTB_DIR=${DTB_PATH%/*}
OVERLAY_DIR=$DTB_DIR/overlay
OVERLAY_PATH=$OVERLAY_DIR/octessera-ahub0-pcm5102.dtbo
case "$TARGET" in
	?*@?*) ;;
	*) die "--target must be a nonempty user@host value" ;;
esac
case "$TARGET" in
	*@*@*) die "--target must contain exactly one user@host separator" ;;
	*[!A-Za-z0-9._@:-]*) die "--target contains unsafe characters" ;;
esac
case "$ACTION" in
	preflight|deploy|rollback) ;;
	*) usage ;;
esac
[ "$ACTION" = preflight ] || [ "$CONFIRM" -eq 1 ] || die "deploy and rollback require explicit --yes"
if [ "$ACTION" = rollback ]; then
	[ -n "$BACKUP_ID" ] || die "rollback requires --backup-id"
	case "$BACKUP_ID" in
		*[!A-Za-z0-9._-]*|'' ) die "backup ID contains unsafe characters" ;;
	esac
fi

command -v "$SSH_BIN" >/dev/null 2>&1 || die "SSH_BIN command is unavailable: $SSH_BIN"
command -v "$SCP_BIN" >/dev/null 2>&1 || die "SCP_BIN command is unavailable: $SCP_BIN"
command -v "$PYTHON" >/dev/null 2>&1 || die "python3 is required"

quote() {
	printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

remote_exec() {
	command=$1
	"$SSH_BIN" -o "BatchMode=$SSH_BATCH_MODE" -o ConnectTimeout=10 -- "$TARGET" "sudo -n sh -c $(quote "$command")"
}

prepare_artifact() {
	"$HERE/validate-fixture.sh"
	TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/octessera-ahub-deploy.XXXXXX")
	trap 'rm -rf "$TMP_DIR"' EXIT HUP INT TERM
	OVERLAY_DTBO=$TMP_DIR/octessera-ahub0-pcm5102.dtbo
	dtc -@ -I dts -O dtb -o "$OVERLAY_DTBO" "$HERE/octessera-ahub0-pi123-overlay.dts"
}

remote_identity_and_merge() {
	REMOTE_STAGE=/tmp/octessera-ahub0-pcm5102.$$.dtbo
	"$SCP_BIN" -o "BatchMode=$SSH_BATCH_MODE" -o ConnectTimeout=10 -- "$OVERLAY_DTBO" "$TARGET:$REMOTE_STAGE"
	remote_exec "set -eu
test \"\$(grep \"^BOARD=\" /etc/armbian-release | cut -d= -f2 | sed -n \"1p\")\" = orangepizero2w
test -r '$DTB_PATH'
test -d '$OVERLAY_DIR'
command -v fdtget >/dev/null
command -v fdtoverlay >/dev/null
command -v sha256sum >/dev/null
config=/boot/config-\$(uname -r)
test -r \"\$config\"
for symbol in CONFIG_ARCH_SUNXI CONFIG_SOUND CONFIG_SND CONFIG_SND_SOC CONFIG_REGMAP_MMIO CONFIG_NVMEM_SUNXI_SID CONFIG_SND_SOC_GENERIC_DMAENGINE_PCM CONFIG_SND_SOC_PCM5102A CONFIG_SND_SOC_SUNXI_AHUB CONFIG_SND_SOC_SUNXI_AHUB_DAM CONFIG_SND_SOC_SUNXI_MACH; do
  grep -q \"^\$symbol=y$\" \"\$config\"
done
grep -q '^# CONFIG_SUNXI_SYS_INFO is not set$' \"\$config\"
compat=\$(fdtget -t s '$DTB_PATH' / compatible)
case \"\$compat\" in
  *xunlong,orangepi-zero2w*allwinner,sun50i-h618*|*allwinner,sun50i-h618*xunlong,orangepi-zero2w*) ;;
  *) exit 21 ;;
esac
root_children=\$(fdtget -l '$DTB_PATH' /)
for node in ahub1_plat ahub1_mach octessera_plat pcm5102a octessera-dac; do
  if printf '%s\n' \"\$root_children\" | grep -Fxq \"\$node\"; then
    exit 22
  fi
done
fdtoverlay -i '$DTB_PATH' -o '$REMOTE_STAGE.merged' '$REMOTE_STAGE'
merged_root_children=\$(fdtget -l '$REMOTE_STAGE.merged' /)
for node in ahub1_plat ahub1_mach; do
  if printf '%s\n' \"\$merged_root_children\" | grep -Fxq \"\$node\"; then
    exit 23
  fi
done
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera_plat status)\" = okay
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera_plat apb_num)\" = 0
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera_plat tdm_num)\" = 0
dma_values=\$(fdtget -t x '$REMOTE_STAGE.merged' /octessera_plat dmas)
set -- \$dma_values
test \"\$#\" = 4
test \"\$2\" = 3
test \"\$4\" = 3
test \"\$1\" = \"\$3\"
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera_plat pinctrl-0)\" = \"\$(fdtget '$REMOTE_STAGE.merged' /pinctrl@300b000/ahub0-pins phandle)\"
test \"\$(fdtget -t s '$REMOTE_STAGE.merged' /pinctrl@300b000/ahub0-pins function)\" = i2s0
test \"\$(fdtget -t s '$REMOTE_STAGE.merged' /pinctrl@300b000/ahub0-pins pins)\" = \"PI1 PI2 PI3\"
test \"\$(fdtget -t s '$REMOTE_STAGE.merged' /pcm5102a compatible)\" = ti,pcm5102a
test \"\$(fdtget '$REMOTE_STAGE.merged' /pcm5102a status)\" = okay
test \"\$(fdtget -t s '$REMOTE_STAGE.merged' /octessera-dac soundcard-mach,name)\" = octessera-dac
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera-dac status)\" = okay
cpu_phandle=\$(fdtget '$REMOTE_STAGE.merged' /octessera-dac/soundcard-mach,cpu phandle)
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera-dac/soundcard-mach,cpu sound-dai)\" = \"\$(fdtget '$REMOTE_STAGE.merged' /octessera_plat phandle)\"
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera-dac/soundcard-mach,codec sound-dai)\" = \"\$(fdtget '$REMOTE_STAGE.merged' /pcm5102a phandle)\"
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera-dac soundcard-mach,frame-master)\" = \"\$cpu_phandle\"
test \"\$(fdtget '$REMOTE_STAGE.merged' /octessera-dac soundcard-mach,bitclock-master)\" = \"\$cpu_phandle\"
if fdtget '$REMOTE_STAGE.merged' /octessera-dac soundcard-mach,mclk-fs >/dev/null 2>&1 || fdtget '$REMOTE_STAGE.merged' /octessera-dac/soundcard-mach,cpu soundcard-mach,mclk-fs >/dev/null 2>&1; then
  exit 24
fi
test \"\$(fdtget '$REMOTE_STAGE.merged' /soc/ahub1_plat status)\" = okay
test \"\$(fdtget '$REMOTE_STAGE.merged' /soc/ahub1_plat tdm_num)\" = 1
test \"\$(fdtget -t s '$REMOTE_STAGE.merged' /soc/ahub1_mach soundcard-mach,name)\" = HDMI
test \"\$(fdtget '$REMOTE_STAGE.merged' /soc/ahub1_mach/soundcard-mach,cpu sound-dai)\" = \"\$(fdtget '$REMOTE_STAGE.merged' /soc/ahub1_plat phandle)\"
test \"\$(fdtget '$REMOTE_STAGE.merged' /ahub_dam_mach status)\" = disabled
rm -f '$REMOTE_STAGE.merged'"
}

cleanup_stage() {
	remote_exec "rm -f '$REMOTE_STAGE' '$REMOTE_STAGE.merged'" || true
}

make_backup() {
	BACKUP_ID=$(date -u +%Y%m%dT%H%M%SZ)
	remote_exec "set -eu
backup=/boot/.octessera-ahub-experiment/$BACKUP_ID
test ! -e \"\$backup\"
mkdir -p \"\$backup\"
printf '%s\\n' schema=1 target=$TARGET dtb_path=$DTB_PATH overlay_path=$OVERLAY_PATH env_path=/boot/armbianEnv.txt > \"\$backup/manifest\"
if test -e /boot/armbianEnv.txt; then
  cp -a /boot/armbianEnv.txt \"\$backup/armbianEnv.txt\"
  printf '%s\\n' env_state=present >> \"\$backup/manifest\"
  printf 'env_sha256=%s\\n' \"\$(sha256sum /boot/armbianEnv.txt | cut -d \" \" -f1)\" >> \"\$backup/manifest\"
else
  printf '%s\\n' env_state=absent env_sha256=ABSENT >> \"\$backup/manifest\"
fi
if test -e '$DTB_PATH'; then
  cp -a '$DTB_PATH' \"\$backup/dtb\"
  printf '%s\\n' dtb_state=present >> \"\$backup/manifest\"
  printf 'dtb_sha256=%s\\n' \"\$(sha256sum '$DTB_PATH' | cut -d \" \" -f1)\" >> \"\$backup/manifest\"
else
  printf '%s\\n' dtb_state=absent dtb_sha256=ABSENT >> \"\$backup/manifest\"
fi
if test -e '$OVERLAY_PATH'; then
  cp -a '$OVERLAY_PATH' \"\$backup/overlay.dtbo\"
  printf '%s\\n' overlay_state=present >> \"\$backup/manifest\"
  printf 'overlay_sha256=%s\\n' \"\$(sha256sum '$OVERLAY_PATH' | cut -d \" \" -f1)\" >> \"\$backup/manifest\"
else
  printf '%s\\n' overlay_state=absent overlay_sha256=ABSENT >> \"\$backup/manifest\"
fi
sha256sum \"\$backup/manifest\" > \"\$backup/manifest.sha256\"
chmod 600 \"\$backup/manifest\" \"\$backup/manifest.sha256\""
}

install_artifact() {
	remote_exec "set -eu
backup=/boot/.octessera-ahub-experiment/$BACKUP_ID
test -f \"\$backup/manifest\"
test -f /boot/armbianEnv.txt
install -m 0644 '$REMOTE_STAGE' '$OVERLAY_PATH'
test \"\$(grep -c \"^overlays=\" /boot/armbianEnv.txt || true)\" -le 1
if grep -Eq \"^overlays=([^[:space:]]+[[:space:]]+)*octessera-ahub0-pcm5102([[:space:]]|$)\" /boot/armbianEnv.txt; then
  exit 42
fi
sed \"/^overlays=/s/$/ octessera-ahub0-pcm5102/\" /boot/armbianEnv.txt > /boot/armbianEnv.txt.octessera.tmp
if ! grep -q \"^overlays=\" /boot/armbianEnv.txt; then
  printf '%s\\n' overlays=octessera-ahub0-pcm5102 >> /boot/armbianEnv.txt.octessera.tmp
fi
mv -f /boot/armbianEnv.txt.octessera.tmp /boot/armbianEnv.txt
printf 'deployed_overlay_sha256=%s\\n' \"\$(sha256sum '$OVERLAY_PATH' | cut -d \" \" -f1)\" >> \"\$backup/manifest\"
sha256sum \"\$backup/manifest\" > \"\$backup/manifest.sha256\"
sync
rm -f '$REMOTE_STAGE'"
}

rollback_remote() {
	remote_exec "set -eu
backup=/boot/.octessera-ahub-experiment/$BACKUP_ID
test -f \"\$backup/manifest\" \"\$backup/manifest.sha256\"
sha256sum -c \"\$backup/manifest.sha256\" >/dev/null
test \"\$(grep \"^target=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")\" = $TARGET
test \"\$(grep \"^dtb_path=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")\" = $DTB_PATH
test \"\$(grep \"^overlay_path=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")\" = $OVERLAY_PATH
env_state=\$(grep \"^env_state=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")
env_hash=\$(grep \"^env_sha256=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")
if test \"\$env_state\" = present; then
  test \"\$(sha256sum \"\$backup/armbianEnv.txt\" | cut -d \" \" -f1)\" = \"\$env_hash\"
  install -m 0644 \"\$backup/armbianEnv.txt\" /boot/armbianEnv.txt
else
  test \"\$env_state\" = absent
  rm -f /boot/armbianEnv.txt
fi
dtb_state=\$(grep \"^dtb_state=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")
dtb_hash=\$(grep \"^dtb_sha256=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")
if test \"\$dtb_state\" = present; then
  test \"\$(sha256sum \"\$backup/dtb\" | cut -d \" \" -f1)\" = \"\$dtb_hash\"
  install -m 0644 \"\$backup/dtb\" '$DTB_PATH'
else
  test \"\$dtb_state\" = absent
  rm -f '$DTB_PATH'
fi
overlay_state=\$(grep \"^overlay_state=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")
overlay_hash=\$(grep \"^overlay_sha256=\" \"\$backup/manifest\" | cut -d= -f2- | sed -n \"1p\")
if test \"\$overlay_state\" = present; then
  test \"\$(sha256sum \"\$backup/overlay.dtbo\" | cut -d \" \" -f1)\" = \"\$overlay_hash\"
  install -m 0644 \"\$backup/overlay.dtbo\" '$OVERLAY_PATH'
else
  test \"\$overlay_state\" = absent
  rm -f '$OVERLAY_PATH'
fi
sync"
}

case "$ACTION" in
	preflight)
	prepare_artifact
	remote_identity_and_merge
	cleanup_stage
	printf '%s\n' 'remote AHUB preflight passed; no boot files were changed'
	;;
	deploy)
	prepare_artifact
	remote_identity_and_merge
	make_backup
	install_artifact
	cleanup_stage
	printf '%s\n' "deployment complete; rollback with --backup-id $BACKUP_ID; device restart was not performed"
	;;
	rollback)
	rollback_remote
	printf '%s\n' "rollback complete for backup $BACKUP_ID; device restart was not performed"
	;;
esac
