use crate::engine::effects::MidiEffect;

pub struct ArpeggiatorEffect {}

impl ArpeggiatorEffect {
    pub fn new() -> Self {
        return Self {};
    }
}
impl MidiEffect for ArpeggiatorEffect {
    fn params(&self) -> &'static [super::Param] {
        todo!()
    }

    fn param(&self, index: usize) -> u8 {
        todo!()
    }

    fn set_param(&mut self, index: usize, value: u8) {
        todo!()
    }

    fn process(
        &mut self,
        message: midi::Message,
        timing: Option<super::Timing>,
        out: &mut Vec<midi::Message>,
    ) {
        todo!()
    }

    fn tick(&mut self, timing: super::Timing, out: &mut Vec<midi::Message>) {}

    fn reset(&mut self, out: &mut Vec<midi::Message>) {}
}
