use crate::data::commands::PitchAction;

/// FT2's linear frequency table has 64 steps to a semitone.
pub const STEPS_PER_SEMITONE: i32 = 64;
/// FT2's portamento speeds are in 1/16 semitones.
const PORTA_STEP: i32 = 4;
const VIBRATO_POSITION_STEP: u8 = 4;
/// Half a sine, straight from FT2.
const VIBRATO_SINE: [u8; 32] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250, 244,
    235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24,
];
/// E44 to E47: don't restart the vibrato on a new note.
const KEEP_POSITION: u8 = 4;

/// FT2's effect memory: a 00 carries on with the last value.
#[derive(Clone, Copy, Default)]
struct Memory {
    porta_up: u8,
    porta_down: u8,
    tone_porta: u8,
    fine_up: u8,
    fine_down: u8,
    vibrato_speed: u8,
    vibrato_depth: u8,
}

/// How far FT2's pitch commands have bent a track's note, in `STEPS_PER_SEMITONE`.
#[derive(Clone, Copy, Default)]
pub struct TrackPitch {
    /// Channel and pitch.
    note: Option<(u8, u8)>,
    /// Stays until the next note.
    porta: i32,
    tone_target: i32,
    /// Vibrato and arpeggio, gone again on the next row without them.
    wobble: i32,
    /// This row's command, with the memory filled in.
    action: Option<PitchAction>,
    memory: Memory,
    vibrato_position: u8,
    waveform: u8,
    glissando: bool,
}

impl TrackPitch {
    pub fn channel(&self) -> Option<u8> {
        return self.note.map(|(channel, _)| channel);
    }

    pub fn offset(&self) -> i32 {
        let porta = match self.action {
            Some(PitchAction::TonePorta(_)) if self.glissando => {
                let half = STEPS_PER_SEMITONE / 2 * self.porta.signum();
                (self.porta + half) / STEPS_PER_SEMITONE * STEPS_PER_SEMITONE
            }
            _ => self.porta,
        };
        return porta + self.wobble;
    }

    pub fn note_started(&mut self, channel: u8, pitch: u8) {
        self.note = Some((channel, pitch));
        self.porta = 0;
        self.tone_target = 0;
        if self.waveform & KEEP_POSITION == 0 {
            self.vibrato_position = 0;
        }
    }

    pub fn start_row(&mut self) {
        self.action = None;
        self.wobble = 0;
    }

    /// Tick 0 of the row. True when the cell's note shouldn't play, because tone portamento
    /// bends to it instead.
    pub fn apply(&mut self, action: PitchAction, note: Option<u8>, limit: i32) -> bool {
        let memory = &mut self.memory;
        let remember = |slot: &mut u8, value: u8| {
            if value != 0 {
                *slot = value;
            }
            return *slot;
        };
        let action = match action {
            PitchAction::PortaUp(speed) => {
                PitchAction::PortaUp(remember(&mut memory.porta_up, speed))
            }
            PitchAction::PortaDown(speed) => {
                PitchAction::PortaDown(remember(&mut memory.porta_down, speed))
            }
            PitchAction::TonePorta(speed) => {
                PitchAction::TonePorta(remember(&mut memory.tone_porta, speed))
            }
            PitchAction::Vibrato { speed, depth } => PitchAction::Vibrato {
                speed: remember(&mut memory.vibrato_speed, speed),
                depth: remember(&mut memory.vibrato_depth, depth),
            },
            PitchAction::FinePortaUp(step) => {
                PitchAction::FinePortaUp(remember(&mut memory.fine_up, step))
            }
            PitchAction::FinePortaDown(step) => {
                PitchAction::FinePortaDown(remember(&mut memory.fine_down, step))
            }
            action => action,
        };
        self.action = Some(action);
        match action {
            PitchAction::FinePortaUp(step) => self.bend_by(step as i32 * PORTA_STEP, limit),
            PitchAction::FinePortaDown(step) => self.bend_by(-(step as i32) * PORTA_STEP, limit),
            PitchAction::Glissando(on) => self.glissando = on,
            PitchAction::VibratoWaveform(waveform) => self.waveform = waveform,
            PitchAction::TonePorta(_) => {
                // nothing playing to bend, so the note just plays
                let (Some((_, from)), Some(to)) = (self.note, note) else {
                    return false;
                };
                let target = (to as i32 - from as i32) * STEPS_PER_SEMITONE;
                self.tone_target = target.clamp(-limit, limit);
                return true;
            }
            _ => {}
        }
        return false;
    }

    pub fn tick(&mut self, tick: u32, limit: i32) {
        match self.action {
            Some(PitchAction::PortaUp(speed)) => self.bend_by(speed as i32 * PORTA_STEP, limit),
            Some(PitchAction::PortaDown(speed)) => {
                self.bend_by(-(speed as i32) * PORTA_STEP, limit)
            }
            Some(PitchAction::TonePorta(speed)) => {
                let step = speed as i32 * PORTA_STEP;
                let distance = self.tone_target - self.porta;
                self.porta += distance.clamp(-step, step);
            }
            Some(PitchAction::Vibrato { speed, depth }) => {
                self.wobble = self.vibrato(depth);
                self.vibrato_position = self
                    .vibrato_position
                    .wrapping_add(speed * VIBRATO_POSITION_STEP);
            }
            Some(PitchAction::Arpeggio { x, y }) => {
                let semitones = match tick % 3 {
                    0 => 0,
                    1 => x,
                    _ => y,
                };
                self.wobble = semitones as i32 * STEPS_PER_SEMITONE;
            }
            _ => {}
        }
    }

