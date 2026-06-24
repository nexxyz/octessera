pub(crate) fn interleave_stereo(left: &[f32], right: &[f32], out: &mut [f32]) {
    let frames = left.len().min(right.len()).min(out.len() / 2);
    interleave_stereo_impl(&left[..frames], &right[..frames], &mut out[..frames * 2]);
}

#[cfg(target_arch = "aarch64")]
fn interleave_stereo_impl(left: &[f32], right: &[f32], out: &mut [f32]) {
    let chunks = left.len() / 4;
    if chunks > 0 {
        unsafe {
            interleave_stereo_neon(left.as_ptr(), right.as_ptr(), out.as_mut_ptr(), chunks);
        }
    }
    let start = chunks * 4;
    interleave_stereo_scalar(&left[start..], &right[start..], &mut out[start * 2..]);
}

#[cfg(not(target_arch = "aarch64"))]
fn interleave_stereo_impl(left: &[f32], right: &[f32], out: &mut [f32]) {
    interleave_stereo_scalar(left, right, out);
}

fn interleave_stereo_scalar(left: &[f32], right: &[f32], out: &mut [f32]) {
    for (frame, (left, right)) in left.iter().zip(right).enumerate() {
        let out_idx = frame * 2;
        out[out_idx] = *left;
        out[out_idx + 1] = *right;
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn interleave_stereo_neon(
    left: *const f32,
    right: *const f32,
    out: *mut f32,
    chunks: usize,
) {
    use core::arch::aarch64::{float32x4x2_t, vld1q_f32, vst2q_f32};

    for chunk in 0..chunks {
        let in_offset = chunk * 4;
        let out_offset = chunk * 8;
        let pair = float32x4x2_t(
            vld1q_f32(left.add(in_offset)),
            vld1q_f32(right.add(in_offset)),
        );
        vst2q_f32(out.add(out_offset), pair);
    }
}

#[cfg(test)]
mod tests {
    use super::{interleave_stereo, interleave_stereo_scalar};

    #[test]
    fn interleaves_matching_slices() {
        let left = [1.0, 2.0, 3.0, 4.0];
        let right = [10.0, 20.0, 30.0, 40.0];
        let mut out = [0.0; 8];

        interleave_stereo(&left, &right, &mut out);

        assert_eq!(out, [1.0, 10.0, 2.0, 20.0, 3.0, 30.0, 4.0, 40.0]);
    }

    #[test]
    fn leaves_extra_output_untouched() {
        let left = [1.0, 2.0];
        let right = [10.0, 20.0];
        let mut out = [99.0; 6];

        interleave_stereo(&left, &right, &mut out);

        assert_eq!(out, [1.0, 10.0, 2.0, 20.0, 99.0, 99.0]);
    }

    #[test]
    fn uses_shortest_input_and_output_frame_count() {
        let left = [1.0, 2.0, 3.0, 4.0, 5.0];
        let right = [10.0, 20.0, 30.0];
        let mut out = [99.0; 5];

        interleave_stereo(&left, &right, &mut out);

        assert_eq!(out, [1.0, 10.0, 2.0, 20.0, 99.0]);
    }

    #[test]
    fn handles_tail_after_vector_width() {
        let left = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let right = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
        let mut out = [0.0; 12];

        interleave_stereo(&left, &right, &mut out);

        assert_eq!(
            out,
            [1.0, 10.0, 2.0, 20.0, 3.0, 30.0, 4.0, 40.0, 5.0, 50.0, 6.0, 60.0]
        );
    }

    #[test]
    fn wrapper_matches_scalar_reference_across_lengths() {
        for frames in 0..33 {
            let left: Vec<f32> = (0..frames).map(|idx| ((idx as f32) * 0.37).sin()).collect();
            let right: Vec<f32> = (0..frames)
                .map(|idx| ((idx as f32) * 0.19).cos() * 0.5)
                .collect();
            let mut actual = vec![99.0; frames * 2 + 3];
            let mut expected = vec![99.0; frames * 2 + 3];

            interleave_stereo(&left, &right, &mut actual);
            interleave_stereo_scalar(&left, &right, &mut expected[..frames * 2]);

            assert_eq!(actual, expected, "frames {frames}");
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn neon_matches_scalar_reference_directly() {
        for frames in 4..33 {
            let left: Vec<f32> = (0..frames)
                .map(|idx| ((idx as f32) * 0.13).sin() * 0.75)
                .collect();
            let right: Vec<f32> = (0..frames)
                .map(|idx| ((idx as f32) * 0.29).cos() * 0.25)
                .collect();
            let chunks = frames / 4;
            let neon_frames = chunks * 4;
            let mut actual = vec![99.0; neon_frames * 2];
            let mut expected = vec![99.0; neon_frames * 2];

            unsafe {
                super::interleave_stereo_neon(
                    left.as_ptr(),
                    right.as_ptr(),
                    actual.as_mut_ptr(),
                    chunks,
                );
            }
            interleave_stereo_scalar(&left[..neon_frames], &right[..neon_frames], &mut expected);

            assert_eq!(actual, expected, "frames {frames}");
        }
    }
}
