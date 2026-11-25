//! `.vsdx` drawing (VISIO-1 to VISIO-16). Packages are built from XML strings by the test-only
//! writer in `src/visio/testpkg.rs`; the expected numbers are worked by hand from VISIO-5 and
//! VISIO-7 (pages are 96 px per inch, y flipped). They show sirene follows its spec, not that it
//! matches Visio (VISIO-16).

#[path = "../src/visio/testpkg.rs"]
mod testpkg;

use sirene_core::scene::{Approx, Color, Primitive, Scene, Segment, svg};
use sirene_core::visio::{self, Visio};
use testpkg::*;

fn open(bytes: &[u8]) -> Visio {
    visio::read(bytes).unwrap().0
}

fn scene(bytes: &[u8], page: usize) -> Scene {
    open(bytes).with_page(page).unwrap().scene(&Approx).unwrap()
}

fn svg_of(bytes: &[u8]) -> String {
    svg::write(&scene(bytes, 0))
}

/// The numbers in an SVG path's `d`.
fn nums(d: &str) -> Vec<f32> {
    d.replace(|c: char| c.is_ascii_alphabetic(), " ")
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect()
}

/// The `d` of every `<path>` in an SVG, in paint order.
fn paths(svg: &str) -> Vec<String> {
    let doc = roxmltree::Document::parse(svg).unwrap();
    doc.descendants()
        .filter(|n| n.has_tag_name("path"))
        .map(|n| n.attribute("d").unwrap().to_owned())
        .collect()
}

fn path_attr(svg: &str, i: usize, attr: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(svg).unwrap();
    doc.descendants()
        .filter(|n| n.has_tag_name("path"))
        .nth(i)
        .and_then(|n| n.attribute(attr).map(str::to_owned))
}

fn assert_close(got: &[f32], want: &[f32]) {
    assert_eq!(got.len(), want.len(), "{got:?} vs {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 0.02, "{got:?} vs {want:?}");
    }
}

/// A one-line summary per element, for snapshots.
fn summary(scene: &Scene) -> String {
    let mut out = format!("{}x{}\n", scene.width, scene.height);
    for e in &scene.elements {
        match &e.primitive {
            Primitive::Path { segments, fill, stroke, .. } => {
                let hex = |c: &Color| format!("#{:02x}{:02x}{:02x}/{}", c.r, c.g, c.b, c.a);
                out.push_str(&format!(
                    "path fill={} stroke={}",
                    fill.as_ref().map_or("none".into(), hex),
                    stroke.as_ref().map_or("none".into(), |s| format!(
                        "{}:{}:{:?}",
                        hex(&s.color),
                        s.width,
                        s.dash
                    ))
                ));
                for s in segments {
                    match s {
                        Segment::Move(p) => out.push_str(&format!(" M{:.1},{:.1}", p.x, p.y)),
                        Segment::Line(p) => out.push_str(&format!(" L{:.1},{:.1}", p.x, p.y)),
                        Segment::Cubic { c1, c2, to } => out.push_str(&format!(
                            " C{:.1},{:.1},{:.1},{:.1},{:.1},{:.1}",
                            c1.x, c1.y, c2.x, c2.y, to.x, to.y
                        )),
                        Segment::Arc { to, .. } => out.push_str(&format!(" A{:.1},{:.1}", to.x, to.y)),
                        Segment::Close => out.push_str(" Z"),
                    }
                }
                out.push('\n');
            }
            Primitive::Text { text, center, size, color, style } => out.push_str(&format!(
                "text {text:?} at {:.1},{:.1} size={:.2} color=#{:02x}{:02x}{:02x} bold={} italic={}\n",
                center.x, center.y, size, color.r, color.g, color.b, style.bold, style.italic
            )),
        }
    }
    out
}

macro_rules! snap {
    ($name:expr, $scene:expr) => {
        insta::assert_snapshot!($name, summary(&$scene));
    };
}

/// A 6 by 4 inch page (576 by 384 px) holding `shapes`.
fn page(shapes: &str) -> Builder {
    Builder::new().page(6.0, 4.0, shapes)
}

fn rect_shape(id: u32, extra: &str, pin: (f64, f64), size: (f64, f64)) -> String {
    shape(
        id,
        r#"Type="Shape""#,
        &format!(
            "{}{}{}",
            xform(pin.0, pin.1, size.0, size.1),
            extra,
            rect_geometry(size.0, size.1)
        ),
    )
}

// ---- geometry ----

#[test]
fn rectangle() {
    let bytes = page(&rect_shape(1, "", (2.0, 1.0), (2.0, 1.0))).build();
    let s = scene(&bytes, 0);
    assert_eq!((s.width, s.height), (576.0, 384.0));
    // The shape's origin is its pin less the pin's offset: page (1, 0.5), so px (96, 336).
    let svg = svg::write(&s);
    let p = paths(&svg);
    assert_eq!(p, ["M96 336 L288 336 L288 240 L96 240 L96 336 Z"]);
    // Defaults: white fill (palette 1), black line (0) of 0.01 in = 0.96 px.
    assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some("#ffffff"));
    assert_eq!(path_attr(&svg, 0, "stroke").as_deref(), Some("#000000"));
    assert_eq!(path_attr(&svg, 0, "stroke-width").as_deref(), Some("0.96"));
    snap!("rectangle", s);
}

#[test]
fn rotated_shape() {
    // Pin (3, 2), 2 by 1, pin at the center, rotated 90 degrees counterclockwise: the local
    // corners (0,0), (2,0), (2,1), (0,1) land at page (3.5,1), (3.5,3), (2.5,3), (2.5,1).
    let shape = rect_shape(
        1,
        &cell("Angle", std::f64::consts::FRAC_PI_2),
        (3.0, 2.0),
        (2.0, 1.0),
    );
    let svg = svg_of(&page(&shape).build());
    assert_close(
        &nums(&paths(&svg)[0]),
        &[
            336.0, 288.0, 336.0, 96.0, 240.0, 96.0, 240.0, 288.0, 336.0, 288.0,
        ],
    );
    snap!("rotated", scene(&page(&shape).build(), 0));
}

#[test]
fn flipped_shapes() {
    // FlipX: x' = 2 - x, so local (0,0) is at page (3, 1.5) and (2,0) at (1, 1.5).
    let fx = rect_shape(1, &cell("FlipX", 1), (2.0, 2.0), (2.0, 1.0));
    let svg = svg_of(&page(&fx).build());
    assert_close(
        &nums(&paths(&svg)[0]),
        &[
            288.0, 240.0, 96.0, 240.0, 96.0, 144.0, 288.0, 144.0, 288.0, 240.0,
        ],
    );
    // FlipY: y' = 1 - y: local (0,0) is at page (1, 2.5), (0,1) at (1, 1.5).
    let fy = rect_shape(1, &cell("FlipY", 1), (2.0, 2.0), (2.0, 1.0));
    let svg = svg_of(&page(&fy).build());
    assert_close(
        &nums(&paths(&svg)[0]),
        &[
            96.0, 144.0, 288.0, 144.0, 288.0, 240.0, 96.0, 240.0, 96.0, 144.0,
        ],
    );
    snap!("flipped", scene(&page(&fx).build(), 0));
}

