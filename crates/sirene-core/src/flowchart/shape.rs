//! Node shapes (FLOWMO-5), named as in mermaid's shape table.

macro_rules! shapes {
    ($($variant:ident => $name:literal $([$($alias:literal),*])?,)*) => {
        /// A node shape. Every variant except `Ellipse` and the use case shapes has a mermaid
        /// `@{ shape }` name.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Shape {
            $($variant,)*
            /// The classic `(- -)` shape, which has no `@{ shape }` name.
            Ellipse,
            /// A use case diagram's stick figure actor, its label below.
            Actor,
            /// A use case diagram's outlined actor.
            HollowActor,
            /// A use case diagram's filled silhouette actor.
            AwesomeActor,
            /// A use case diagram's ellipse, roomier than `Ellipse`.
            UseCase,
        }

        const NAMES: &[(&str, Shape)] = &[
            $(($name, Shape::$variant), $($(($alias, Shape::$variant),)*)?)*
        ];

        impl Shape {
            /// Every shape a flowchart can name, in mermaid's documentation order, then
            /// `Ellipse`. The use case shapes are not among them.
            pub const ALL: &[Shape] = &[$(Shape::$variant,)* Shape::Ellipse];

            /// The mermaid short name, e.g. `fr-rect`. `Ellipse` is `ellipse`, and the use case
            /// shapes have mermaid's internal names, such as `usecaseActor`.
            pub fn name(self) -> &'static str {
                match self {
                    $(Shape::$variant => $name,)*
                    Shape::Ellipse => "ellipse",
                    Shape::Actor => "usecaseActor",
                    Shape::HollowActor => "usecaseActorHollow",
                    Shape::AwesomeActor => "usecaseActorAwesome",
                    Shape::UseCase => "usecaseEllipse",
                }
            }

            /// Whether the shape is one of a use case diagram's actors.
            pub fn is_actor(self) -> bool {
                matches!(self, Shape::Actor | Shape::HollowActor | Shape::AwesomeActor)
            }
        }
    };
}

shapes! {
    Rect => "rect" ["proc", "process", "rectangle"],
    Rounded => "rounded" ["event"],
    Stadium => "stadium" ["terminal", "pill"],
    FramedRect => "fr-rect" ["subprocess", "subproc", "framed-rectangle", "subroutine"],
    Cylinder => "cyl" ["db", "database", "cylinder"],
    DataStore => "datastore" ["data-store"],
    Folder => "folder" ["directory"],
    Bucket => "bucket",
    Console => "console",
    Browser => "browser",
    Person => "person",
    Circle => "circle" ["circ"],
    Bang => "bang",
    Cloud => "cloud",
    Diamond => "diam" ["decision", "diamond", "question"],
    Hexagon => "hex" ["hexagon", "prepare"],
    LeanRight => "lean-r" ["lean-right", "in-out"],
    LeanLeft => "lean-l" ["lean-left", "out-in"],
    TrapezoidBottom => "trap-b" ["priority", "trapezoid-bottom", "trapezoid"],
    TrapezoidTop => "trap-t" ["manual", "trapezoid-top", "inv-trapezoid"],
    DoubleCircle => "dbl-circ" ["double-circle"],
    Text => "text",
    NotchedRect => "notch-rect" ["card", "notched-rectangle"],
    LinedRect => "lin-rect" ["lined-rectangle", "lined-process", "lin-proc", "shaded-process"],
    SmallCircle => "sm-circ" ["start", "small-circle"],
    FramedCircle => "fr-circ" ["stop", "framed-circle"],
    Fork => "fork" ["join"],
    Hourglass => "hourglass" ["collate"],
    Brace => "brace" ["comment", "brace-l"],
    BraceRight => "brace-r",
    Braces => "braces",
    Bolt => "bolt" ["com-link", "lightning-bolt"],
    Document => "doc" ["document"],
    Delay => "delay" ["half-rounded-rectangle"],
    HorizontalCylinder => "h-cyl" ["das", "horizontal-cylinder"],
    LinedCylinder => "lin-cyl" ["disk", "lined-cylinder"],
    CurvedTrapezoid => "curv-trap" ["curved-trapezoid", "display"],
    DividedRect => "div-rect" ["div-proc", "divided-rectangle", "divided-process"],
    Triangle => "tri" ["extract", "triangle"],
    WindowPane => "win-pane" ["internal-storage", "window-pane"],
    FilledCircle => "f-circ" ["junction", "filled-circle"],
    NotchedPentagon => "notch-pent" ["loop-limit", "notched-pentagon"],
    FlippedTriangle => "flip-tri" ["manual-file", "flipped-triangle"],
    SlopedRect => "sl-rect" ["manual-input", "sloped-rectangle"],
    Documents => "docs" ["documents", "st-doc", "stacked-document"],
    StackedRect => "st-rect" ["procs", "processes", "stacked-rectangle"],
    BowTieRect => "bow-rect" ["stored-data", "bow-tie-rectangle"],
    CrossedCircle => "cross-circ" ["summary", "crossed-circle"],
    TaggedDocument => "tag-doc" ["tagged-document"],
    TaggedRect => "tag-rect" ["tagged-rectangle", "tag-proc", "tagged-process"],
    Flag => "flag" ["paper-tape"],
    Odd => "odd",
    LinedDocument => "lin-doc" ["lined-document"],
}

