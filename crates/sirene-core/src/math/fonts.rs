//! The KaTeX fonts embedded for drawing formulas (LRES-T4, LRES-T5).
//!
//! The twenty TTFs in `fonts/katex/` are KaTeX 0.19.1's fonts, SIL OFL 1.1 (see
//! THIRD-PARTY-NOTICES). Each is parsed once, on first use.

use std::sync::OnceLock;

use super::metrics_data::Face;

/// A parsed KaTeX font.
pub(crate) type Font = ttf_parser::Face<'static>;

/// The two KaTeX faces that have no entry in `metrics_data` (KaTeX keeps no metrics for them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExtraFace {
    CaligraphicBold,
    FrakturBold,
}

impl ExtraFace {
    pub(crate) const ALL: [ExtraFace; 2] = [ExtraFace::CaligraphicBold, ExtraFace::FrakturBold];

    fn bytes(self) -> &'static [u8] {
        match self {
            ExtraFace::CaligraphicBold => {
                include_bytes!("../../fonts/katex/KaTeX_Caligraphic-Bold.ttf")
            }
            ExtraFace::FrakturBold => include_bytes!("../../fonts/katex/KaTeX_Fraktur-Bold.ttf"),
        }
    }

    fn slot(self) -> usize {
        match self {
            ExtraFace::CaligraphicBold => 0,
            ExtraFace::FrakturBold => 1,
        }
    }
}

/// The embedded TTF bytes of a face.
pub(crate) fn bytes(face: Face) -> &'static [u8] {
    match face {
        Face::AmsRegular => include_bytes!("../../fonts/katex/KaTeX_AMS-Regular.ttf"),
        Face::CaligraphicRegular => {
            include_bytes!("../../fonts/katex/KaTeX_Caligraphic-Regular.ttf")
        }
        Face::FrakturRegular => include_bytes!("../../fonts/katex/KaTeX_Fraktur-Regular.ttf"),
        Face::MainBold => include_bytes!("../../fonts/katex/KaTeX_Main-Bold.ttf"),
        Face::MainBoldItalic => include_bytes!("../../fonts/katex/KaTeX_Main-BoldItalic.ttf"),
        Face::MainItalic => include_bytes!("../../fonts/katex/KaTeX_Main-Italic.ttf"),
        Face::MainRegular => include_bytes!("../../fonts/katex/KaTeX_Main-Regular.ttf"),
        Face::MathBoldItalic => include_bytes!("../../fonts/katex/KaTeX_Math-BoldItalic.ttf"),
        Face::MathItalic => include_bytes!("../../fonts/katex/KaTeX_Math-Italic.ttf"),
        Face::SansSerifBold => include_bytes!("../../fonts/katex/KaTeX_SansSerif-Bold.ttf"),
        Face::SansSerifItalic => include_bytes!("../../fonts/katex/KaTeX_SansSerif-Italic.ttf"),
        Face::SansSerifRegular => include_bytes!("../../fonts/katex/KaTeX_SansSerif-Regular.ttf"),
        Face::ScriptRegular => include_bytes!("../../fonts/katex/KaTeX_Script-Regular.ttf"),
        Face::Size1Regular => include_bytes!("../../fonts/katex/KaTeX_Size1-Regular.ttf"),
        Face::Size2Regular => include_bytes!("../../fonts/katex/KaTeX_Size2-Regular.ttf"),
        Face::Size3Regular => include_bytes!("../../fonts/katex/KaTeX_Size3-Regular.ttf"),
        Face::Size4Regular => include_bytes!("../../fonts/katex/KaTeX_Size4-Regular.ttf"),
        Face::TypewriterRegular => {
            include_bytes!("../../fonts/katex/KaTeX_Typewriter-Regular.ttf")
        }
    }
}

fn index(face: Face) -> usize {
    Face::ALL.iter().position(|f| *f == face).unwrap_or(0)
}

/// The parsed font of a face, or `None` if its bytes do not parse.
pub(crate) fn font(face: Face) -> Option<&'static Font> {
    static FONTS: [OnceLock<Option<Font>>; 18] = [const { OnceLock::new() }; 18];
    FONTS
        .get(index(face))?
        .get_or_init(|| ttf_parser::Face::parse(bytes(face), 0).ok())
        .as_ref()
}

/// The parsed font of one of the faces without metrics.
pub(crate) fn extra_font(face: ExtraFace) -> Option<&'static Font> {
    static FONTS: [OnceLock<Option<Font>>; 2] = [const { OnceLock::new() }; 2];
    FONTS
        .get(face.slot())?
        .get_or_init(|| ttf_parser::Face::parse(face.bytes(), 0).ok())
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_parses() {
        for face in Face::ALL {
            let font = font(face).unwrap_or_else(|| panic!("{face:?} does not parse"));
            assert_eq!(font.units_per_em(), 1000, "{face:?}");
        }
        for face in ExtraFace::ALL {
            let font = extra_font(face).unwrap_or_else(|| panic!("{face:?} does not parse"));
            assert_eq!(font.units_per_em(), 1000, "{face:?}");
        }
    }

    #[test]
    fn face_index_matches_all() {
        for (i, face) in Face::ALL.iter().enumerate() {
            assert_eq!(index(*face), i);
        }
    }

    /// Code points KaTeX measures that draw nothing, so they have no outline.
    const BLANK: [u32; 3] = [0x20, 0xa0, 0x2009];

    /// Measured glyphs the font has no glyph for. KaTeX's table lists the combining marks
    /// U+0302, U+0303, and U+0308 for Typewriter-Regular, but the font has no cmap entry for
    /// them (KaTeX falls back to Main-Regular for accents in `\texttt`).
    const NOT_IN_FONT: [(Face, u32); 3] = [
        (Face::TypewriterRegular, 0x302),
        (Face::TypewriterRegular, 0x303),
        (Face::TypewriterRegular, 0x308),
    ];

    #[test]
    fn every_measured_glyph_has_an_outline() {
        let mut missing = Vec::new();
        for face in Face::ALL {
            let font = font(face).expect("parses");
            for (code, _) in face.glyphs() {
                let Some(ch) = char::from_u32(*code) else {
                    missing.push(format!("{} U+{code:04X} not a char", face.name()));
                    continue;
                };
                let Some(id) = font.glyph_index(ch) else {
                    if !BLANK.contains(code) && !NOT_IN_FONT.contains(&(face, *code)) {
                        missing.push(format!("{} U+{code:04X} no glyph", face.name()));
                    }
                    continue;
                };
                let mut sink = Sink(0);
                if font.outline_glyph(id, &mut sink).is_none() && !BLANK.contains(code) {
                    missing.push(format!("{} U+{code:04X} no outline", face.name()));
                }
            }
        }
        assert!(missing.is_empty(), "glyphs without outlines: {missing:#?}");
    }

    /// Counts the segments of an outline.
    struct Sink(usize);

    impl ttf_parser::OutlineBuilder for Sink {
        fn move_to(&mut self, _: f32, _: f32) {
            self.0 += 1;
        }
        fn line_to(&mut self, _: f32, _: f32) {
            self.0 += 1;
        }
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
            self.0 += 1;
        }
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
            self.0 += 1;
        }
        fn close(&mut self) {}
    }
}
