use crate::evt::EvtDecoder;
use ebkit_macros::EvtDecode;

/// Stateful decoder for the EVT 2.0 event stream format.
///
/// EVT 2.0 encodes each event as a single 32-bit word. Timestamps are 34 bits
/// wide, split between a 28-bit `EVT_TIME_HIGH` word and a 6-bit field embedded
/// in each CD / trigger word.
///
/// No events are emitted until the first `EVT_TIME_HIGH` word is encountered.
#[derive(Debug)]
pub struct Evt20Decoder {
    /// Upper 28 bits of the current timestamp, set by `EVT_TIME_HIGH`.
    /// `None` until the first `EVT_TIME_HIGH` is received.
    time_high: Option<u32>,
}

impl Evt20Decoder {
    pub fn new() -> Self {
        Self { time_high: None }
    }

    fn full_timestamp(&self, time_low: u8) -> i64 {
        let th = self.time_high.unwrap_or(0);
        (i64::from(th) << 6) | i64::from(time_low)
    }
}

impl EvtDecoder for Evt20Decoder {
    fn decode(&mut self, buf: &[u8]) -> (Vec<ebkit_core::EventCD>, Vec<ebkit_core::ExtTrigger>) {
        let mut cd_events = Vec::new();
        let mut triggers = Vec::new();

        let mut pos = 0;
        self.time_high = None;

        while pos + 4 <= buf.len() {
            let raw = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);

            let word = match Word::from_u32(raw) {
                Some(w) => w,
                None => {
                    // Unknown event type — skip this word per spec.
                    pos += 4;
                    continue;
                }
            };

            match word {
                Word::TimeHigh { timestamp } => {
                    self.time_high = Some(timestamp);
                    pos += 4;
                }

                Word::Event2D {
                    time_low,
                    polarity,
                    x,
                    y,
                } => {
                    // Spec: no events before the first EVT_TIME_HIGH.
                    if self.time_high.is_none() {
                        pos += 4;
                        continue;
                    }

                    let t = self.full_timestamp(time_low);

                    cd_events.push(ebkit_core::EventCD {
                        x,
                        y,
                        p: i16::from(polarity),
                        t,
                    });
                    pos += 4;
                }

                Word::ExtTrigger {
                    time_low,
                    id,
                    value,
                } => {
                    if self.time_high.is_none() {
                        pos += 4;
                        continue;
                    }

                    let t = self.full_timestamp(time_low);

                    triggers.push(ebkit_core::ExtTrigger {
                        p: i16::from(value),
                        c: i16::from(id),
                        t,
                    });
                    pos += 4;
                }

                Word::EventImu { .. } | Word::Others { .. } | Word::Continued { .. } => {
                    pos += 4;
                }
            }
        }

        (cd_events, triggers)
    }
}

#[derive(Debug, PartialEq, Eq, EvtDecode)]
#[evt(word = "u32", tag_lsb = 28, tag_width = 4)]
enum Word {
    #[evt(tag = 0x0)]
    #[evt(tag = 0x1)]
    Event2D {
        #[field(lsb = 22, width = 6)]
        time_low: u8,
        #[field(lsb = 28, width = 4)]
        polarity: u8,
        #[field(lsb = 11, width = 11)]
        x: u16,
        #[field(lsb = 0, width = 11)]
        y: u16,
    },
    #[evt(tag = 0x8)]
    TimeHigh {
        #[field(lsb = 0, width = 28)]
        timestamp: u32,
    },
    #[evt(tag = 0xA)]
    ExtTrigger {
        #[field(lsb = 22, width = 6)]
        time_low: u8,
        #[field(lsb = 8, width = 5)]
        id: u8,
        #[field(lsb = 0, width = 1)]
        value: u8,
    },
    #[evt(tag = 0xD)]
    EventImu {
        #[field(lsb = 0, width = 28)]
        data: u32,
    },
    #[evt(tag = 0xE)]
    Others {
        #[field(lsb = 22, width = 6)]
        time_low: u8,
        #[field(lsb = 16, width = 1)]
        class: u8,
        #[field(lsb = 0, width = 16)]
        subtype: u16,
    },
    #[evt(tag = 0xF)]
    Continued {
        #[field(lsb = 0, width = 28)]
        data: u32,
    },
}

impl Word {
    fn from_u32(data: u32) -> Option<Self> {
        Self::decode(data)
    }
}

#[cfg(test)]
mod tests {
    const TYPE_CD_OFF: u8 = 0x0;
    const TYPE_CD_ON: u8 = 0x1;
    const TYPE_EVT_TIME_HIGH: u8 = 0x8;
    const TYPE_EXT_TRIGGER: u8 = 0xA;
    const TYPE_IMU_EVT: u8 = 0xD;
    const TYPE_OTHERS: u8 = 0xE;
    const TYPE_CONTINUED: u8 = 0xF;
    use ebkit_core::{EventCD, ExtTrigger};

    use super::*;
    use crate::evt::EvtDecoder;
    fn make_word(ev_type: u8, payload: u32) -> u32 {
        ((ev_type as u32) << 28) | (payload & 0x0FFF_FFFF)
    }

