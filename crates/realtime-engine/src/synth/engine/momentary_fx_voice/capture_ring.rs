/// A stereo ring of the most recent input frames, read back at a distance behind the newest.
#[derive(Clone, Debug)]
pub(super) struct CaptureRing {
    left: Vec<f32>,
    right: Vec<f32>,
    newest: usize,
}

impl CaptureRing {
    pub(super) fn new(frames: usize) -> Self {
        let frames = frames.max(4);
        Self {
            left: vec![0.0; frames],
            right: vec![0.0; frames],
            newest: frames - 1,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.left.len()
    }

    pub(super) fn push(&mut self, left: f32, right: f32) {
        self.newest += 1;
        if self.newest == self.left.len() {
            self.newest = 0;
        }
        self.left[self.newest] = left;
        self.right[self.newest] = right;
    }

    /// The ring index `back` frames before the newest one.
    pub(super) fn index_back(&self, back: usize) -> usize {
        let len = self.left.len();
        (self.newest + len - back % len) % len
    }

    pub(super) fn frame_at(&self, index: usize) -> (f32, f32) {
        (self.left[index], self.right[index])
    }

    /// The frame `back` frames before the newest, linearly interpolated between frames.
    pub(super) fn read_back(&self, back: f32) -> (f32, f32) {
        let whole = back as usize;
        let frac = back - whole as f32;
        let newer = self.index_back(whole);
        let older = self.index_back(whole + 1);
        (
            self.left[newer] + (self.left[older] - self.left[newer]) * frac,
            self.right[newer] + (self.right[older] - self.right[newer]) * frac,
        )
    }
}
