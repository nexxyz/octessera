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

printf '\n'
cat <<'EOF'
                          OOOO    OOOO
                         OOOOO   OOOOOO
                       OOO     OOO    OOO
                     OOOO    OOOO       OOOO
                   OOOO    OOOO   OOOO    OOOO
                   OOOO    OOOO   OOOO    OOOO
                      OOO       OOOO    OOO
                        OOOO   OOO    OOOO
                          OOOOOO   OOOOO
                           OOOO    OOOO

      OOOOO OOOOO OOOOOO OOOOO OOOOO OOOOO OOOOO OOOOO OOOOO
      O   O O       OO   O     O     O     O     O   O O   O
      O   O O       OO   OOOOO OOOOO OOOOO OOOOO OOOOO OOOOO
      O   O O       OO   O         O     O O     O  OO O   O
      OOOOO OOOOO   OO   OOOOO OOOOO OOOOO OOOOO O   O O   O
EOF
printf '\n  cellular automata -> music\n'
printf '  service: systemctl status octessera\n'
printf '  logs:    journalctl -u octessera -f\n\n'
