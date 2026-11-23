"""Rewrites a cloned board store for one host-driven board-scenarios setup (run as root on the board).

usage: board-scenarios-store.py <store-dir> <setup>
setups:
  usb_midi        external sync on USB MIDI, Send MIDI all inst.; L1 steps a Drum on every bottom tile
                  at 1/8, L2 a MIDI Drum with two stamped tiles; no autoplay
  usb_midi_swing  usb_midi with Swing 4:5
  usb_midi_cc     usb_midi with CC1 (any channel) mapped to Vel Scale
  usb_audio       USB Audio on, the store's own patch and Master Vol
  usb_audio_fx    usb_audio with a tape stop on master mapped to Play FX tile (3, 3)
  usb_audio_clip  USB Audio on, Master Vol 100, eight synth notes per step pushed past full scale
  keymap          Auto Save on, L1 a sequencer with an empty grid, keyboard capture allowed
  usb_looper      the usb_midi rig with Auto Save on; L1 an empty 8-step 1/8 Looper on a Synth
                  with Hold, tile (x, y) playing note 60 + x + 8y
  usb_notes_in    the usb_midi rig; slot 1 a Synth with Hold, L1 steps slot 5, a MIDI Inst. on
                  channel 1, on tile (0, 0), which plays note 60
  resets          Auto Save on, View on Back off, keyboard capture allowed, USB and audio outputs at
                  their factory values (so Factory System changes no device setting)
  old_system      keyboard capture allowed, System without midi.notesIn (as saved before it existed)
  unknown_key     System with an unknown top-level key
  unknown_runtime_key  System with an unknown key in runtimeConfig
  truncated_patch the patch cut to half its bytes
  full_card       Auto Save on, View on Back off, keyboard capture allowed
  usb_hold_trig   the usb_midi rig; only the store's Life layer (L1), Event Triggers on, On none, Hold = note
                  on and Off = note off for slot 1, a Synth with Hold, seeded with a diagonal of three
                  cells whose middle one stays lit for one step and then dies with the rest
  usb_echo_midi   the usb_midi rig; slot 1 a MIDI Inst. on channel 1, Notes In on; L1 steps C4 on slot 1
                  every 1/16 (all of row 0 plays note 60) with One-Shot Len 150 ms
  usb_echo_synth  usb_echo_midi with slot 1 a Synth instead, mirrored by Send MIDI all inst.
  tempo_mod       the store's own patch with Auto Save on, internal clock with Clock Out on USB MIDI,
                  Link LFO 1 on BPM (1/4, depth 75%) and CC1 (any channel) on BPM
  presets         Auto Save on, View on Back off, keyboard capture allowed; presets A and AB, the
                  store's patch with L1 One-Shot Len 301 and 302 ms (BPM would also move the
                  tempo-synced delay times the runtime derives from it)
"""
import json
import os
import sys
from pathlib import Path

MIDI_DRUM_TILES = [(0, 1, 38), (4, 1, 42)]
MIDI_DRUM_MS = 150
VEL_SCALE = {"invert": False, "key": "sound.velocityScalePct", "kind": "number", "label": "Velocity Scale",
             "max": 200, "min": 0, "step": 1}
# config/generated/pi/default.json's USB and audio output settings, the ones Factory System writes.
FACTORY_DEVICE = {"usb": {"dataRole": "gadget", "midiOutEnabled": False},
                  "audioOutputs": {"dac": True, "hdmi": False, "usb": False}}
BPM = {"invert": False, "key": "transport.bpm", "kind": "number", "label": "BPM", "max": 240, "min": 40, "step": 1}
TAPE_STOP = {"fxType": "tape_stop", "params": {"startMs": 300, "stopMs": 200}, "targetKey": "master"}


def none_layer(layer):
    layer["build"] = {"behaviorConfig": None, "behaviorConfigHistory": {}, "behaviorId": "none", "seeded": False}
    layer["name"] = "none"