    fn bend_by(&mut self, steps: i32, limit: i32) {
        self.porta = (self.porta + steps).clamp(-limit, limit);
    }

    // same as FT2: pitch goes down for the first half of the cycle
    fn vibrato(&self, depth: u8) -> i32 {
        let index = (self.vibrato_position / 4) & 0x1F;
        let rising = self.vibrato_position >= 128;
        let amplitude = match self.waveform & 3 {
            0 => VIBRATO_SINE[index as usize],
            1 => match rising {
                true => !(index * 8),
                false => index * 8,
            },
            _ => u8::MAX,
        };
        let steps = amplitude as i32 * depth as i32 / 32;
        return if rising { steps } else { -steps };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: i32 = 12 * STEPS_PER_SEMITONE;

    fn playing(pitch: u8) -> TrackPitch {
        let mut track = TrackPitch::default();
        track.note_started(0, pitch);
        return track;
    }

    fn row(track: &mut TrackPitch, action: PitchAction, note: Option<u8>, ticks: u32) -> Vec<i32> {
        track.start_row();
        track.apply(action, note, LIMIT);
        let mut offsets = vec![track.offset()];
        for tick in 1..ticks {
            track.tick(tick, LIMIT);
            offsets.push(track.offset());
        }
        return offsets;
    }

    #[test]
    fn portamento_moves_on_every_tick_but_the_first_and_remembers_its_speed() {
        let mut track = playing(60);
        assert_eq!(
            row(&mut track, PitchAction::PortaUp(2), None, 3),
            [0, 8, 16]
        );
        assert_eq!(row(&mut track, PitchAction::PortaUp(0), None, 2), [16, 24]);
        assert_eq!(
            row(&mut track, PitchAction::PortaDown(1), None, 2),
            [24, 20]
        );
    }

    #[test]
    fn portamento_stops_at_the_bend_range() {
        let mut track = playing(60);
        let offsets = row(&mut track, PitchAction::PortaDown(0xFF), None, 10);
        assert_eq!(offsets.last(), Some(&-LIMIT));
    }

    #[test]
    fn tone_portamento_bends_to_the_note_instead_of_playing_it() {
        let mut track = playing(60);
        track.start_row();
        assert!(track.apply(PitchAction::TonePorta(0x10), Some(62), LIMIT));
        for tick in 1..6 {
            track.tick(tick, LIMIT);
        }
        assert_eq!(track.offset(), 2 * STEPS_PER_SEMITONE);
        let mut silent = TrackPitch::default();
        assert!(!silent.apply(PitchAction::TonePorta(0x10), Some(62), LIMIT));
    }

    #[test]
    fn glissando_moves_tone_portamento_in_semitones() {
        let mut track = playing(60);
        track.apply(PitchAction::Glissando(true), None, LIMIT);
        let offsets = row(&mut track, PitchAction::TonePorta(5), Some(62), 3);
        assert_eq!(offsets, [0, 0, 64]);
    }

    #[test]
    fn vibrato_goes_down_then_up_and_stops_with_its_rows() {
        let mut track = playing(60);
        let offsets = row(
            &mut track,
            PitchAction::Vibrato { speed: 8, depth: 8 },
            None,
            9,
        );
        assert_eq!(offsets[..3], [0, 0, -45]);
        assert!(
            offsets[5..].iter().all(|&offset| offset >= 0),
            "{offsets:?}"
        );
        track.start_row();
        assert_eq!(track.offset(), 0);
    }

    #[test]
    fn arpeggio_steps_through_its_notes() {
        let mut track = playing(60);
        let offsets = row(&mut track, PitchAction::Arpeggio { x: 4, y: 7 }, None, 6);
        assert_eq!(offsets, [0, 256, 448, 0, 256, 448]);
    }

    #[test]
    fn fine_portamento_bends_once_and_a_new_note_starts_unbent() {
        let mut track = playing(60);
        assert_eq!(
            row(&mut track, PitchAction::FinePortaUp(3), None, 3),
            [12, 12, 12]
        );
        assert_eq!(row(&mut track, PitchAction::FinePortaDown(1), None, 1), [8]);
        track.note_started(0, 64);
        assert_eq!(track.offset(), 0);
    }

    #[test]
    fn square_vibrato_jumps_between_its_depths() {
        let mut track = playing(60);
        track.apply(PitchAction::VibratoWaveform(2), None, LIMIT);
        let offsets = row(
            &mut track,
            PitchAction::Vibrato {
                speed: 0x10,
                depth: 4,
            },
            None,
            6,
        );
        assert_eq!(offsets, [0, -31, -31, 31, 31, -31]);
    }

    #[test]
    fn a_new_note_restarts_the_vibrato_unless_the_waveform_keeps_it() {
        let vibrato = PitchAction::Vibrato { speed: 8, depth: 8 };
        let mut track = playing(60);
        row(&mut track, vibrato, None, 3);
        track.note_started(0, 62);
        assert_eq!(row(&mut track, vibrato, None, 2), [0, 0]);
        track.apply(PitchAction::VibratoWaveform(4), None, LIMIT);
        row(&mut track, vibrato, None, 3);
        track.note_started(0, 64);
        assert_ne!(row(&mut track, vibrato, None, 2), [0, 0]);
    }
}
