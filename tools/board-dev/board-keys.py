"""Virtual uinput keyboard that drives the instrument menu on a board (run as root).

usage: board-keys.py <usb|bt> <mark-log> <label> "<steps>" [seconds]
steps: space-separated tokens: a key name with an optional *N repeat, wait:<s>, gap:<s> (pause after
each tap, default 0.12), or mark:<name> (writes "<uptime> <label>_<name>" to the mark log). With
seconds, the steps repeat until that much time has passed.
"""
import fcntl
import os
import struct
import sys
import time

KEYS = {"esc": 1, "enter": 28, "space": 57, "shift": 42, "ctrl": 29, "up": 103, "down": 108,
        "left": 105, "right": 106, "1": 2, "2": 3, "3": 4, "q": 16, "w": 17, "e": 18, "a": 30,
        "s": 31, "d": 32}
EV_SYN, EV_KEY = 0, 1
UI_SET_EVBIT, UI_SET_KEYBIT, UI_DEV_CREATE, UI_DEV_DESTROY = 0x40045564, 0x40045565, 0x5501, 0x5502
BUS = {"usb": 0x03, "bt": 0x05}


def emit(fd, kind, code, value):
    os.write(fd, struct.pack("llHHi", 0, 0, kind, code, value))


def main():
    bus, log, label, steps = sys.argv[1:5]
    seconds = float(sys.argv[5]) if len(sys.argv) > 5 else 0.0
    fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
    fcntl.ioctl(fd, UI_SET_EVBIT, EV_KEY)
    for code in range(1, 256):
        fcntl.ioctl(fd, UI_SET_KEYBIT, code)
    name = b"Octessera Test Keyboard".ljust(80, b"\0")
    os.write(fd, name + struct.pack("HHHHi", BUS[bus], 0x1209, 0x0001, 1, 0) + b"\0" * (64 * 4 * 4))
    fcntl.ioctl(fd, UI_DEV_CREATE)
    time.sleep(3.0)
    gap = 0.12
    end = time.monotonic() + seconds
    while True:
        for token in steps.split():
            kind, _, value = token.partition(":")
            if kind == "wait":
                time.sleep(float(value))
            elif kind == "gap":
                gap = float(value)
            elif kind == "mark":
                with open(log, "a") as out:
                    out.write(f"{open('/proc/uptime').read().split()[0]} {label}_{value}\n")
            else:
                key, _, count = token.partition("*")
                for _ in range(int(count or 1)):
                    emit(fd, EV_KEY, KEYS[key], 1)
                    emit(fd, EV_SYN, 0, 0)
                    time.sleep(0.03)
                    emit(fd, EV_KEY, KEYS[key], 0)
                    emit(fd, EV_SYN, 0, 0)
                    time.sleep(gap)
        if time.monotonic() >= end:
            break
    time.sleep(0.5)
    fcntl.ioctl(fd, UI_DEV_DESTROY)
    os.close(fd)


main()