def sequencer(layer, slot, tiles):
    none_layer(layer)
    layer["build"].update(behaviorId="sequencer", stepRate="1/8",
                          savedState={"tiles": [(i % 8, i // 8) in tiles for i in range(64)], "width": 8, "height": 8})
    layer["name"] = "sequencer"
    link = layer["link"]
    link.update(eventEnabled=True, stateNotesEnabled=True, scanAxis="columns", scanDirection="forward",
                scanMode="scanning", scanSections=1, scanUnit="1/8", triggerProbabilityMode="full",
                triggerProbabilityMap=["full"] * 64)
    # A scan plays the On, Hold and Off rows: dark to lit fires On, lit to lit Hold, so Hold = note on
    # sounds every lit step.
    actions = {"activate": "note_on", "deactivate": "note_off", "stable": "note_on"}
    link["mapping"] = {event: {"action": action, "delaySteps": 0, "retriggerCount": 0, "slot": slot}
                       for event, action in actions.items()}


def chromatic(layer):
    """Tile (x, y) plays note 60 + x + 8y."""
    link = layer["link"]
    link["pitch"].update(root="C", scale="chromatic", startingNote=60, lowestNote=0, highestNote=127)
    link["x"]["pitch"]["steps"] = 1
    link["y"]["pitch"]["steps"] = 8


def looper(layer, slot, length):
    sequencer(layer, slot, set())
    config = {"lengthSteps": length, "mode": "overdub"}
    layer["build"].update(behaviorId="looper", behaviorConfig=config, behaviorConfigHistory={"looper": config},
                          savedState=dict(config, steps=[[] for _ in range(length)]))
    layer["link"].update(eventEnabled=True, scanMode="none")
    layer["name"] = "looper"


def midi_rig(config):
    config["midi"].update(enabled=True, syncMode="external", clockInEnabled=True, clockOutEnabled=False,
                          respondToStartStop=True, sendInstruments="all")
    config["usb"]["midiOutEnabled"] = True


def allow_keyboard(config):
    """board-keys.py types on a uinput keyboard: off the terminal HDMI mode, and on the Raspberry over Bluetooth."""
    if config["hdmi"]["mode"] == "terminal":
        config["hdmi"]["mode"] = "live-grid"
    config["bluetooth"]["enabled"] = True


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
        midi_rig(config)
        patch_layers(music, {0: (0, {(x, 0) for x in range(8)}), 1: (7, {(x, y) for x, y, _ in MIDI_DRUM_TILES})})
        instruments[0].update(type="drum", name="Drum", autoName=True)
        instruments[0]["drum"]["assignments"] = [
            {"x": x, "y": 0, "voice": 0} for x in range(8)]
        instruments[7].update(type="midi_drum", name="MIDI Drum", autoName=True)
        instruments[7]["midiDrum"].update(enabled=True, channel=10, durationMs=MIDI_DRUM_MS, assignments=[
            {"x": x, "y": y, "note": note, "velocity": 100} for x, y, note in MIDI_DRUM_TILES])
        music["transport"].update(swingA=4, swingB=5 if setup == "usb_midi_swing" else 4)
        if setup == "usb_midi_cc":
            music["midiCcMappings"][0].update(cc=1, channel=0, target=VEL_SCALE)
    elif setup.startswith("usb_audio"):
        config["audioOutputs"]["usb"] = True
        if setup == "usb_audio_fx":
            allow_keyboard(config)
            music["playFx"]["assignments"] = [{"config": TAPE_STOP, "x": 3, "y": 3}]
        if setup == "usb_audio_clip":
            allow_keyboard(config)
            config["masterVolume"] = 100
            patch_layers(music, {0: (0, {(x, y) for x in range(8) for y in range(8)})})
            instruments[0].update(type="synth", name="Synth", autoName=True, noteBehavior="oneshot")
            synth = instruments[0]["synth"]
            synth["amp"]["gainPct"] = 100
            synth["ampEnv"].update(attackMs=5, sustainPct=100)
            synth["filter"].update(cutoffHz=20000, resonance=0)
            synth["osc1"].update(waveform="saw", octave=0, detuneCents=0, levelPct=100)
            synth["osc2"].update(waveform="square", octave=0, detuneCents=0, levelPct=100)
            synth["filter"].update(envAmountPct=0, keyTrackingPct=0)
            for lfo in music["linkLfos"]:
                lfo["enabled"] = False
            instruments[0]["mixer"].update(route="direct", volume=100)
            # The engine keeps eight full voices near -6 dBFS, so 12 dB of compressor makeup at ratio 1 drives it
            # to about 2x full scale; Master Vol 20 brings it back to about -8 dBFS.
            slots = music["mixer"]["master"]["slots"]
            slots[:] = [{"params": {"attackMs": 36, "makeupDb": 12, "mixPct": 100, "ratio": 1, "releaseMs": 100,
                                    "thresholdDb": 0}, "type": "compressor"}] + [{"params": {}, "type": "none"}] * (len(slots) - 1)
            music["sound"].update(velocityScalePct=200)
    elif setup in ("usb_looper", "usb_notes_in"):
        midi_rig(config)
        instruments[0].update(type="synth", name="Synth", autoName=True, noteBehavior="hold")
        if setup == "usb_looper":
            config["autoSaveDefault"] = True
            patch_layers(music, {})
            looper(music["layers"][0], 0, 8)
            music["activeBehavior"] = "looper"
        else:
            patch_layers(music, {0: (4, {(0, 0)})})
            instruments[4].update(type="midi", name="MIDI", autoName=True)
            instruments[4]["midi"].update(enabled=True, channel=1)
        chromatic(music["layers"][0])
    elif setup == "usb_hold_trig":
        midi_rig(config)
        life = json.loads(json.dumps(music["layers"][0]))
        if life["build"]["behaviorId"] != "life":
            sys.exit("usb_hold_trig needs the store's L1 to be Life")
        patch_layers(music, {})
        music["layers"][0] = life
        music.update(activeBehavior="life", activeLayerIndex=0)
        instruments[0].update(type="synth", name="Synth", autoName=True, noteBehavior="hold")
        life["link"].update(eventEnabled=True, stateNotesEnabled=True)
        life["link"]["mapping"]["activate"].update(action="none", slot=0)
        life["link"]["mapping"]["stable"].update(action="note_on", slot=0)
        life["link"]["mapping"]["deactivate"].update(action="note_off", slot=0)
        life["build"]["savedState"].update(cells=[i in (18, 27, 36) for i in range(64)], triggerTypes=["none"] * 64,
                                           gliderSpawnInterval=0, randomCellsPerTick=0)
    elif setup in ("usb_echo_midi", "usb_echo_synth"):
        midi_rig(config)
        config["midi"]["notesIn"] = True
        patch_layers(music, {0: (0, {(x, 0) for x in range(8)})})
        layer = music["layers"][0]
        layer["build"]["stepRate"] = "1/16"
        chromatic(layer)
        layer["link"].update(scanUnit="1/16", oneShotLengthMs=150)
        layer["link"]["x"]["pitch"]["steps"] = 0
        if setup == "usb_echo_midi":
            instruments[0].update(type="midi", name="MIDI", autoName=True)
            instruments[0]["midi"].update(enabled=True, channel=1)
        else:
            instruments[0].update(type="synth", name="Synth", autoName=True, noteBehavior="oneshot")
    elif setup == "tempo_mod":
        config.update(autoSaveDefault=True, perfViewOnBack=False)
        allow_keyboard(config)
        config["midi"].update(enabled=True, syncMode="internal", clockInEnabled=False, clockOutEnabled=True,
                              respondToStartStop=False)
        config["usb"]["midiOutEnabled"] = True
        music["linkLfos"][0].update(enabled=True, period="1/4", depthPct=75, target=BPM)
        music["midiCcMappings"][0].update(cc=1, channel=0, target=BPM)
    elif setup == "presets":
        # Back on the home page would toggle the performance view, so how many Back presses reach the
        # home page would depend on the menu depth each step ends at.
        config.update(autoSaveDefault=True, perfViewOnBack=False)
        allow_keyboard(config)
        owner = os.stat(store)
        (store / "patches").mkdir(exist_ok=True)
        os.chown(store / "patches", owner.st_uid, owner.st_gid)
        for name, note_length in (("A", 301), ("AB", 302)):
            preset = json.loads(json.dumps(patch))
            preset["runtimeConfig"]["layers"][0]["link"]["oneShotLengthMs"] = note_length
            path = store / "patches" / f"{name}.json"
            path.write_text(json.dumps(preset, separators=(",", ":")))
            os.chown(path, owner.st_uid, owner.st_gid)
    elif setup in ("resets", "old_system", "unknown_key", "unknown_runtime_key", "truncated_patch", "full_card"):
        allow_keyboard(config)
        config.update(autoSaveDefault=setup in ("resets", "full_card"), perfViewOnBack=False)
        if setup == "resets":
            config.update(json.loads(json.dumps(FACTORY_DEVICE)))
        if setup == "old_system":
            del config["midi"]["notesIn"]
        if setup == "unknown_key":
            system["fromTheFuture"] = True
        if setup == "unknown_runtime_key":
            config["fromTheFuture"] = True
    elif setup == "keymap":
        config["autoSaveDefault"] = True
        allow_keyboard(config)
        patch_layers(music, {0: (0, set())})
    else:
        sys.exit(f"unknown setup {setup}")
    (store / "system.json").write_text(json.dumps(system, separators=(",", ":")))
    text = json.dumps(patch, separators=(",", ":"))
    (store / "default.patch.json").write_text(text[: len(text) // 2] if setup == "truncated_patch" else text)


main()
