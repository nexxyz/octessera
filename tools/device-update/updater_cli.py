#!/usr/bin/env python3
import sys

from updater_protocol import UpdateError, Updater


def usage() -> int:
    print("Usage: octessera-update check [vX.Y.Z] | apply [vX.Y.Z] | rollback", file=sys.stderr)
    return 2


def main(argv: list[str]) -> int:
    if not argv or len(argv) > 2 or argv[0] not in {"check", "apply", "rollback", "bootstrap", "guard", "recover"}:
        return usage()
    operation = argv[0]
    if operation in {"rollback", "bootstrap", "guard"} and len(argv) != 1:
        return usage()
    if operation == "recover" and (len(argv) > 2 or (len(argv) == 2 and argv[1] != "--boot")):
        return usage()
    updater = Updater()
    try:
        if operation == "check":
            updater.locked("check", argv[1] if len(argv) == 2 else "")
        elif operation == "apply":
            updater.locked("apply", argv[1] if len(argv) == 2 else "")
        else:
            updater.locked(operation, *argv[1:])
    except UpdateError as exc:
        print(str(exc), file=sys.stderr)
        return 1
    except Exception as exc:
        print(f"Updater failure: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