    fn parse(ev_type: u8, payload: u32) -> Word {
        Word::from_u32(make_word(ev_type, payload)).expect("expected valid word")
    }

    fn words_to_bytes(words: &[u32]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(words.len() * 4);
        for w in words {
            buf.extend_from_slice(&w.to_le_bytes());
        }
        buf
    }

    fn time_high_word(ts: u32) -> u32 {
        make_word(TYPE_EVT_TIME_HIGH, ts)
    }

    fn cd_word(polarity: u8, time_low: u8, x: u16, y: u16) -> u32 {
        let ty = if polarity == 0 {
            TYPE_CD_OFF
        } else {
            TYPE_CD_ON
        };
        make_word(
            ty,
            (u32::from(time_low) << 22) | (u32::from(x) << 11) | u32::from(y),
        )
    }

    fn trigger_word(time_low: u8, id: u8, value: u8) -> u32 {
        make_word(
            TYPE_EXT_TRIGGER,
            (u32::from(time_low) << 22) | (u32::from(id) << 8) | u32::from(value),
        )
    }

    #[test]
    fn word_cd_event() {
        let w = parse(TYPE_CD_OFF, (100 << 11) | 200);
        assert_eq!(
            w,
            Word::Event2D {
                time_low: 0,
                polarity: 0,
                x: 100,
                y: 200
            }
        );

        let w = parse(TYPE_CD_ON, (63 << 22) | (2047 << 11) | 2047);
        assert_eq!(
            w,
            Word::Event2D {
                time_low: 63,
                polarity: 1,
                x: 2047,
                y: 2047
            }
        );

        assert_eq!(
            parse(TYPE_CD_OFF, 0b111111 << 22),
            Word::Event2D {
                time_low: 63,
                polarity: 0,
                x: 0,
                y: 0
            },
        );
        assert_eq!(
            parse(TYPE_CD_OFF, 0b11111111111 << 11),
            Word::Event2D {
                time_low: 0,
                polarity: 0,
                x: 2047,
                y: 0
            },
        );
        assert_eq!(
            parse(TYPE_CD_OFF, 0b11111111111),
            Word::Event2D {
                time_low: 0,
                polarity: 0,
                x: 0,
                y: 2047
            },
        );
    }

    #[test]
    fn word_time_high() {
        assert_eq!(
            parse(TYPE_EVT_TIME_HIGH, 0x0ABC_1234),
            Word::TimeHigh {
                timestamp: 0x0ABC_1234
            }
        );
        assert_eq!(
            parse(TYPE_EVT_TIME_HIGH, 0x0FFF_FFFF),
            Word::TimeHigh {
                timestamp: 0x0FFF_FFFF
            }
        );
    }

    #[test]
    fn word_ext_trigger() {
        let w = parse(TYPE_EXT_TRIGGER, (10 << 22) | (1 << 8) | 1);
        assert_eq!(
            w,
            Word::ExtTrigger {
                time_low: 10,
                id: 1,
                value: 1
            }
        );

        let w = parse(TYPE_EXT_TRIGGER, 5 << 22);
        assert_eq!(
            w,
            Word::ExtTrigger {
                time_low: 5,
                id: 0,
                value: 0
            }
        );

        let payload = (20 << 22) | (0x1FF << 13) | (0x1F << 8) | (0x7F << 1) | 1;
        assert_eq!(
            parse(TYPE_EXT_TRIGGER, payload),
            Word::ExtTrigger {
                time_low: 20,
                id: 0x1F,
                value: 1
            }
        );
    }

    #[test]
    fn word_imu_evt() {
        assert_eq!(
            parse(TYPE_IMU_EVT, 0x0012_3456),
            Word::EventImu { data: 0x0012_3456 }
        );
    }

    #[test]
    fn word_others() {
        assert_eq!(
            parse(TYPE_OTHERS, (33 << 22) | (1 << 16) | 0xBEEF),
            Word::Others {
                time_low: 33,
                class: 1,
                subtype: 0xBEEF
            },
        );
        assert_eq!(
            parse(TYPE_OTHERS, 0x0001),
            Word::Others {
                time_low: 0,
                class: 0,
                subtype: 1
            }
        );
    }

    #[test]
    fn word_continued() {
        assert_eq!(
            parse(TYPE_CONTINUED, 0x0FED_CBA9),
            Word::Continued { data: 0x0FED_CBA9 }
        );
        assert_eq!(parse(TYPE_CONTINUED, 0), Word::Continued { data: 0 });
    }

    #[test]
    fn word_unknown_type_returns_error() {
        for ty in [0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x9, 0xB, 0xC] {
            assert!(Word::from_u32(make_word(ty, 0)).is_none());
        }
    }

    #[test]
    fn no_events_before_time_high() {
        let buf = words_to_bytes(&[cd_word(0, 10, 100, 200), cd_word(1, 20, 300, 400)]);
        let mut dec = Evt20Decoder::new();
        let (cd, trig) = dec.decode(&buf);
        assert!(cd.is_empty());
        assert!(trig.is_empty());
    }

