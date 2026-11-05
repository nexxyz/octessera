"""Rewrites a cloned board store for one host-driven board-scenarios setup (run as root on the board).

usage: board-scenarios-store.py <store-dir> <setup>
setups:
  usb_midi        external sync on USB MIDI, Send MIDI all inst.; L1 steps a Drum on every bottom tile
                  at 1/8, L2 a MIDI Drum with two stamped tiles; no autoplay
  usb_midi_swing  usb_midi with Swing 4:5
  usb_audio       USB Audio on, the store's own patch and Master Vol
  usb_audio_clip  USB Audio on, Master Vol 100, a hot synth playing eight notes per step
  keymap          Auto Save on, L1 a sequencer with an empty grid, keyboard capture allowed
"""
import json
import sys
from pathlib import Path

MIDI_DRUM_TILES = [(0, 1, 38), (4, 1, 42)]
MIDI_DRUM_MS = 150


def none_layer(layer):
    layer["build"] = {"behaviorConfig": None, "behaviorConfigHistory": {}, "behaviorId": "none", "seeded": False}
    layer["name"] = "none"


def sequencer(layer, slot, tiles):
    none_layer(layer)
    layer["build"].update(behaviorId="sequencer", stepRate="1/8",
                          savedState={"tiles": [(i % 8, i // 8) in tiles for i in range(64)], "width": 8, "height": 8})
    layer["name"] = "sequencer"
    link = layer["link"]
    link.update(eventEnabled=False, scanAxis="columns", scanDirection="forward", scanMode="scanning",
                scanSections=1, scanUnit="1/8", triggerProbabilityMode="full",
                triggerProbabilityMap=["full"] * 64)
    actions = {"activate": "note_on", "deactivate": "note_off", "scanned": "note_on", "scanned_empty": "none", "stable": "none"}
    link["mapping"] = {event: {"action": action, "delaySteps": 0, "retriggerCount": 0, "slot": slot}
                       for event, action in actions.items()}


def patch_layers(music, layers):
    for index, layer in enumerate(music["layers"]):
        if index in layers:
            sequencer(layer, *layers[index])
        else:
            none_layer(layer)
    music.update(activeBehavior="sequencer", activeLayerIndex=0)


def main():
    store, setup = Path(sys.argv[1]), sys.argv[2]
    system = json.loads((store / "system.json").read_text())
    patch = json.loads((store / "default.patch.json").read_text())
    config, music = system["runtimeConfig"], patch["runtimeConfig"]
    config.update(autoSaveDefault=False, dimTimerSeconds=0, screenSleepSeconds=0)
    instruments = music["instruments"]
    if setup.startswith("usb_midi"):
        config["midi"].update(enabled=True, syncMode="external", clockInEnabled=True, clockOutEnabled=False,
                              respondToStartStop=True, sendInstruments="all")
        config["usb"]["midiOutEnabled"] = True
        patch_layers(music, {0: (0, {(x, 0) for x in range(8)}), 1: (7, {(x, y) for x, y, _ in MIDI_DRUM_TILES})})
        instruments[0].update(type="drum", name="Drum", autoName=True)
        instruments[0]["drum"]["assignments"] = [
            {"x": x, "y": 0, "voice": 0} for x in range(8)]
        instruments[7].update(type="midi_drum", name="MIDI Drum", autoName=True)
        instruments[7]["midiDrum"].update(enabled=True, channel=10, durationMs=MIDI_DRUM_MS, assignments=[
            {"x": x, "y": y, "note": note, "velocity": 100} for x, y, note in MIDI_DRUM_TILES])
        music["transport"].update(swingA=4, swingB=5 if setup == "usb_midi_swing" else 4)
    elif setup.startswith("usb_audio"):
        config["audioOutputs"]["usb"] = True
        if setup == "usb_audio_clip":
            config["masterVolume"] = 100
            patch_layers(music, {0: (0, {(x, y) for x in range(8) for y in range(8)})})
            instruments[0].update(type="synth", name="Synth", autoName=True, noteBehavior="oneshot")
            synth = instruments[0]["synth"]
            synth["amp"]["gainPct"] = 100
            synth["ampEnv"].update(attackMs=5, sustainPct=100)
            synth["filter"].update(cutoffHz=20000, resonance=0)
            for osc in ("osc1", "osc2"):
                synth[osc]["levelPct"] = 100
            instruments[0]["mixer"].update(route="direct", volume=100)
            music["mixer"]["master"]["slots"] = [{"params": {}, "type": "none"}] * len(music["mixer"]["master"]["slots"])
            music["sound"].update(noteLengthMs=240, velocityScalePct=200)
    elif setup == "keymap":
        config["autoSaveDefault"] = True
        if config["hdmi"]["mode"] == "terminal":
            config["hdmi"]["mode"] = "live-grid"
        config["bluetooth"]["enabled"] = True
        patch_layers(music, {0: (0, set())})
    else:
        sys.exit(f"unknown setup {setup}")
    (store / "system.json").write_text(json.dumps(system, separators=(",", ":")))
    (store / "default.patch.json").write_text(json.dumps(patch, separators=(",", ":")))


main()