#[test]
fn group_children_use_the_group_frame() {
    // Group pin (4,2), 2 by 2, pin at its center: its origin is page (3,1). The child's origin
    // is its pin (0.5,0.5) less (0.5,0.5): the group's origin, so page (3,1) = px (288,288).
    let child = rect_shape(2, "", (0.5, 0.5), (1.0, 1.0));
    let group = shape(
        1,
        r#"Type="Group""#,
        &format!("{}<Shapes>{child}</Shapes>", xform(4.0, 2.0, 2.0, 2.0)),
    );
    let bytes = page(&group).build();
    let svg = svg_of(&bytes);
    assert_close(
        &nums(&paths(&svg)[0]),
        &[
            288.0, 288.0, 384.0, 288.0, 384.0, 192.0, 288.0, 192.0, 288.0, 288.0,
        ],
    );
    snap!("group", scene(&bytes, 0));
}

#[test]
fn rotated_group_rotates_its_children() {
    // A group rotated 90 degrees about its center (pin (3,2), 2 by 2); a child at the group's
    // local (0,0) to (1,1) maps to page x in [3,4]... the local origin goes to (4,1), local
    // x runs up the page and local y runs left.
    let child = rect_shape(2, "", (0.5, 0.5), (1.0, 1.0));
    let group = shape(
        1,
        r#"Type="Group""#,
        &format!(
            "{}{}<Shapes>{child}</Shapes>",
            xform(3.0, 2.0, 2.0, 2.0),
            cell("Angle", std::f64::consts::FRAC_PI_2)
        ),
    );
    let svg = svg_of(&page(&group).build());
    // Local (0,0): dx=-1, dy=-1 -> (3 + 1, 2 - 1) = (4, 1); (1,0): dx=0, dy=-1 -> (4, 2);
    // (1,1): (3, 2); (0,1): (3, 1).
    assert_close(
        &nums(&paths(&svg)[0]),
        &[
            384.0, 288.0, 384.0, 192.0, 288.0, 192.0, 288.0, 288.0, 384.0, 288.0,
        ],
    );
}

#[test]
fn ellipse_row() {
    // Center (1,1), right end (2,1), top end (1,2) in a 2 by 2 shape at pin (3,2): page center
    // (3,2) = px (288,192); the path starts at the right end, px (384,192), and its first curve
    // ends at the top, px (288,96).
    let geom = geometry(
        0,
        "",
        &[row(
            "Ellipse",
            1,
            &[
                ("X", 1.0),
                ("Y", 1.0),
                ("A", 2.0),
                ("B", 1.0),
                ("C", 1.0),
                ("D", 2.0),
            ],
        )],
    );
    let sh = shape(1, "", &format!("{}{geom}", xform(3.0, 2.0, 2.0, 2.0)));
    let bytes = page(&sh).build();
    let svg = svg_of(&bytes);
    let d = &paths(&svg)[0];
    assert!(d.starts_with("M384 192 C"), "{d}");
    assert_eq!(d.matches('C').count(), 4);
    let n = nums(d);
    assert_close(&n[6..8], &[288.0, 96.0]);
    assert!(d.ends_with('Z'));
    snap!("ellipse", scene(&bytes, 0));
}

#[test]
fn polyline_and_lines() {
    let geom = geometry(
        0,
        r#"<Cell N="NoFill" V="1"/>"#,
        &[
            row("MoveTo", 1, &[("X", 0.0), ("Y", 0.0)]),
            String::from(
                r#"<Row T="PolylineTo" IX="2"><Cell N="X" V="3"/><Cell N="Y" V="0"/><Cell N="A" V="" F="POLYLINE(1, 1, 1, 1, 2, 0.5)"/></Row>"#,
            ),
        ],
    );
    let sh = shape(1, "", &format!("{}{geom}", xform(2.0, 1.0, 3.0, 1.0)));
    let bytes = page(&sh).build();
    let svg = svg_of(&bytes);
    // Origin: pin (2,1) less (1.5, 0.5) = (0.5, 0.5): px (48, 336).
    assert_eq!(paths(&svg)[0], "M48 336 L144 240 L240 288 L336 336");
    assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some("none"));
    snap!("polyline", scene(&bytes, 0));
}

#[test]
fn circular_arc_through_the_bulge() {
    // From local (0,0) to (2,0) with a bulge of 1 (a semicircle bowing up): the shape's origin
    // is page (1,1). The first quarter ends at the bulge's top, local (1,1) = page (2,2) =
    // px (192,192); the end is px (288,288).
    let geom = geometry(
        0,
        r#"<Cell N="NoFill" V="1"/>"#,
        &[
            row("MoveTo", 1, &[("X", 0.0), ("Y", 0.0)]),
            row("ArcTo", 2, &[("X", 2.0), ("Y", 0.0), ("A", 1.0)]),
        ],
    );
    let body = format!(
        "{}{}{}{}{}{}{geom}",
        cell("PinX", 1.0),
        cell("PinY", 1.0),
        cell("Width", 2.0),
        cell("Height", 1.0),
        cell("LocPinX", 0.0),
        cell("LocPinY", 0.0)
    );
    let sh = shape(1, "", &body);
    let bytes = page(&sh).build();
    let svg = svg_of(&bytes);
    let n = nums(&paths(&svg)[0]);
    assert_eq!(n.len(), 2 + 6 + 6);
    assert_close(&n[2..4], &[96.0, 288.0 - 0.5523 * 96.0]);
    assert_close(&n[6..8], &[192.0, 192.0]);
    assert_close(&n[12..14], &[288.0, 288.0]);
    snap!("arc", scene(&bytes, 0));
}