impl Shape {
    /// Looks up an `@{ shape }` name or alias. Names are case sensitive, as in mermaid.
    pub fn from_name(name: &str) -> Option<Shape> {
        NAMES.iter().find(|(n, _)| *n == name).map(|&(_, s)| s)
    }

    /// The classic delimiters that draw the shape, e.g. `("([", "])")` for `Stadium`; `None`
    /// for shapes written only with `@{ shape }`.
    pub fn delimiters(self) -> Option<(&'static str, &'static str)> {
        Some(match self {
            Shape::Rect => ("[", "]"),
            Shape::Rounded => ("(", ")"),
            Shape::Stadium => ("([", "])"),
            Shape::FramedRect => ("[[", "]]"),
            Shape::Cylinder => ("[(", ")]"),
            Shape::Circle => ("((", "))"),
            Shape::DoubleCircle => ("(((", ")))"),
            Shape::Ellipse => ("(-", "-)"),
            Shape::Odd => (">", "]"),
            Shape::Diamond => ("{", "}"),
            Shape::Hexagon => ("{{", "}}"),
            Shape::LeanRight => ("[/", "/]"),
            Shape::LeanLeft => ("[\\", "\\]"),
            Shape::TrapezoidBottom => ("[/", "\\]"),
            Shape::TrapezoidTop => ("[\\", "/]"),
            _ => return None,
        })
    }

    /// A name for people: the variant's words, e.g. `Framed rect` for `FramedRect`.
    pub fn title(self) -> String {
        let name = format!("{self:?}");
        let mut out = String::new();
        for (i, c) in name.chars().enumerate() {
            if i > 0 && c.is_uppercase() {
                out.push(' ');
                out.extend(c.to_lowercase());
            } else {
                out.push(c);
            }
        }
        out
    }

    /// The shape drawn by classic delimiters, given the opening and closing delimiter text.
    pub(crate) fn from_delimiters(open: &str, close: &str) -> Shape {
        match (open, close) {
            ("(((", _) => Shape::DoubleCircle,
            ("((", _) => Shape::Circle,
            ("([", _) => Shape::Stadium,
            ("(-", _) => Shape::Ellipse,
            ("(", _) => Shape::Rounded,
            ("[[", _) => Shape::FramedRect,
            ("[(", _) => Shape::Cylinder,
            ("[/", "\\]") => Shape::TrapezoidBottom,
            ("[/", _) => Shape::LeanRight,
            ("[\\", "/]") => Shape::TrapezoidTop,
            ("[\\", _) => Shape::LeanLeft,
            ("{{", _) => Shape::Hexagon,
            ("{", _) => Shape::Diamond,
            (">", _) => Shape::Odd,
            _ => Shape::Rect,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for &shape in Shape::ALL {
            if shape != Shape::Ellipse {
                assert_eq!(Shape::from_name(shape.name()), Some(shape), "{shape:?}");
            }
        }
        assert_eq!(Shape::Ellipse.name(), "ellipse");
        assert_eq!(Shape::from_name("ellipse"), None);
    }

    #[test]
    fn aliases() {
        assert_eq!(Shape::from_name("process"), Some(Shape::Rect));
        assert_eq!(Shape::from_name("subroutine"), Some(Shape::FramedRect));
        assert_eq!(Shape::from_name("db"), Some(Shape::Cylinder));
        assert_eq!(Shape::from_name("inv-trapezoid"), Some(Shape::TrapezoidTop));
        assert_eq!(
            Shape::from_name("lined-document"),
            Some(Shape::LinedDocument)
        );
        assert_eq!(Shape::from_name("Rect"), None);
        assert_eq!(Shape::from_name("nope"), None);
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = NAMES.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        let len = names.len();
        names.dedup();
        assert_eq!(names.len(), len);
    }

    #[test]
    fn delimiters() {
        let cases = [
            ("[", "]", Shape::Rect),
            ("(", ")", Shape::Rounded),
            ("([", "])", Shape::Stadium),
            ("[[", "]]", Shape::FramedRect),
            ("[(", ")]", Shape::Cylinder),
            ("((", "))", Shape::Circle),
            ("(((", ")))", Shape::DoubleCircle),
            ("(-", "-)", Shape::Ellipse),
            (">", "]", Shape::Odd),
            ("{", "}", Shape::Diamond),
            ("{{", "}}", Shape::Hexagon),
            ("[/", "/]", Shape::LeanRight),
            ("[\\", "\\]", Shape::LeanLeft),
            ("[/", "\\]", Shape::TrapezoidBottom),
            ("[\\", "/]", Shape::TrapezoidTop),
        ];
        for (open, close, shape) in cases {
            assert_eq!(Shape::from_delimiters(open, close), shape, "{open} {close}");
            assert_eq!(shape.delimiters(), Some((open, close)), "{shape:?}");
        }
        let classic = Shape::ALL
            .iter()
            .filter(|s| s.delimiters().is_some())
            .count();
        assert_eq!(classic, cases.len());
    }

    #[test]
    fn titles() {
        assert_eq!(Shape::Rect.title(), "Rect");
        assert_eq!(Shape::FramedRect.title(), "Framed rect");
        assert_eq!(Shape::TrapezoidBottom.title(), "Trapezoid bottom");
    }
}
