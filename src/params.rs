use nih_plug::prelude::*;

#[derive(Params)]
pub struct UltrawaveParams {
    /// The output's level in dB.
    #[id = "gain"]
    pub gain: FloatParam,

    // RAM RECORD Parameters (R1-R4)
    #[id = "mlev"]
    pub mlev: IntParam,
    #[id = "mbal"]
    pub mbal: IntParam,
    #[id = "ilev"]
    pub ilev: IntParam,
    #[id = "ibal"]
    pub ibal: IntParam,
    #[id = "cue1"]
    pub cue1: IntParam,
    #[id = "cue2"]
    pub cue2: IntParam,
    #[id = "rec_len"]
    pub rec_len: IntParam,
    #[id = "rec_rate"]
    pub rec_rate: IntParam,

    // RAM PLAY Parameters (P1-P4)
    #[id = "strt"]
    pub strt: IntParam,
    #[id = "end"]
    pub end: IntParam,
    #[id = "pitch"]
    pub pitch: IntParam,
    #[id = "hold"]
    pub hold: IntParam,
    #[id = "dec"]
    pub dec: IntParam,
    #[id = "rtrg"]
    pub rtrg: IntParam,
    #[id = "rtim"]
    pub rtim: IntParam,

    // Filter Parameters
    #[id = "fltf"]
    pub fltf: IntParam,
    #[id = "fltq"]
    pub fltq: IntParam,
    #[id = "fltw"]
    pub fltw: IntParam,

    // Output
    #[id = "vol"]
    pub vol: IntParam,
    #[id = "pan"]
    pub pan: IntParam,
}

impl Default for UltrawaveParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                0.0,
                FloatRange::Linear {
                    min: -80.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            // RAM RECORD (0-127)
            mlev: IntParam::new("Main Level", 64, IntRange::Linear { min: 0, max: 127 }),
            mbal: IntParam::new("Main Balance", 64, IntRange::Linear { min: 0, max: 127 }),
            ilev: IntParam::new("Input Level", 64, IntRange::Linear { min: 0, max: 127 }),
            ibal: IntParam::new("Input Balance", 64, IntRange::Linear { min: 0, max: 127 }),
            cue1: IntParam::new("CUE1", 0, IntRange::Linear { min: 0, max: 127 }),
            cue2: IntParam::new("CUE2", 0, IntRange::Linear { min: 0, max: 127 }),
            rec_len: IntParam::new("Rec Length", 64, IntRange::Linear { min: 0, max: 127 }),
            rec_rate: IntParam::new("Rec Rate", 127, IntRange::Linear { min: 0, max: 127 }),

            // RAM PLAY (0-127)
            strt: IntParam::new("Start", 0, IntRange::Linear { min: 0, max: 127 }),
            end: IntParam::new("End", 127, IntRange::Linear { min: 0, max: 127 }),
            pitch: IntParam::new("Pitch", 64, IntRange::Linear { min: 0, max: 127 }),
            hold: IntParam::new("Hold", 0, IntRange::Linear { min: 0, max: 127 }),
            dec: IntParam::new("Decay", 0, IntRange::Linear { min: 0, max: 127 }),
            rtrg: IntParam::new("Retrigger", 0, IntRange::Linear { min: 0, max: 127 }),
            rtim: IntParam::new("Retrig Time", 0, IntRange::Linear { min: 0, max: 127 }),

            // Filter (0-127)
            fltf: IntParam::new("Filter Freq", 64, IntRange::Linear { min: 0, max: 127 }),
            fltq: IntParam::new("Filter Q", 0, IntRange::Linear { min: 0, max: 127 }),
            fltw: IntParam::new("Filter Width", 0, IntRange::Linear { min: 0, max: 127 }),

            // Output (0-127)
            vol: IntParam::new("Volume", 100, IntRange::Linear { min: 0, max: 127 }),
            pan: IntParam::new("Pan", 64, IntRange::Linear { min: 0, max: 127 }),
        }
    }
}
