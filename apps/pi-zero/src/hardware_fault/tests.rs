use super::*;

fn fault(failures: &[(&'static str, &str)]) -> HardwareFault {
    let mut fault = HardwareFault::new();
    for (name, message) in failures {
        fault.push(name, (*message).to_string());
    }
    fault
}

fn screen(failures: &[(&'static str, &str)]) -> (&'static str, Vec<String>) {
    fault_screen(&fault(failures))
}

const TIMED_OUT: &str = "HW ID read failed: Connection timed out (os error 110)";
const MISSING: &str = "reset failed: Input/output error (os error 5)";

#[test]
fn timed_out_grid_and_keys_ask_for_a_power_cycle() {
    assert_eq!(
        screen(&[("TRELLIS", TIMED_OUT), ("NEOKEY", TIMED_OUT)]),
        (
            "HARDWARE FAULT",
            [
                "Grid + Keys",
                "not responding",
                "",
                "Unplug power and",
                "plug it back in.",
            ]
            .map(str::to_string)
            .to_vec()
        )
    );
}

#[test]
fn a_seesaw_startup_timeout_names_grid_and_keys() {
    let (_, lines) = screen(&[(
        "SEESAW",
        "Seesaw startup handshake timed out: timed out waiting on channel",
    )]);
    assert_eq!(lines[..2], ["Grid + Keys", "not responding"]);
}

#[test]
fn missing_parts_name_only_their_real_connections() {
    assert_eq!(
        screen(&[("NEOKEY", MISSING)]).1,
        [
            "Keys",
            "not found",
            "",
            "Unplug power,",
            "reseat the NeoKey,",
            "plug it back in."
        ]
    );
    assert_eq!(
        screen(&[("TRELLIS", MISSING)]).1,
        [
            "Grid",
            "not found",
            "",
            "Unplug power,",
            "check grid cable,",
            "plug it back in."
        ]
    );
    assert_eq!(
        screen(&[("encoder_main", "busy"), ("encoder_aux_1", "busy")]).1,
        [
            "Encoders",
            "not found",
            "",
            "Unplug power and",
            "plug it back in."
        ]
    );
}

#[test]
fn unknown_parts_keep_their_own_name() {
    assert_eq!(screen(&[("RTC", MISSING)]).1[0], "RTC");
}

#[test]
fn long_part_lists_collapse_and_still_fit() {
    let (_, lines) = screen(&[
        ("TRELLIS", MISSING),
        ("NEOKEY", MISSING),
        ("SEESAW_INT", MISSING),
        ("OLED", MISSING),
    ]);
    assert_eq!(
        lines,
        [
            "Several parts",
            "not found",
            "Unplug power,",
            "check grid cable,",
            "reseat the NeoKey,",
            "reseat the screen,",
            "plug it back in.",
        ]
    );
}

#[test]
fn audio_failures_show_the_audio_screen_without_cable_advice() {
    let expected = (
        "AUDIO NOT READY",
        [
            "Retrying...",
            "",
            "If this stays for a",
            "few minutes, power-",
            "cycle the device.",
        ]
        .map(str::to_string)
        .to_vec(),
    );
    assert_eq!(screen(&[("AUDIO", "no ALSA device")]), expected);
    assert_eq!(screen(&[("DAC", "no ALSA device")]), expected);
    let (header, lines) = screen(&[("DAC", MISSING), ("NEOKEY", MISSING)]);
    assert_eq!(header, "HARDWARE FAULT");
    assert_eq!(lines[0], "Keys");
}

#[test]
fn only_audio_faults_restart_to_retry() {
    assert!(retries_audio(&fault(&[("AUDIO", "no ALSA device")])));
    assert!(retries_audio(&fault(&[("DAC", "no ALSA device")])));
    assert!(!retries_audio(&fault(&[
        ("DAC", MISSING),
        ("NEOKEY", MISSING)
    ])));
    assert!(!retries_audio(&fault(&[("I2C", MISSING)])));
    assert!(!retries_audio(&fault(&[("TRELLIS", TIMED_OUT)])));
}

#[test]
fn encoder_input_wakes_the_fault_screen_and_lost_encoders_keep_it_on() {
    let (tx, rx) = std::sync::mpsc::channel();
    let soon = || Instant::now() + Duration::from_millis(20);
    assert_eq!(wait_for_wake(&rx, soon()), Wake::Quiet);
    tx.send(HardwareEvent::EncoderTurn {
        id: "encoder_main",
        delta: 1,
    })
    .unwrap();
    tx.send(HardwareEvent::EncoderPress {
        id: "encoder_aux_1",
    })
    .unwrap();
    assert_eq!(wait_for_wake(&rx, soon()), Wake::Input);
    assert_eq!(wait_for_wake(&rx, soon()), Wake::Quiet);
    drop(tx);
    let until = soon();
    assert_eq!(wait_for_wake(&rx, until), Wake::Lost);
    assert!(Instant::now() >= until);
}

#[test]
fn every_fault_screen_fits_the_oled() {
    for message in [TIMED_OUT, MISSING] {
        for names in [
            &["TRELLIS", "NEOKEY", "SEESAW_INT", "OLED", "I2C"][..],
            &["SEESAW"],
            &["AUDIO"],
            &["encoder_main"],
        ] {
            let failures: Vec<_> = names.iter().map(|name| (*name, message)).collect();
            let (header, lines) = screen(&failures);
            assert!(header.chars().count() <= 15);
            assert!(lines.len() <= MAX_FAULT_LINES);
            for line in lines {
                assert!(line.chars().count() <= FAULT_LINE_CHARS, "{line}");
            }
        }
    }
}