#[test]
fn elliptical_arc() {
    // The upper half of an ellipse with semi-axes 2 and 1 about local (2,0): the first quarter
    // ends at its top, local (2,1).
    let geom = geometry(
        0,
        r#"<Cell N="NoFill" V="1"/>"#,
        &[
            row("MoveTo", 1, &[("X", 0.0), ("Y", 0.0)]),
            row(
                "EllipticalArcTo",
                2,
                &[
                    ("X", 4.0),
                    ("Y", 0.0),
                    ("A", 2.0),
                    ("B", 1.0),
                    ("C", 0.0),
                    ("D", 2.0),
                ],
            ),
        ],
    );
    let sh = shape(1, "", &format!("{}{geom}", xform(2.0, 1.0, 4.0, 1.0)));
    let bytes = page(&sh).build();
    let n = nums(&paths(&svg_of(&bytes))[0]);
    // Origin: pin (2,1) less (2,0.5) = page (0,0.5): px (0,336). Top at local (2,1): page
    // (2,1.5): px (192,240).
    assert_close(&n[0..2], &[0.0, 336.0]);
    assert_close(&n[6..8], &[192.0, 240.0]);
    assert_close(&n[n.len() - 2..], &[384.0, 336.0]);
    snap!("elliptical_arc", scene(&bytes, 0));
}

#[test]
fn spline_and_nurbs_end_where_they_say() {
    let geom = geometry(
        0,
        r#"<Cell N="NoFill" V="1"/>"#,
        &[
            row("MoveTo", 1, &[("X", 0.0), ("Y", 0.0)]),
            row("SplineStart", 2, &[("X", 1.0), ("Y", 2.0), ("D", 3.0)]),
            row("SplineKnot", 3, &[("X", 3.0), ("Y", 2.0)]),
            row("SplineKnot", 4, &[("X", 4.0), ("Y", 0.0)]),
            String::from(
                r#"<Row T="NURBSTo" IX="5"><Cell N="X" V="6"/><Cell N="Y" V="0"/><Cell N="B" V="1"/><Cell N="D" V="1"/><Cell N="E" V="" F="NURBS(1, 2, 1, 1, 5, 2, 0.5, 1)"/></Row>"#,
            ),
        ],
    );
    let sh = shape(1, "", &format!("{}{geom}", xform(3.0, 1.0, 6.0, 2.0)));
    let bytes = page(&sh).build();
    let n = nums(&paths(&svg_of(&bytes))[0]);
    // Origin: pin (3,1) less (3,1) = page (0,0): px (0,384). The spline's end (4,0) is at
    // px (384,384); the NURBS ends at (6,0): px (576,384).
    assert_close(&n[0..2], &[0.0, 384.0]);
    assert_close(&n[n.len() - 2..], &[576.0, 384.0]);
    // Every point stays inside the control hull: y in [0, 2] inches, i.e. px 192..=384.
    for p in n.chunks(2) {
        assert!(p[1] >= 191.9 && p[1] <= 384.1, "{p:?}");
    }
    assert!(n.len() > 40);
}

#[test]
fn no_show_sections_and_no_line_flags() {
    let hidden = geometry(
        0,
        r#"<Cell N="NoShow" V="1"/>"#,
        &[
            row("MoveTo", 1, &[("X", 0.0), ("Y", 0.0)]),
            row("LineTo", 2, &[("X", 1.0), ("Y", 1.0)]),
        ],
    );
    let shown = geometry(
        1,
        r#"<Cell N="NoLine" V="1"/>"#,
        &[
            row("MoveTo", 1, &[("X", 0.0), ("Y", 0.0)]),
            row("LineTo", 2, &[("X", 1.0), ("Y", 0.0)]),
            row("LineTo", 3, &[("X", 1.0), ("Y", 1.0)]),
            row("LineTo", 4, &[("X", 0.0), ("Y", 0.0)]),
        ],
    );
    let sh = shape(
        1,
        "",
        &format!("{}{hidden}{shown}", xform(2.0, 2.0, 1.0, 1.0)),
    );
    let svg = svg_of(&page(&sh).build());
    assert_eq!(paths(&svg).len(), 1);
    assert_eq!(path_attr(&svg, 0, "stroke"), None);
    assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some("#ffffff"));
}

// ---- line and fill ----

#[test]
fn line_and_fill_cells() {
    let extra = format!(
        "{}{}{}{}",
        cell("LineWeight", 0.05),
        r##"<Cell N="LineColor" V="#0000ff"/>"##,
        cell("LinePattern", 2),
        r##"<Cell N="FillForegnd" V="#ff0000"/><Cell N="FillForegndTrans" V="0.5"/>"##,
    );
    let bytes = page(&rect_shape(1, &extra, (2.0, 1.0), (2.0, 1.0))).build();
    let svg = svg_of(&bytes);
    assert_eq!(path_attr(&svg, 0, "stroke-width").as_deref(), Some("4.8"));
    assert_eq!(path_attr(&svg, 0, "stroke").as_deref(), Some("#0000ff"));
    assert_eq!(
        path_attr(&svg, 0, "stroke-dasharray").as_deref(),
        Some("19.2 9.6")
    );
    assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some("#ff0000"));
    assert_eq!(path_attr(&svg, 0, "fill-opacity").as_deref(), Some("0.5"));
    let none = rect_shape(
        1,
        &(cell("LinePattern", 0) + &cell("FillPattern", 0)),
        (2.0, 1.0),
        (2.0, 1.0),
    );
    let s = scene(&page(&none).build(), 0);
    assert!(s.elements.is_empty());
}

#[test]
fn indexed_and_palette_colors() {
    for (v, hex) in [
        ("2", "#ff0000"),
        ("14", "#c0c0c0"),
        ("23", "#1a1a1a"),
        ("24", "#102030"),
        ("99", "#000000"),
    ] {
        let extra = format!(r#"<Cell N="FillForegnd" V="{v}"/>"#);
        let b = page(&rect_shape(1, &extra, (2.0, 1.0), (2.0, 1.0)))
            .colors(r##"<ColorEntry IX="24" RGB="#102030"/>"##);
        let svg = svg_of(&b.build());
        assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some(hex), "V={v}");
    }
}

#[test]
fn theme_colors_resolve_through_quickstyle() {
    let extra = r#"<Cell N="FillForegnd" V="Themed" F="THEMEVAL()"/><Cell N="QuickStyleFillColor" V="2"/>
        <Cell N="LineColor" V="Themed" F="THEMEVAL()"/><Cell N="QuickStyleLineColor" V="0"/>"#;
    let b = page(&rect_shape(1, extra, (2.0, 1.0), (2.0, 1.0))).theme(
        r#"<a:dk1><a:sysClr val="windowText" lastClr="111111"/></a:dk1><a:accent1><a:srgbClr val="AA0000"/></a:accent1><a:accent2><a:srgbClr val="00AA00"/></a:accent2>"#,
    );
    let svg = svg_of(&b.build());
    assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some("#00aa00"));
    assert_eq!(path_attr(&svg, 0, "stroke").as_deref(), Some("#111111"));
}

