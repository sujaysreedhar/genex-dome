//! Show timing and transitions use explicit deltas so pause and load boundaries are testable.
#[derive(Debug, Clone)]
enum Fade {
    Out {
        target: usize,
        half: f64,
        elapsed: f64,
    },
    Loading {
        half: f64,
    },
    In {
        half: f64,
        elapsed: f64,
    },
}
#[derive(Debug, Default)]
pub struct Sequencer {
    pub elapsed: f64,
    fade: Option<Fade>,
}
impl Sequencer {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn reset_cue(&mut self) {
        self.elapsed = 0.;
    }
    pub fn transitioning(&self) -> bool {
        self.fade.is_some()
    }
    pub fn loading(&self) -> bool {
        matches!(self.fade, Some(Fade::Loading { .. }))
    }
    pub fn holding_outgoing(&self) -> bool {
        matches!(self.fade, Some(Fade::Out { .. }))
    }
    pub fn gain(&self) -> f32 {
        match self.fade {
            Some(Fade::Out { half, elapsed, .. }) => (1. - elapsed / half).clamp(0., 1.) as f32,
            Some(Fade::Loading { .. }) => 0.,
            Some(Fade::In { half, elapsed }) => (elapsed / half).clamp(0., 1.) as f32,
            None => 1.,
        }
    }
    /// Returns an immediate switch for cuts; fades emit the switch once at black.
    pub fn request(&mut self, target: usize, seconds: f32) -> Option<usize> {
        if seconds <= 0. || !seconds.is_finite() {
            self.fade = None;
            return Some(target);
        }
        self.fade = Some(Fade::Out {
            target,
            half: seconds as f64 / 2.,
            elapsed: 0.,
        });
        None
    }
    pub fn loaded(&mut self) {
        self.elapsed = 0.;
        if let Some(Fade::Loading { half }) = self.fade {
            self.fade = Some(Fade::In { half, elapsed: 0. });
        }
    }
    pub fn tick(&mut self, delta: f64, playing: bool) -> Option<usize> {
        if !playing || !delta.is_finite() || delta <= 0. {
            return None;
        }
        match &mut self.fade {
            Some(Fade::Out {
                target,
                half,
                elapsed,
            }) => {
                *elapsed += delta;
                if *elapsed >= *half {
                    let target = *target;
                    self.fade = Some(Fade::Loading { half: *half });
                    return Some(target);
                }
            }
            Some(Fade::In { half, elapsed }) => {
                *elapsed += delta;
                self.elapsed += delta;
                if *elapsed >= *half {
                    self.fade = None;
                }
            }
            Some(Fade::Loading { .. }) => {}
            None => self.elapsed += delta,
        }
        None
    }
}
pub fn next_index(selected: usize, len: usize, looping: bool) -> Option<usize> {
    if selected >= len || len == 0 {
        None
    } else if selected + 1 < len {
        Some(selected + 1)
    } else if looping {
        Some(0)
    } else {
        None
    }
}
pub fn duration_reached(
    cue: &crate::state::CueSettings,
    is_video: bool,
    position: f64,
    elapsed: f64,
    slide_seconds: f32,
) -> bool {
    if is_video {
        cue.duration_seconds.is_some_and(|limit| position >= limit)
    } else {
        elapsed >= cue.duration_seconds.unwrap_or(slide_seconds as f64)
    }
}
