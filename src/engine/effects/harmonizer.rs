use crate::engine::effects::{MidiEffect, Param, note_event};

const PARAMS: &[Param] = &[];
pub struct Harmonizer {}
impl Harmonizer {
    pub fn new() -> Self {
        return Self {};
    }
}

impl MidiEffect for Harmonizer {
    fn tick(&mut self, timing: super::Timing, out: &mut Vec<midi::Message>) {}
    fn process(
        &mut self,
        message: midi::Message,
        timing: Option<super::Timing>,
        out: &mut Vec<midi::Message>,
    ) {
        let Some(note) = note_event(&message) else {
            return;
        };
    }

    fn reset(&mut self, out: &mut Vec<midi::Message>) {}

    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }

    fn param(&self, index: usize) -> u8 {
        fn param(&self, index: usize) -> u8 {
            return match index {
                LOW_FREQUENCY => self.low_frequency,
                HIGH_FREQUENCY => self.high_frequency,
                LOW_VELOCITY => self.low_velocity,
                MID_VELOCITY => self.mid_velocity,
                HIGH_VELOCITY => self.high_velocity,
                HIGH_PASS_CUTOFF => self.high_pass_cutoff,
                LOW_PASS_CUTOFF => self.low_pass_cutoff,
                FILTER_VELOCITY_PER_OCTAVE => self.filter_velocity_per_octave,
                _ => 0,
            };
        }
    }

    fn set_param(&mut self, index: usize, value: u8) {
        todo!()
    }
}
