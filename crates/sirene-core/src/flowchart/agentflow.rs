//! Agentflow's shape vocabulary (MAGF-4): its aliases, the shapes it removed, and the shapes it
//! allows. Follows Mermaid's `diagrams/agentflow/shapes.ts`.

use super::Shape;

/// What an `@{ shape }` name means in an agentflow diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Named {
    Shape(Shape),
    /// A shape agentflow removed: an error, drawn as a rounded rectangle.
    Removed,
    /// A Mermaid shape agentflow does not allow: a warning, drawn as a rounded rectangle.
    Unsupported,
}

/// Shapes agentflow removed.
const REMOVED: &[&str] = &[
    "doc",
    "stadium",
    "terminal",
    "circle",
    "trapezoid",
    "inv_trapezoid",
    "inv-trapezoid",
    "doublecircle",
    "double-circle",
    "typeDeclaration",
    "procs",
    "lean_left",
    "lean-left",
    "in-out",
    "cylinder",
    "ellipse",
    "odd",
    "tag-rect",
    "tagged-rectangle",
    "delay",
    "half-rounded-rectangle",
    "lin-rect",
    "lined-rectangle",
    "win-pane",
    "window-pane",
    "curv-trap",
    "curved-trapezoid",
];

/// The shape an agentflow `@{ shape }` name gives, or `None` for a name that is no shape at all.
pub(crate) fn named(name: &str) -> Option<Named> {
    let shape = match name {
        // Aliases, and the rounded default under its names.
        "task" | "round" | "rect" | "squareRect" | "roundedRect" | "connector" => Shape::Rounded,
        "tool" | "subroutine" | "subprocess" | "subproc" | "framed-rectangle" => Shape::FramedRect,
        "input" | "lean_right" | "lean-right" => Shape::LeanRight,
        "decision" | "diamond" => Shape::Diamond,
        "refdoc" | "lin-doc" | "lined-document" => Shape::LinedDocument,
        "action" | "hexagon" | "hex" => Shape::Hexagon,
        _ if REMOVED.contains(&name) => return Some(Named::Removed),
        _ => return Shape::from_name(name).map(|_| Named::Unsupported),
    };
    Some(Named::Shape(shape))
}

/// Mermaid's name for a shape drawn with brackets, for messages.
pub(crate) fn delimited_name(shape: Shape) -> &'static str {
    match shape {
        Shape::Stadium => "stadium",
        Shape::Circle => "circle",
        Shape::DoubleCircle => "doublecircle",
        Shape::Ellipse => "ellipse",
        Shape::Cylinder => "cylinder",
        Shape::Odd => "odd",
        Shape::TrapezoidBottom => "trapezoid",
        Shape::TrapezoidTop => "inv_trapezoid",
        Shape::LeanLeft => "lean_left",
        other => other.name(),
    }
}

pub(crate) fn removed(name: &str) -> String {
    format!("agentflow removed the `{name}` shape; drawing a rounded rectangle")
}

pub(crate) fn unsupported(name: &str) -> String {
    format!("agentflow does not support the `{name}` shape; drawing a rounded rectangle")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(named("task"), Some(Named::Shape(Shape::Rounded)));
        assert_eq!(named("tool"), Some(Named::Shape(Shape::FramedRect)));
        assert_eq!(named("input"), Some(Named::Shape(Shape::LeanRight)));
        assert_eq!(named("decision"), Some(Named::Shape(Shape::Diamond)));
        assert_eq!(named("refdoc"), Some(Named::Shape(Shape::LinedDocument)));
        assert_eq!(named("action"), Some(Named::Shape(Shape::Hexagon)));
        assert_eq!(named("cylinder"), Some(Named::Removed));
        assert_eq!(named("stadium"), Some(Named::Removed));
        assert_eq!(named("cloud"), Some(Named::Unsupported));
        assert_eq!(named("nope"), None);
    }
}
