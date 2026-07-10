case $- in
    *i*) ;;
    *) return 0 ;;
esac

if [ -n "${OCTESSERA_WELCOME_SHOWN:-}" ]; then
    return 0
fi
export OCTESSERA_WELCOME_SHOWN=1

if [ ! -t 1 ]; then
    return 0
fi

cat <<'EOF'
                          ████    ████
                         █████   ██████
                       ███     ███    ███
                     ████    ████       ████
                   ████    ████   ████    ████
                   ████    ████   ████    ████
                      ███       ████    ███
                        ████   ███    ████
                          ██████   █████
                           ████    ████

      █████ █████ ██████ █████ █████ █████ █████ █████ █████
      █   █ █       ██   █     █     █     █     █   █ █   █
      █   █ █       ██   █████ █████ █████ █████ █████ █████
      █   █ █       ██   █         █     █ █     █  ██ █   █
      █████ █████   ██   █████ █████ █████ █████ █   █ █   █
EOF
printf '\n  cellular automata -> music\n'
printf '  service: systemctl status octessera\n'
printf '  logs:    journalctl -u octessera -f\n\n'