    #[test]
    fn basic_cd_events() {
        let th: u32 = 1000;
        let buf = words_to_bytes(&[
            time_high_word(th),
            cd_word(0, 5, 100, 200),
            cd_word(1, 10, 300, 400),
        ]);
        let mut dec = Evt20Decoder::new();
        let (cd, trig) = dec.decode(&buf);

        assert_eq!(cd.len(), 2);
        assert!(trig.is_empty());

        assert_eq!(
            cd[0],
            EventCD {
                x: 100,
                y: 200,
                p: 0,
                t: (i64::from(th) << 6) | 5,
            }
        );
        assert_eq!(
            cd[1],
            EventCD {
                x: 300,
                y: 400,
                p: 1,
                t: (i64::from(th) << 6) | 10,
            }
        );
    }

    #[test]
    fn trigger_events() {
        let th: u32 = 500;
        let buf = words_to_bytes(&[
            time_high_word(th),
            trigger_word(7, 0, 1),
            trigger_word(15, 1, 0),
        ]);
        let mut dec = Evt20Decoder::new();
        let (cd, trig) = dec.decode(&buf);

        assert!(cd.is_empty());
        assert_eq!(trig.len(), 2);

        assert_eq!(
            trig[0],
            ExtTrigger {
                p: 1,
                c: 0,
                t: (i64::from(th) << 6) | 7,
            }
        );
        assert_eq!(
            trig[1],
            ExtTrigger {
                p: 0,
                c: 1,
                t: (i64::from(th) << 6) | 15,
            }
        );
    }

    #[test]
    fn decodes_all_events() {
        let th: u32 = 100;
        let buf = words_to_bytes(&[
            time_high_word(th),
            cd_word(0, 1, 0, 0),
            cd_word(0, 2, 1, 0),
            cd_word(0, 3, 2, 0),
            cd_word(0, 4, 3, 0),
            cd_word(0, 5, 4, 0),
        ]);
        let mut dec = Evt20Decoder::new();
        let (cd, _) = dec.decode(&buf);
        assert_eq!(cd.len(), 5);
        assert_eq!(cd[0].x, 0);
        assert_eq!(cd[4].x, 4);
    }

    #[test]
    fn time_high_updates_across_batches() {
        let buf = words_to_bytes(&[
            time_high_word(10),
            cd_word(1, 5, 0, 0),
            time_high_word(20),
            cd_word(0, 3, 1, 1),
        ]);
        let mut dec = Evt20Decoder::new();

        let (cd, _) = dec.decode(&buf);
        assert_eq!(cd.len(), 2);
        assert_eq!(cd[0].t, (10_i64 << 6) | 5);
        assert_eq!(cd[1].t, (20_i64 << 6) | 3);
    }

    #[test]
    fn interleaved_cd_and_triggers() {
        let th: u32 = 42;
        let buf = words_to_bytes(&[
            time_high_word(th),
            cd_word(0, 1, 10, 20),
            trigger_word(2, 0, 1),
            cd_word(1, 3, 30, 40),
            trigger_word(4, 1, 0),
        ]);
        let mut dec = Evt20Decoder::new();
        let (cd, trig) = dec.decode(&buf);

        assert_eq!(cd.len(), 2);
        assert_eq!(trig.len(), 2);

        assert_eq!(trig[0].c, 0);
        assert_eq!(trig[0].p, 1);
        assert_eq!(trig[1].c, 1);
        assert_eq!(trig[1].p, 0);
    }

    #[test]
    fn unknown_event_types_skipped() {
        let buf = words_to_bytes(&[time_high_word(1), make_word(0x3, 0), cd_word(0, 5, 10, 10)]);
        let mut dec = Evt20Decoder::new();
        let (cd, _) = dec.decode(&buf);

        assert_eq!(cd.len(), 1);
        assert_eq!(cd[0].x, 10);
    }

    #[test]
    fn imu_and_continued_skipped() {
        let buf = words_to_bytes(&[
            time_high_word(1),
            make_word(TYPE_IMU_EVT, 0x1234),
            make_word(TYPE_CONTINUED, 0x5678),
            make_word(TYPE_CONTINUED, 0x9ABC),
            cd_word(1, 0, 5, 5),
        ]);
        let mut dec = Evt20Decoder::new();
        let (cd, _) = dec.decode(&buf);

        assert_eq!(cd.len(), 1);
        assert_eq!(cd[0].x, 5);
        assert_eq!(cd[0].p, 1);
    }

    #[test]
    fn empty_buffer() {
        let mut dec = Evt20Decoder::new();
        let (cd, trig) = dec.decode(&[]);
        assert!(cd.is_empty());
        assert!(trig.is_empty());
    }

    #[test]
    fn trailing_bytes_ignored() {
        let mut buf = time_high_word(1).to_le_bytes().to_vec();
        buf.extend_from_slice(&[0xFF, 0xFF, 0xFF]);
        let mut dec = Evt20Decoder::new();

        let (cd, _) = dec.decode(&buf);
        assert!(cd.is_empty());
    }
}