#[test]
fn cached_values_beat_formulas() {
    // PinX is 2 whatever its formula says; a THEMEVAL cell with a usable V keeps the V.
    let extra = r##"<Cell N="FillForegnd" V="#123456" F="THEMEVAL(&quot;x&quot;)"/>"##;
    let sh = shape(
        1,
        "",
        &format!(
            r#"<Cell N="PinX" V="2" F="Width*100"/>{}{extra}{}"#,
            xform(2.0, 1.0, 2.0, 1.0).replace(&cell("PinX", 2), ""),
            rect_geometry(2.0, 1.0)
        ),
    );
    let svg = svg_of(&page(&sh).build());
    assert_eq!(
        paths(&svg)[0],
        "M96 336 L288 336 L288 240 L96 240 L96 336 Z"
    );
    assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some("#123456"));
}

#[test]
fn fill_patterns_draw_solid_with_a_warning() {
    let bytes = page(&rect_shape(
        1,
        &cell("FillPattern", 25),
        (2.0, 1.0),
        (2.0, 1.0),
    ))
    .build();
    let v = open(&bytes);
    let problems = v.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].message.contains("solid"));
    assert_eq!(
        path_attr(&svg_of(&bytes), 0, "fill").as_deref(),
        Some("#ffffff")
    );
}

// ---- masters and styles ----

#[test]
fn master_instances_inherit_cells_and_geometry() {
    let master = shape(
        1,
        "",
        &format!(
            "{}{}{}{}",
            xform(1.0, 0.5, 2.0, 1.0),
            cell("LineWeight", 0.05),
            r##"<Cell N="FillForegnd" V="#ff0000"/>"##,
            rect_geometry(2.0, 1.0)
        ),
    );
    // The instance gives only its pin; size, pin offset, geometry, line, and fill are the master's.
    let instance = shape(
        10,
        r#"Master="5""#,
        &(cell("PinX", 2.0) + &cell("PinY", 1.0)),
    );
    let bytes = page(&instance).master(5, &master).build();
    let svg = svg_of(&bytes);
    assert_eq!(
        paths(&svg)[0],
        "M96 336 L288 336 L288 240 L96 240 L96 336 Z"
    );
    assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some("#ff0000"));
    assert_eq!(path_attr(&svg, 0, "stroke-width").as_deref(), Some("4.8"));
    snap!("master", scene(&bytes, 0));
}

#[test]
fn a_resized_instance_scales_inherited_geometry() {
    let master = shape(
        1,
        "",
        &format!("{}{}", xform(1.0, 0.5, 2.0, 1.0), rect_geometry(2.0, 1.0)),
    );
    // Width 4 (twice the master's): the inherited rectangle spans 4 inches; its origin is the pin
    // (2,1) less (2, 0.5).
    let instance = shape(
        10,
        r#"Master="5""#,
        &(cell("PinX", 2.0) + &cell("PinY", 1.0) + &cell("Width", 4.0) + &cell("LocPinX", 2.0)),
    );
    let bytes = page(&instance).master(5, &master).build();
    assert_eq!(
        paths(&svg_of(&bytes))[0],
        "M0 336 L384 336 L384 240 L0 240 L0 336 Z"
    );
}

