use super::*;

const SIZE: usize = 1024;
const RATE: u32 = 24_000;

fn sine(hz: f32, amplitude: f32) -> Vec<f32> {
    (0..SIZE)
        .map(|index| amplitude * (std::f32::consts::TAU * hz * index as f32 / RATE as f32).sin())
        .collect()
}

fn loudest(levels: &[u8]) -> usize {
    (0..levels.len())
        .max_by_key(|index| levels[*index])
        .unwrap()
}

fn band_of(hz: f32, count: usize) -> usize {
    let span = (RATE as f32 / 2.0 / LOW_HZ).ln();
    ((hz / LOW_HZ).ln() / span * count as f32) as usize
}

#[test]
fn the_fft_puts_a_bin_centred_sine_into_its_bin_at_a_quarter_of_the_size() {
    let mut fft = Fft::new(SIZE);
    let hz = 64.0 * RATE as f32 / SIZE as f32;
    let magnitudes = fft.magnitudes(&sine(hz, 1.0)).to_vec();
    assert_eq!(magnitudes.len(), SIZE / 2);
    assert!((magnitudes[64] - SIZE as f32 / 4.0).abs() < 1.0);
    assert!(magnitudes[63] > 100.0 && magnitudes[65] > 100.0);
    assert!(magnitudes
        .iter()
        .enumerate()
        .all(|(bin, magnitude)| (63..=65).contains(&bin) || *magnitude < 0.5));
}

#[test]
fn log_bars_light_the_band_of_a_tone_and_sit_near_zero_elsewhere() {
    let mut fft = Fft::new(SIZE);
    for hz in [110.0, 1_000.0, 6_000.0] {
        let mut levels = [0_u8; SPECTRUM_BARS];
        log_levels(fft.magnitudes(&sine(hz, 1.0)), RATE, SIZE, &mut levels);
        let band = loudest(&levels);
        assert!(
            band.abs_diff(band_of(hz, SPECTRUM_BARS)) <= 2,
            "{hz} Hz in {band}"
        );
        assert!(levels[band] > 240, "{hz} Hz at {}", levels[band]);
        let far = levels
            .iter()
            .enumerate()
            .filter(|(index, _)| hz > 500.0 && index.abs_diff(band) > 12)
            .all(|(_, level)| *level < 60);
        assert!(far, "{hz} Hz: {levels:?}");
    }
    let mut quiet = [0_u8; SPECTRUM_BARS];
    log_levels(fft.magnitudes(&sine(1_000.0, 0.01)), RATE, SIZE, &mut quiet);
    let band = loudest(&quiet);
    assert!((55..=70).contains(&quiet[band]), "{}", quiet[band]);
}

#[test]
fn bars_jump_up_then_fall_gently_under_slower_peaks_until_flat() {
    let mut history = SpectrumHistory::new(SIZE);
    let tone = sine(1_000.0, 1.0);
    let OledSignalVisual::Spectrum { levels, peaks } = history.spectrum(&tone, RATE) else {
        panic!("a tone shows bars");
    };
    let band = loudest(&levels);
    assert_eq!(peaks[band], levels[band]);

    let silence = vec![0.0; SIZE];
    let OledSignalVisual::Spectrum {
        levels: fallen,
        peaks: held,
    } = history.spectrum(&silence, RATE)
    else {
        panic!("falling bars still show");
    };
    assert_eq!(fallen[band], levels[band] - BAR_FALL);
    assert_eq!(held[band], levels[band] - PEAK_FALL);

    let steps = (0..60)
        .take_while(|_| history.spectrum(&silence, RATE) != OledSignalVisual::Flat)
        .count();
    assert!((30..45).contains(&steps), "{steps}");
}

#[test]
fn the_waterfall_adds_the_newest_column_at_the_right_and_scrolls_left() {
    let mut history = SpectrumHistory::new(SIZE);
    let silence = vec![0.0; SIZE];
    assert_eq!(history.waterfall(&silence, RATE), OledSignalVisual::Flat);
    let OledSignalVisual::Waterfall { levels } = history.waterfall(&sine(1_000.0, 1.0), RATE)
    else {
        panic!("a tone lights the waterfall");
    };
    assert_eq!(levels.len(), SIGNAL_BAND_WIDTH * SIGNAL_BAND_HEIGHT);
    let newest = &levels[levels.len() - SIGNAL_BAND_HEIGHT..];
    assert!(loudest(newest).abs_diff(band_of(1_000.0, SIGNAL_BAND_HEIGHT)) <= 1);
    assert!(levels[..levels.len() - SIGNAL_BAND_HEIGHT]
        .iter()
        .all(|level| *level == 0));

    let OledSignalVisual::Waterfall { levels: scrolled } = history.waterfall(&silence, RATE) else {
        panic!("the tone column is still on screen");
    };
    let previous = scrolled.len() - 2 * SIGNAL_BAND_HEIGHT;
    assert_eq!(&scrolled[previous..previous + SIGNAL_BAND_HEIGHT], newest);
    assert!(scrolled[scrolled.len() - SIGNAL_BAND_HEIGHT..]
        .iter()
        .all(|level| *level == 0));
}