#[test]
fn master_group_children_use_master_shape() {
    let master = shape(
        1,
        r#"Type="Group""#,
        &format!(
            "{}<Shapes>{}</Shapes>",
            xform(1.0, 1.0, 2.0, 2.0),
            shape(
                2,
                "",
                &format!("{}{}", xform(1.0, 1.0, 2.0, 2.0), rect_geometry(2.0, 2.0))
            )
        ),
    );
    let child = shape(7, r#"MasterShape="2""#, &xform(1.0, 1.0, 2.0, 2.0));
    let instance = shape(
        10,
        r#"Type="Group" Master="5""#,
        &format!("{}<Shapes>{child}</Shapes>", xform(3.0, 2.0, 2.0, 2.0)),
    );
    let bytes = page(&instance).master(5, &master).build();
    let svg = svg_of(&bytes);
    // The group's origin is page (2,1); the child's is the same: px (192,288) to (384,96).
    assert_eq!(
        paths(&svg)[0],
        "M192 288 L384 288 L384 96 L192 96 L192 288 Z"
    );
}

#[test]
fn style_sheets_supply_missing_cells_through_their_chain() {
    let styles = r##"
        <StyleSheet ID="0" LineStyle="0" FillStyle="0" TextStyle="0"><Cell N="LinePattern" V="3"/><Cell N="LineWeight" V="0.02"/></StyleSheet>
        <StyleSheet ID="3" LineStyle="0" FillStyle="0" TextStyle="0"><Cell N="LineColor" V="#0000ff"/></StyleSheet>
        <StyleSheet ID="4" LineStyle="3" FillStyle="3" TextStyle="3"><Cell N="FillForegnd" V="#00ff00"/></StyleSheet>"##;
    let sh = rect_shape(1, "", (2.0, 1.0), (2.0, 1.0)).replace(
        r#"Type="Shape""#,
        r#"Type="Shape" LineStyle="4" FillStyle="4""#,
    );
    let svg = svg_of(&page(&sh).styles(styles).build());
    // Fill from style 4; line color from its parent 3; pattern and weight from 0 above that.
    assert_eq!(path_attr(&svg, 0, "fill").as_deref(), Some("#00ff00"));
    assert_eq!(path_attr(&svg, 0, "stroke").as_deref(), Some("#0000ff"));
    assert_eq!(path_attr(&svg, 0, "stroke-width").as_deref(), Some("1.92"));
    assert_eq!(
        path_attr(&svg, 0, "stroke-dasharray").as_deref(),
        Some("1.92 3.84")
    );
}

#[test]
fn style_cycles_are_cut() {
    let styles = r#"<StyleSheet ID="1" LineStyle="2"/><StyleSheet ID="2" LineStyle="1"/>"#;
    let sh = rect_shape(1, "", (2.0, 1.0), (2.0, 1.0))
        .replace(r#"Type="Shape""#, r#"Type="Shape" LineStyle="1""#);
    let svg = svg_of(&page(&sh).styles(styles).build());
    assert_eq!(paths(&svg).len(), 1);
}

// ---- text ----

fn text_shape(attrs: &str, text: &str, extra: &str) -> String {
    shape(
        1,
        attrs,
        &format!("{}{extra}<Text>{text}</Text>", xform(3.0, 2.0, 4.0, 2.0)),
    )
}

fn texts(s: &Scene) -> Vec<(String, f32, f32, f32, Color, bool, bool)> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text {
                text,
                center,
                size,
                color,
                style,
            } => Some((
                text.clone(),
                center.x,
                center.y,
                *size,
                *color,
                style.bold,
                style.italic,
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn centered_text_with_character_formatting() {
    let chars = r##"<Section N="Character" IX="0"><Row IX="0"><Cell N="Size" V="0.2"/><Cell N="Color" V="#ff0000"/><Cell N="Style" V="7"/></Row></Section>"##;
    let bytes = page(&text_shape("", "Hello", chars)).build();
    let s = scene(&bytes, 0);
    let t = texts(&s);
    assert_eq!(t.len(), 1);
    let (text, x, y, size, color, bold, italic) = &t[0];
    // The block is the whole shape: center page (3,2) = px (288,192); one line, middle aligned.
    assert_eq!(text, "Hello");
    assert!(
        (x - 288.0).abs() < 0.01 && (y - 192.0).abs() < 0.01,
        "{x} {y}"
    );
    assert!((size - 19.2).abs() < 0.01);
    assert_eq!(*color, Color::rgb(0xff0000));
    assert!(*bold && *italic);
    // Underline (bit 4): a path under the text, 5 characters of 0.6 em wide at 19.2 px = 57.6.
    let under: Vec<_> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Path { segments, .. } => Some(segments.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(under.len(), 1);
    if let [Segment::Move(a), Segment::Line(b)] = under[0][..] {
        assert!(
            (b.x - a.x - 57.6).abs() < 0.1 && (a.x - 259.2).abs() < 0.1,
            "{a:?} {b:?}"
        );
        assert!(a.y > 192.0);
    } else {
        panic!("{:?}", under[0]);
    }
    assert!(svg::write(&s).contains("font-weight=\"bold\""));
    snap!("text", s);
}

#[test]
fn paragraph_alignment_and_vertical_alignment() {
    let left =
        r#"<Section N="Paragraph" IX="0"><Row IX="0"><Cell N="HorzAlign" V="0"/></Row></Section>"#;
    let right =
        r#"<Section N="Paragraph" IX="0"><Row IX="0"><Cell N="HorzAlign" V="2"/></Row></Section>"#;
    let top = format!("{}{}", left, cell("VerticalAlign", 0));
    let bottom = format!("{}{}", right, cell("VerticalAlign", 2));
    let size =
        r#"<Section N="Character" IX="0"><Row IX="0"><Cell N="Size" V="0.2"/></Row></Section>"#;
    let at = |extra: &str| {
        let s = scene(
            &page(&text_shape("", "Hi", &format!("{size}{extra}"))).build(),
            0,
        );
        let t = texts(&s);
        (t[0].1, t[0].2)
    };
    // "Hi" is 2 * 0.6 * 19.2 = 23.04 px; the block spans px x 96..480, y 96..288, margins
    // 0.0556 in = 5.34 px; a line is 1.2 * 19.2 = 23.04 px tall.
    let m = 0.0556 * 96.0;
    let (lx, ly) = at(&top);
    assert!((lx - (96.0 + m + 11.52)).abs() < 0.05, "{lx}");
    assert!((ly - (96.0 + m + 11.52)).abs() < 0.05, "{ly}");
    let (rx, ry) = at(&bottom);
    assert!((rx - (480.0 - m - 11.52)).abs() < 0.05, "{rx}");
    assert!((ry - (288.0 - m - 11.52)).abs() < 0.05, "{ry}");
}

#[test]
fn text_wraps_in_the_block_and_breaks_on_newlines() {
    let size =
        r#"<Section N="Character" IX="0"><Row IX="0"><Cell N="Size" V="0.2"/></Row></Section>"#;
    // The block is 1.2 in wide less margins (104.3 px): "aaaa bbbb" is 9 * 11.52 = 103.7 px
    // fits; "aaaa bbbb cccc" does not.
    let narrow = format!(
        "{}{}{}{}{}",
        size,
        cell("TxtWidth", 1.2),
        cell("TxtLocPinX", 0.6),
        cell("TxtHeight", 2.0),
        cell("TxtPinX", 2.0)
    );
    let s = scene(&page(&text_shape("", "aaaa bbbb cccc", &narrow)).build(), 0);
    let t = texts(&s);
    assert_eq!(
        t.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(),
        ["aaaa bbbb", "cccc"]
    );
    assert!(
        t[1].2 - t[0].2 > 23.0 && t[1].2 - t[0].2 < 23.1,
        "{:?}",
        (t[0].2, t[1].2)
    );
    let lines = scene(&page(&text_shape("", "one\ntwo", size)).build(), 0);
    assert_eq!(
        texts(&lines)
            .iter()
            .map(|t| t.0.clone())
            .collect::<Vec<_>>(),
        ["one", "two"]
    );
}

#[test]
fn character_runs_switch_at_cp_markers_and_fields_read_cached_text() {
    let chars = r##"<Section N="Character" IX="0"><Row IX="0"><Cell N="Size" V="0.2"/></Row><Row IX="1"><Cell N="Size" V="0.2"/><Cell N="Style" V="1"/><Cell N="Color" V="#00ff00"/></Row></Section>"##;
    let s = scene(
        &page(&text_shape(
            "",
            r#"plain <cp IX="1"/>bold <fld IX="2">42</fld>"#,
            chars,
        ))
        .build(),
        0,
    );
    let t = texts(&s);
    // Runs of one format on a line are one text item: "bold " and the field's "42" are bold.
    assert_eq!(t.len(), 2, "{t:?}");
    assert_eq!((t[0].0.as_str(), t[0].5), ("plain ", false));
    assert_eq!(
        (t[1].0.as_str(), t[1].5, t[1].4),
        ("bold 42", true, Color::rgb(0x00ff00))
    );
}

#[test]
fn text_comes_from_the_master_and_styles() {
    let master = shape(
        1,
        "",
        &format!("{}<Text>from master</Text>", xform(1.0, 1.0, 2.0, 1.0)),
    );
    let styles = r##"<StyleSheet ID="2" LineStyle="0" FillStyle="0" TextStyle="0"><Section N="Character" IX="0"><Row IX="0"><Cell N="Size" V="0.25"/><Cell N="Color" V="#0000ff"/></Row></Section></StyleSheet>"##;
    let inst = shape(
        10,
        r#"Master="5" TextStyle="2""#,
        &(cell("PinX", 3.0) + &cell("PinY", 2.0)),
    );
    let s = scene(&page(&inst).master(5, &master).styles(styles).build(), 0);
    let t = texts(&s);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].0, "from master");
    assert!((t[0].3 - 24.0).abs() < 0.01);
    assert_eq!(t[0].4, Color::rgb(0x0000ff));
}

#[test]
fn rotated_text_is_drawn_upright_with_a_warning() {
    let sh = text_shape("", "turn", &cell("Angle", std::f64::consts::FRAC_PI_2));
    let b = page(&sh).build();
    let p = open(&b).problems();
    assert!(p.iter().any(|d| d.message.contains("upright")), "{p:?}");
    assert_eq!(texts(&scene(&b, 0)).len(), 1);
}

// ---- connectors, layers, backgrounds, foreign shapes, pages ----

#[test]
fn connector_draws_its_geometry_with_an_arrowhead() {
    let geom = geometry(
        0,
        r#"<Cell N="NoFill" V="1"/>"#,
        &[
            row("MoveTo", 1, &[("X", 0.0), ("Y", 0.0)]),
            row("LineTo", 2, &[("X", 2.0), ("Y", 0.0)]),
        ],
    );
    let sh = shape(
        1,
        r#"Type="Shape""#,
        &format!(
            "{}{}{geom}{}",
            xform(2.0, 1.0, 2.0, 0.0),
            cell("EndArrow", 4),
            r#"<Cell N="BeginX" V="1"/>"#
        ),
    );
    let bytes = page(&sh).build();
    let s = scene(&bytes, 0);
    let svg = svg::write(&s);
    let p = paths(&svg);
    assert_eq!(p.len(), 2);
    // Origin: pin (2,1) less (1, 0): page (1,1) = px (96, 288); the line runs to (288, 288).
    assert_eq!(p[0], "M96 288 L288 288");
    // The arrow's tip is the line's end.
    assert!(p[1].starts_with("M288 288 L"), "{}", p[1]);
    assert_eq!(path_attr(&svg, 1, "fill").as_deref(), Some("#000000"));
    snap!("connector", s);
}

#[test]
fn hidden_layers_hide_their_shapes() {
    let layers = r#"<Section N="Layer"><Row IX="0"><Cell N="Visible" V="1"/></Row><Row IX="1"><Cell N="Visible" V="0"/></Row></Section>"#;
    let on = rect_shape(1, &cell("LayerMember", "0"), (1.0, 1.0), (1.0, 1.0));
    let off = rect_shape(2, &cell("LayerMember", "1"), (3.0, 1.0), (1.0, 1.0));
    let both = rect_shape(3, &cell("LayerMember", "1;0"), (5.0, 1.0), (1.0, 1.0));
    let none = rect_shape(4, "", (1.0, 3.0), (1.0, 1.0));
    let b = Builder::new().page_with(
        "",
        &format!(r#"<Cell N="PageWidth" V="6"/><Cell N="PageHeight" V="4"/>{layers}"#),
        &format!("{on}{off}{both}{none}"),
    );
    let p = paths(&svg_of(&b.build()));
    assert_eq!(p.len(), 3);
    assert!(p[0].starts_with("M48 "), "{p:?}");
    assert!(p[1].starts_with("M432 "), "{p:?}");
}

#[test]
fn background_pages_draw_first_and_are_not_outputs() {
    let back = rect_shape(1, "", (1.0, 1.0), (1.0, 1.0));
    let front = rect_shape(2, "", (4.0, 3.0), (1.0, 1.0));
    let b = Builder::new()
        .page_with(
            r#"BackPage="1""#,
            r#"<Cell N="PageWidth" V="6"/><Cell N="PageHeight" V="4"/>"#,
            &front,
        )
        .page_with(
            r#"Background="1""#,
            r#"<Cell N="PageWidth" V="6"/><Cell N="PageHeight" V="4"/>"#,
            &back,
        );
    let v = open(&b.build());
    assert_eq!(v.page_count(), 1);
    let s = v.with_page(0).unwrap().scene(&Approx).unwrap();
    let p = paths(&svg::write(&s));
    assert_eq!(p.len(), 2);
    assert!(p[0].starts_with("M48 "), "background first: {p:?}");
    assert!(p[1].starts_with("M336 "), "{p:?}");
}

#[test]
fn several_pages_are_separate_scenes() {
    let b = Builder::new()
        .page(6.0, 4.0, &rect_shape(1, "", (2.0, 1.0), (2.0, 1.0)))
        .page(3.0, 2.0, &rect_shape(1, "", (1.0, 1.0), (1.0, 1.0)));
    let bytes = b.build();
    let v = open(&bytes);
    assert_eq!(v.page_count(), 2);
    let (a, c) = (scene(&bytes, 0), scene(&bytes, 1));
    assert_eq!((a.width, a.height), (576.0, 384.0));
    assert_eq!((c.width, c.height), (288.0, 192.0));
    assert_ne!(summary(&a), summary(&c));
    assert!(v.with_page(2).is_none());
    snap!("page_2", c);
}

#[test]
fn foreign_shapes_draw_a_placeholder_and_warn_once_per_kind() {
    let fg = |id: u32, kind: &str| {
        shape(
            id,
            r#"Type="Foreign""#,
            &format!(
                r#"{}<ForeignData ForeignType="{kind}"/>"#,
                xform(2.0, 1.0, 2.0, 1.0)
            ),
        )
    };
    let bytes = page(&format!(
        "{}{}{}",
        fg(1, "Bitmap"),
        fg(2, "Bitmap"),
        fg(3, "Metafile")
    ))
    .build();
    let s = scene(&bytes, 0);
    assert_eq!(s.elements.len(), 3);
    let svg = svg::write(&s);
    assert_eq!(nums(&paths(&svg)[0])[..2], [96.0, 336.0]);
    assert_eq!(path_attr(&svg, 0, "stroke").as_deref(), Some("#888888"));
    let p = open(&bytes).problems();
    assert_eq!(p.len(), 2, "{p:?}");
    assert!(p[0].message.contains("Bitmap"));
    assert!(p[1].message.contains("Metafile"));
    snap!("foreign", s);
}

#[test]
fn unsupported_shape_types_are_skipped_with_one_warning() {
    let guides = format!(
        "{}{}",
        shape(1, r#"Type="Guide""#, ""),
        shape(2, r#"Type="Guide""#, "")
    );
    let b = page(&guides).build();
    assert!(scene(&b, 0).elements.is_empty());
    assert_eq!(open(&b).problems().len(), 1);
}

#[test]
fn unusable_transforms_skip_the_shape() {
    let bad = rect_shape(1, &cell("Angle", "1e9"), (1.0, 1.0), (1.0, 1.0));
    let huge = rect_shape(2, &cell("PinX", 1e9), (1.0, 1.0), (1.0, 1.0));
    let ok = rect_shape(3, "", (2.0, 1.0), (2.0, 1.0));
    let b = page(&format!("{bad}{huge}{ok}")).build();
    assert_eq!(scene(&b, 0).elements.len(), 1);
    assert_eq!(open(&b).problems().len(), 2);
}

#[test]
fn a_page_without_a_size_cannot_be_drawn() {
    let b = Builder::new()
        .page_with("", "", &rect_shape(1, "", (1.0, 1.0), (1.0, 1.0)))
        .build();
    let v = open(&b);
    let e = v.with_page(0).unwrap().scene(&Approx).unwrap_err();
    assert!(e.report().contains("PageWidth"), "{}", e.report());
    assert_eq!(v.problems().len(), 1);
    let huge = Builder::new().page(1e6, 1.0, "").build();
    assert!(open(&huge).with_page(0).unwrap().scene(&Approx).is_err());
}

// ---- errors ----

fn read_err(bytes: &[u8]) -> String {
    visio::read(bytes).unwrap_err().report()
}

#[test]
fn error_cases() {
    assert!(read_err(b"not a zip").contains("ZIP"));
    assert!(read_err(b"").contains("ZIP"));
    let good = Builder::new().page(1.0, 1.0, "");
    assert!(read_err(&good.build_with(|n| n != "visio/document.xml")).contains("document.xml"));
    assert!(read_err(&good.build_with(|n| n != "visio/pages/pages.xml")).contains("pages.xml"));
    assert!(read_err(&good.build_with(|n| n != "_rels/.rels")).contains("document relationship"));
    // Relationships that name nothing usable, or only parts outside the package.
    let bad_rels = |content: &str| {
        let entries: Vec<Entry> = good
            .parts()
            .into_iter()
            .map(|(name, data)| Entry {
                data: if name == "_rels/.rels" {
                    content.as_bytes().to_vec()
                } else {
                    data
                },
                name,
                deflate: false,
            })
            .collect();
        zip(&entries)
    };
    assert!(read_err(&bad_rels("<Relationships")).contains(".rels"));
    assert!(read_err(&bad_rels(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r" Type="x/document" Target="../../etc/passwd"/></Relationships>"#)).contains("document relationship"));
    assert!(read_err(&bad_rels(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r" Type="x/document" Target="http://evil/" TargetMode="External"/></Relationships>"#)).contains("document relationship"));
}

#[test]
fn a_zip_bomb_part_is_refused() {
    let mut entries: Vec<Entry> = Builder::new()
        .page(1.0, 1.0, "")
        .parts()
        .into_iter()
        .map(|(name, data)| Entry {
            name,
            data,
            deflate: false,
        })
        .collect();
    entries.push(Entry {
        name: "visio/media/bomb.bin".into(),
        data: vec![0; 16 * 1024 * 1024],
        deflate: true,
    });
    assert!(read_err(&zip(&entries)).contains("expands more than"));
}

#[test]
fn traversal_names_are_refused() {
    let mut entries: Vec<Entry> = Builder::new()
        .page(1.0, 1.0, "")
        .parts()
        .into_iter()
        .map(|(name, data)| Entry {
            name,
            data,
            deflate: false,
        })
        .collect();
    entries.push(stored("../evil.xml", b"<a/>"));
    assert!(read_err(&zip(&entries)).contains("unsafe entry name"));
}

#[test]
fn doctypes_are_refused_in_pages() {
    let entries: Vec<Entry> = Builder::new()
        .page(1.0, 1.0, "")
        .parts()
        .into_iter()
        .map(|(name, data)| Entry {
            data: if name == "visio/pages/page1.xml" {
                b"<!DOCTYPE x [<!ENTITY a \"aaaa\"><!ENTITY b \"&a;&a;&a;\">]><PageContents>&b;</PageContents>".to_vec()
            } else {
                data
            },
            name,
            deflate: false,
        })
        .collect();
    let e = read_err(&zip(&entries));
    assert!(e.contains("DOCTYPE") && e.contains("page1.xml"), "{e}");
}

#[test]
fn too_many_shapes_and_deep_nesting_are_errors() {
    let many = "<Shape ID=\"1\"/>".repeat(50_001);
    assert!(read_err(&page(&many).build()).contains("shapes"));
    let mut deep = String::new();
    for _ in 0..70 {
        deep.push_str(r#"<Shape ID="1" Type="Group"><Shapes>"#);
    }
    for _ in 0..70 {
        deep.push_str("</Shapes></Shape>");
    }
    assert!(read_err(&page(&deep).build()).contains("nest deeper"));
}

#[test]
fn a_shape_with_too_many_rows_is_an_error() {
    let rows: Vec<String> = (0..4100)
        .map(|i| row("LineTo", i, &[("X", 1.0), ("Y", 1.0)]))
        .collect();
    let sh = shape(1, "", &geometry(0, "", &rows));
    assert!(read_err(&page(&sh).build()).contains("geometry rows"));
}

// ---- diagram integration (VISIO-12) ----

mod lowered {
    use super::*;
    use sirene_core::diagram::{self, Kind, Lang};
    use sirene_core::scene::Theme;

    fn rect() -> String {
        rect_shape(1, "", (1.0, 1.0), (2.0, 1.0))
    }

    #[test]
    fn bytes_are_a_visio_drawing_by_lang_or_magic() {
        let bytes = page(&rect()).build();
        for lang in [None, Some(Lang::Visio)] {
            let l = diagram::lower_bytes(&bytes, lang);
            assert_eq!(l.kind, Kind::Visio);
            assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
            assert!(l.svg(&Theme::LIGHT).unwrap().contains("<path"));
        }
        assert_eq!(Kind::Visio.name(), "Visio drawing");
        assert_eq!(Kind::Visio.lang(), Lang::Visio);
        assert_eq!(Lang::of_path("a.VSDX".as_ref()), Some(Lang::Visio));
    }

    #[test]
    fn text_bytes_are_read_as_text() {
        let l = diagram::lower_bytes(b"flowchart LR\nA --> B\n", None);
        assert_eq!(l.kind, Kind::Flowchart);
        let l = diagram::lower_bytes(b"\xff\xfe", None);
        assert_eq!(l.diagnostics.len(), 1);
    }

    #[test]
    fn a_bad_package_is_one_error_and_an_empty_drawing() {
        let l = diagram::lower_bytes(b"PK\x03\x04 not a zip", None);
        assert_eq!(l.kind, Kind::Visio);
        assert_eq!(l.diagnostics.len(), 1);
        assert_eq!(
            l.diagnostics[0].severity,
            sirene_core::diagnostic::Severity::Error
        );
        assert_eq!(l.page_count(), 1);
    }

    #[test]
    fn a_drawing_has_no_text_form() {
        let l = diagram::lower_bytes(&page(&rect()).build(), None);
        let e = l.ascii(&Default::default()).unwrap_err();
        assert_eq!(e.report(), "a Visio drawing has no text form");
        // Source text cannot name a package.
        let l = diagram::lower_as("anything", Some(Lang::Visio));
        assert_eq!(l.diagnostics.len(), 1);
    }

    #[test]
    fn pages_are_selected_with_page() {
        let bytes = page(&rect()).page(3.0, 2.0, &rect()).build();
        let l = diagram::lower_bytes(&bytes, None);
        assert_eq!(l.page_count(), 2);
        let second = l.with_page(1).unwrap();
        assert!(second.svg(&Theme::LIGHT).unwrap().contains("width=\"288\""));
        assert!(l.with_page(2).is_none());
    }
}

// ---- fuzz seeds ----

/// The seed corpus of the `visio` fuzz target (`fuzz/fuzz_targets/visio.rs`) is built here: a
/// missing seed is written, an existing one must equal what is built, so the files cannot drift
/// from the writer. Every seed reads without error.
#[test]
fn fuzz_seeds_are_current_and_read() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/visio");
    std::fs::create_dir_all(&dir).unwrap();
    let rect = rect_shape(1, "", (1.0, 1.0), (2.0, 1.0));
    let mut deflated = page(&rect).page(3.0, 2.0, &rect);
    deflated.deflate = true;
    let seeds = [
        ("one-page", page(&rect).build()),
        ("two-pages-deflated", deflated.build()),
        ("empty", Vec::new()),
        ("truncated", page(&rect).build()[..40].to_vec()),
    ];
    for (name, bytes) in seeds {
        let path = dir.join(name);
        match std::fs::read(&path) {
            Ok(have) => assert_eq!(have, bytes, "{name} is stale; delete it to rewrite"),
            Err(_) => std::fs::write(&path, &bytes).unwrap(),
        }
        let ok = !bytes.is_empty() && name != "truncated";
        assert_eq!(visio::read(&bytes).is_ok(), ok, "{name}");
    }
}

// ---- property test ----

mod props {
    use super::*;
    use proptest::prelude::*;

    fn value() -> impl Strategy<Value = String> {
        prop_oneof![
            (-5.0f64..10.0).prop_map(|v| v.to_string()),
            Just("0".to_owned()),
            Just("NaN".to_owned()),
            Just("1e300".to_owned()),
            Just("-1e300".to_owned()),
            Just("#ff00aa".to_owned()),
            Just("junk".to_owned()),
            Just(String::new()),
        ]
    }

    fn cells() -> impl Strategy<Value = String> {
        let names = prop::sample::select(vec![
            "PinX",
            "PinY",
            "Width",
            "Height",
            "LocPinX",
            "LocPinY",
            "Angle",
            "FlipX",
            "FlipY",
            "LineWeight",
            "LineColor",
            "LinePattern",
            "FillForegnd",
            "FillPattern",
            "BeginArrow",
            "EndArrow",
            "TxtWidth",
            "TxtPinX",
            "LayerMember",
        ]);
        prop::collection::vec((names, value()), 0..12).prop_map(|cs| {
            cs.into_iter()
                .map(|(n, v)| format!(r#"<Cell N="{n}" V="{v}"/>"#))
                .collect()
        })
    }

    fn rows() -> impl Strategy<Value = String> {
        let kinds = prop::sample::select(vec![
            "MoveTo",
            "LineTo",
            "ArcTo",
            "EllipticalArcTo",
            "Ellipse",
            "PolylineTo",
            "SplineStart",
            "SplineKnot",
            "NURBSTo",
            "RelMoveTo",
            "RelLineTo",
            "RelCubBezTo",
            "RelQuadBezTo",
            "InfiniteLine",
            "CubBezTo",
            "QuadBezTo",
            "Bogus",
        ]);
        let cell = (
            prop::sample::select(vec!["X", "Y", "A", "B", "C", "D", "E"]),
            value(),
        );
        prop::collection::vec((kinds, prop::collection::vec(cell, 0..7)), 0..14).prop_map(|rs| {
            let rows: String = rs
                .into_iter()
                .enumerate()
                .map(|(i, (t, cs))| {
                    let cells: String = cs
                        .into_iter()
                        .map(|(n, v)| {
                            format!(r#"<Cell N="{n}" V="{v}" F="POLYLINE(1, 1, {v}, 1)"/>"#)
                        })
                        .collect();
                    format!(r#"<Row T="{t}" IX="{i}">{cells}</Row>"#)
                })
                .collect();
            format!(r#"<Section N="Geometry" IX="0">{rows}</Section>"#)
        })
    }

    fn tree(depth: u32) -> BoxedStrategy<String> {
        let leaf = (
            cells(),
            rows(),
            prop::bool::ANY,
            prop::sample::select(vec!["Shape", "Group", "Foreign", "Odd"]),
        )
            .prop_map(|(c, r, text, ty)| {
                let t = if text {
                    r#"<Text>some <cp IX="1"/>text</Text>"#
                } else {
                    ""
                };
                format!(r#"<Shape ID="1" Type="{ty}" Master="1">{c}{r}{t}</Shape>"#)
            });
        if depth == 0 {
            return leaf.boxed();
        }
        (cells(), prop::collection::vec(tree(depth - 1), 0..4))
            .prop_map(|(c, kids)| {
                format!(
                    r#"<Shape ID="2" Type="Group">{c}<Shapes>{}</Shapes></Shape>"#,
                    kids.concat()
                )
            })
            .boxed()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// Random shape trees never panic: reading and drawing return a result, and a drawn
        /// page stays within the element bound its shape count implies.
        #[test]
        fn random_shape_trees_never_panic(shapes in prop::collection::vec(tree(3), 0..4)) {
            let bytes = page(&shapes.concat()).master(1, &shapes.concat()).build();
            if let Ok((v, _)) = visio::read(&bytes)
                && let Some(p) = v.with_page(0)
            {
                {
                    if let Ok(s) = p.scene(&Approx) {
                        prop_assert!(s.elements.len() < 200_000);
                        prop_assert!(s.width <= 200.0 * 96.0 && s.height <= 200.0 * 96.0);
                    }
                    let _ = p.problems();
                }
            }
        }

        /// Arbitrary bytes are an error or a document, never a panic.
        #[test]
        fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..400)) {
            let _ = visio::read(&bytes);
            let mut with_magic = b"PK\x03\x04".to_vec();
            with_magic.extend(bytes);
            let _ = visio::read(&with_magic);
        }

        /// Corrupting one byte of a valid package is an error or a drawable document.
        #[test]
        fn one_flipped_byte_never_panics(at in 0usize..4000, to in any::<u8>()) {
            let mut bytes = page(&rect_shape(1, "", (2.0, 1.0), (2.0, 1.0))).build();
            let i = at % bytes.len();
            if let Some(b) = bytes.get_mut(i) { *b = to; }
            if let Ok((v, _)) = visio::read(&bytes)
                && let Some(p) = v.with_page(0)
            {
                let _ = p.scene(&Approx);
            }
        }
    }
}
