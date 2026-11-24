//! The SVG paths KaTeX builds from parameters: radical signs, stretchy delimiter parts, and
//! the phase angle.
//!
//! Ported from KaTeX's `src/svgGeometry.ts` (v0.19.1), MIT licensed, Copyright (c) 2013-2020 Khan
//! Academy and other contributors; see THIRD-PARTY-NOTICES. The fixed paths are in `svg_data`.
//! Output matches KaTeX character for character, including how JavaScript prints numbers, so a
//! test compares each builder against KaTeX's own output.

/// Padding above a radical's vinculum, in view box units. Prevents cropping.
pub(crate) const H_LINE_PAD: f64 = 80.0;

/// Prints a number the way a JavaScript template literal does: `80` not `80.0`, `0.5` not `.5`,
/// no exponent for magnitudes in `1e-6..1e21`, and `0` for negative zero.
pub(crate) fn js_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    // Shortest round-trip digits and exponent, as `d.ddde[-]x`.
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    // The decimal point sits after `n` digits.
    let n = exp.parse::<i32>().unwrap_or(0) + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        let (int, frac) = digits.split_at(n as usize);
        format!("{int}.{frac}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        let (first, rest) = digits.split_at(1);
        let point = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        format!("{first}{point}e{sign}{}", e.abs())
    };
    if x < 0.0 { format!("-{body}") } else { body }
}

fn n(x: f64) -> String {
    js_number(x)
}

/// KaTeX's `doubleBrushStroke`: the path twice, so the thin shaft is drawn with sharper edges.
fn double_brush_stroke(path: &str) -> String {
    format!("{path} {path}")
}

/// The radical sign for a `\sqrt` at its base size (glyph U+221A of KaTeX_Main).
/// `extra_vinculum` is in view box units (1000 per em).
pub(crate) fn sqrt_main(extra_vinculum: f64, h_line_pad: f64) -> String {
    let ev = n(extra_vinculum);
    let top = n(622.0 + extra_vinculum + h_line_pad);
    let slant = n(extra_vinculum / 2.075);
    let thick = n(40.0 + extra_vinculum);
    let bar = n(834.0 + extra_vinculum);
    let pad = n(h_line_pad);
    format!(
        "M95,{top}\n\
c-2.7,0,-7.17,-2.7,-13.5,-8c-5.8,-5.3,-9.5,-10,-9.5,-14\n\
c0,-2,0.3,-3.3,1,-4c1.3,-2.7,23.83,-20.7,67.5,-54\n\
c44.2,-33.3,65.8,-50.3,66.5,-51c1.3,-1.3,3,-2,5,-2c4.7,0,8.7,3.3,12,10\n\
s173,378,173,378c0.7,0,35.3,-71,104,-213c68.7,-142,137.5,-285,206.5,-429\n\
c69,-144,104.5,-217.7,106.5,-221\n\
l{slant} -{ev}\n\
c5.3,-9.3,12,-14,20,-14\n\
H400000v{thick}H845.2724\n\
s-225.272,467,-225.272,467s-235,486,-235,486c-2.7,4.7,-9,7,-19,7\n\
c-6,0,-10,-1,-12,-3s-194,-422,-194,-422s-65,47,-65,47z\n\
M{bar} {pad}h400000v{thick}h-400000z"
    )
}

/// The radical sign from KaTeX_Size1-Regular.
pub(crate) fn sqrt_size1(extra_vinculum: f64, h_line_pad: f64) -> String {
    let ev = n(extra_vinculum);
    let top = n(601.0 + extra_vinculum + h_line_pad);
    let slant = n(extra_vinculum / 2.084);
    let thick = n(40.0 + extra_vinculum);
    let bar = n(1001.0 + extra_vinculum);
    let pad = n(h_line_pad);
    format!(
        "M263,{top}c0.7,0,18,39.7,52,119\n\
c34,79.3,68.167,158.7,102.5,238c34.3,79.3,51.8,119.3,52.5,120\n\
c340,-704.7,510.7,-1060.3,512,-1067\n\
l{slant} -{ev}\n\
c4.7,-7.3,11,-11,19,-11\n\
H40000v{thick}H1012.3\n\
s-271.3,567,-271.3,567c-38.7,80.7,-84,175,-136,283c-52,108,-89.167,185.3,-111.5,232\n\
c-22.3,46.7,-33.8,70.3,-34.5,71c-4.7,4.7,-12.3,7,-23,7s-12,-1,-12,-1\n\
s-109,-253,-109,-253c-72.7,-168,-109.3,-252,-110,-252c-10.7,8,-22,16.7,-34,26\n\
c-22,17.3,-33.3,26,-34,26s-26,-26,-26,-26s76,-59,76,-59s76,-60,76,-60z\n\
M{bar} {pad}h400000v{thick}h-400000z"
    )
}

/// The radical sign from KaTeX_Size2-Regular.
pub(crate) fn sqrt_size2(extra_vinculum: f64, h_line_pad: f64) -> String {
    let ev = n(extra_vinculum);
    let top = n(10.0 + extra_vinculum + h_line_pad);
    let slant = n(extra_vinculum / 3.13);
    let thick = n(40.0 + extra_vinculum);
    let bar = n(1001.0 + extra_vinculum);
    let pad = n(h_line_pad);
    format!(
        "M983 {top}\n\
l{slant} -{ev}\n\
c4,-6.7,10,-10,18,-10 H400000v{thick}\n\
H1013.1s-83.4,268,-264.1,840c-180.7,572,-277,876.3,-289,913c-4.7,4.7,-12.7,7,-24,7\n\
s-12,0,-12,0c-1.3,-3.3,-3.7,-11.7,-7,-25c-35.3,-125.3,-106.7,-373.3,-214,-744\n\
c-10,12,-21,25,-33,39s-32,39,-32,39c-6,-5.3,-15,-14,-27,-26s25,-30,25,-30\n\
c26.7,-32.7,52,-63,76,-91s52,-60,52,-60s208,722,208,722\n\
c56,-175.3,126.3,-397.3,211,-666c84.7,-268.7,153.8,-488.2,207.5,-658.5\n\
c53.7,-170.3,84.5,-266.8,92.5,-289.5z\n\
M{bar} {pad}h400000v{thick}h-400000z"
    )
}

/// The radical sign from KaTeX_Size3-Regular.
pub(crate) fn sqrt_size3(extra_vinculum: f64, h_line_pad: f64) -> String {
    let ev = n(extra_vinculum);
    let top = n(2398.0 + extra_vinculum + h_line_pad);
    let slant = n(extra_vinculum / 4.223);
    let thick = n(40.0 + extra_vinculum);
    let bar = n(1001.0 + extra_vinculum);
    let pad = n(h_line_pad);
    format!(
        "M424,{top}\n\
c-1.3,-0.7,-38.5,-172,-111.5,-514c-73,-342,-109.8,-513.3,-110.5,-514\n\
c0,-2,-10.7,14.3,-32,49c-4.7,7.3,-9.8,15.7,-15.5,25c-5.7,9.3,-9.8,16,-12.5,20\n\
s-5,7,-5,7c-4,-3.3,-8.3,-7.7,-13,-13s-13,-13,-13,-13s76,-122,76,-122s77,-121,77,-121\n\
s209,968,209,968c0,-2,84.7,-361.7,254,-1079c169.3,-717.3,254.7,-1077.7,256,-1081\n\
l{slant} -{ev}c4,-6.7,10,-10,18,-10 H400000\n\
v{thick}H1014.6\n\
s-87.3,378.7,-272.6,1166c-185.3,787.3,-279.3,1182.3,-282,1185\n\
c-2,6,-10,9,-24,9\n\
c-8,0,-12,-0.7,-12,-2z M{bar} {pad}\n\
h400000v{thick}h-400000z"
    )
}

/// The radical sign from KaTeX_Size4-Regular.
pub(crate) fn sqrt_size4(extra_vinculum: f64, h_line_pad: f64) -> String {
    let ev = n(extra_vinculum);
    let top = n(2713.0 + extra_vinculum + h_line_pad);
    let slant = n(extra_vinculum / 5.298);
    let thick = n(40.0 + extra_vinculum);
    let bar = n(1001.0 + extra_vinculum);
    let pad = n(h_line_pad);
    format!(
        "M473,{top}\n\
c339.3,-1799.3,509.3,-2700,510,-2702 l{slant} -{ev}\n\
c3.3,-7.3,9.3,-11,18,-11 H400000v{thick}H1017.7\n\
s-90.5,478,-276.2,1466c-185.7,988,-279.5,1483,-281.5,1485c-2,6,-10,9,-24,9\n\
c-8,0,-12,-0.7,-12,-2c0,-1.3,-5.3,-32,-16,-92c-50.7,-293.3,-119.7,-693.3,-207,-1200\n\
c0,-1.3,-5.3,8.7,-16,30c-10.7,21.3,-21.3,42.7,-32,64s-16,33,-16,33s-26,-26,-26,-26\n\
s76,-153,76,-153s77,-151,77,-151c0.7,0.7,35.7,202,105,604c67.3,400.7,102,602.7,104,\n\
606zM{bar} {pad}h400000v{thick}H1017.7z"
    )
}

/// The radical sign for a tall `\sqrt` (glyph U+23B7 of KaTeX_Size4-Regular), whose left edge
/// stretches to `view_box_height`.
pub(crate) fn sqrt_tall(extra_vinculum: f64, h_line_pad: f64, view_box_height: f64) -> String {
    // The vertical edge runs from the vinculum to near the bottom of the surd.
    let vert = n(view_box_height - 54.0 - h_line_pad - extra_vinculum);
    let top = n(extra_vinculum + h_line_pad);
    let thick = n(40.0 + extra_vinculum);
    let pad = n(h_line_pad);
    // KaTeX writes `H400000` and the thickness with no space between, and so does this.
    format!(
        "M702 {top}H400000{thick}\n\
H742v{vert}l-4 4-4 4c-.667.7 -2 1.5-4 2.5s-4.167 1.833-6.5 2.5-5.5 1-9.5 1\n\
h-12l-28-84c-16.667-52-96.667 -294.333-240-727l-212 -643 -85 170\n\
c-4-3.333-8.333-7.667-13 -13l-13-13l77-155 77-156c66 199.333 139 419.667\n\
219 661 l218 661zM702 {pad}H400000v{thick}H742z"
    )
}

/// A radical sign variant, named as in KaTeX's `sqrtPath`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SqrtSize {
    Main,
    Size1,
    Size2,
    Size3,
    Size4,
    Tall,
}

/// The radical path of `size`. `extra_vinculum` is in ems and `view_box_height` in view box
/// units; the height matters only for [`SqrtSize::Tall`].
pub(crate) fn sqrt_path(size: SqrtSize, extra_vinculum: f64, view_box_height: f64) -> String {
    // From document ems to view box units.
    let ev = 1000.0 * extra_vinculum;
    match size {
        SqrtSize::Main => sqrt_main(ev, H_LINE_PAD),
        SqrtSize::Size1 => sqrt_size1(ev, H_LINE_PAD),
        SqrtSize::Size2 => sqrt_size2(ev, H_LINE_PAD),
        SqrtSize::Size3 => sqrt_size3(ev, H_LINE_PAD),
        SqrtSize::Size4 => sqrt_size4(ev, H_LINE_PAD),
        SqrtSize::Tall => sqrt_tall(ev, H_LINE_PAD, view_box_height),
    }
}

/// The path of the phase angle (`\phase`) for a view box `y` units tall.
pub(crate) fn phase_path(y: f64) -> String {
    // The x coordinate at the top of the angle.
    let x = y / 2.0;
    format!(
        "M400000 {} H0 L{} 0 l65 45 L145 {} H400000z",
        n(y),
        n(x),
        n(y - 80.0)
    )
}

/// The straight middle part of a stretchy delimiter, `height` view box units tall. Empty for a
/// delimiter with no such part.
pub(crate) fn inner_path(name: &str, height: f64) -> String {
    let h = n(height);
    let bar = |x0: u32, x1: u32| double_brush_stroke(&format!("M{x0} 0 H{x1} V{h} H{x0}z"));
    match name {
        "\u{239c}" => bar(291, 417),
        "\u{2223}" => bar(145, 188),
        "\u{2225}" => bar(145, 188) + &bar(367, 410),
        "\u{239f}" => bar(457, 583),
        "\u{23a2}" => bar(319, 403),
        "\u{23a5}" => bar(263, 347),
        "\u{23aa}" => bar(384, 504),
        "\u{23d0}" => bar(312, 355),
        "\u{2016}" => bar(257, 300) + &bar(478, 521),
        _ => String::new(),
    }
}

/// The path of a stretchy tall delimiter (`lbrack`, `rbrack`, `vert`, `doublevert`, `lfloor`,
/// `rfloor`, `lceil`, `rceil`, `lparen`, `rparen`) with a middle section `mid_height` tall.
/// `None` for another label, where KaTeX throws "Unknown stretchy delimiter."
pub(crate) fn tall_delim(label: &str, mid_height: f64) -> Option<String> {
    let m = n(mid_height);
    let neg = n(-mid_height);
    let path = match label {
        "lbrack" => format!(
            "M403 1759 V84 H666 V0 H319 V1759 v{m} v1759 v84 h347 v-84\n\
H403z M403 1759 V0 H319 V1759 v{m} v1759 v84 h84z"
        ),
        "rbrack" => format!(
            "M347 1759 V0 H0 V84 H263 V1759 v{m} v1759 H0 v84 H347z\n\
M347 1759 V0 H263 V1759 v{m} v1759 h84z"
        ),
        "vert" => format!(
            "M145 15 v585 v{m} v585 c2.667,10,9.667,15,21,15\n\
c10,0,16.667,-5,20,-15 v-585 v{neg} v-585 c-2.667,-10,-9.667,-15,-21,-15\n\
c-10,0,-16.667,5,-20,15z M188 15 H145 v585 v{m} v585 h43z"
        ),
        "doublevert" => format!(
            "M145 15 v585 v{m} v585 c2.667,10,9.667,15,21,15\n\
c10,0,16.667,-5,20,-15 v-585 v{neg} v-585 c-2.667,-10,-9.667,-15,-21,-15\n\
c-10,0,-16.667,5,-20,15z M188 15 H145 v585 v{m} v585 h43z\n\
M367 15 v585 v{m} v585 c2.667,10,9.667,15,21,15\n\
c10,0,16.667,-5,20,-15 v-585 v{neg} v-585 c-2.667,-10,-9.667,-15,-21,-15\n\
c-10,0,-16.667,5,-20,15z M410 15 H367 v585 v{m} v585 h43z"
        ),
        // The doubled `M` in the two floors is in KaTeX's source.
        "lfloor" => format!(
            "M319 602 V0 H403 V602 v{m} v1715 h263 v84 H319z\n\
MM319 602 V0 H403 V602 v{m} v1715 H319z"
        ),
        "rfloor" => format!(
            "M319 602 V0 H403 V602 v{m} v1799 H0 v-84 H319z\n\
MM319 602 V0 H403 V602 v{m} v1715 H319z"
        ),
        "lceil" => format!(
            "M403 1759 V84 H666 V0 H319 V1759 v{m} v602 h84z\n\
M403 1759 V0 H319 V1759 v{m} v602 h84z"
        ),
        "rceil" => format!(
            "M347 1759 V0 H0 V84 H263 V1759 v{m} v602 h84z\n\
M347 1759 V0 h-84 V1759 v{m} v602 h84z"
        ),
        "lparen" => {
            let a = n(mid_height + 84.0);
            let b = n(mid_height + 92.0);
            format!(
                "M863,9c0,-2,-2,-5,-6,-9c0,0,-17,0,-17,0c-12.7,0,-19.3,0.3,-20,1\n\
c-5.3,5.3,-10.3,11,-15,17c-242.7,294.7,-395.3,682,-458,1162c-21.3,163.3,-33.3,349,\n\
-36,557 l0,{a}c0.2,6,0,26,0,60c2,159.3,10,310.7,24,454c53.3,528,210,\n\
949.7,470,1265c4.7,6,9.7,11.7,15,17c0.7,0.7,7,1,19,1c0,0,18,0,18,0c4,-4,6,-7,6,-9\n\
c0,-2.7,-3.3,-8.7,-10,-18c-135.3,-192.7,-235.5,-414.3,-300.5,-665c-65,-250.7,-102.5,\n\
-544.7,-112.5,-882c-2,-104,-3,-167,-3,-189\n\
l0,-{b}c0,-162.7,5.7,-314,17,-454c20.7,-272,63.7,-513,129,-723c65.3,\n\
-210,155.3,-396.3,270,-559c6.7,-9.3,10,-15.3,10,-18z"
            )
        }
        "rparen" => {
            let a = n(mid_height + 9.0);
            let b = n(mid_height + 144.0);
            format!(
                "M76,0c-16.7,0,-25,3,-25,9c0,2,2,6.3,6,13c21.3,28.7,42.3,60.3,\n\
63,95c96.7,156.7,172.8,332.5,228.5,527.5c55.7,195,92.8,416.5,111.5,664.5\n\
c11.3,139.3,17,290.7,17,454c0,28,1.7,43,3.3,45l0,{a}\n\
c-3,4,-3.3,16.7,-3.3,38c0,162,-5.7,313.7,-17,455c-18.7,248,-55.8,469.3,-111.5,664\n\
c-55.7,194.7,-131.8,370.3,-228.5,527c-20.7,34.7,-41.7,66.3,-63,95c-2,3.3,-4,7,-6,11\n\
c0,7.3,5.7,11,17,11c0,0,11,0,11,0c9.3,0,14.3,-0.3,15,-1c5.3,-5.3,10.3,-11,15,-17\n\
c242.7,-294.7,395.3,-681.7,458,-1161c21.3,-164.7,33.3,-350.7,36,-558\n\
l0,-{b}c-2,-159.3,-10,-310.7,-24,-454c-53.3,-528,-210,-949.7,\n\
-470,-1265c-4.7,-6,-9.7,-11.7,-15,-17c-0.7,-0.7,-6.7,-1,-18,-1z"
            )
        }
        _ => return None,
    };
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/math/katex-svg.json"))
            .unwrap_or(Value::Null)
    }

    fn cases(key: &str) -> Vec<Value> {
        let value = fixture();
        let list = value[key].as_array().cloned().unwrap_or_default();
        assert!(!list.is_empty(), "no `{key}` cases in the fixture");
        list
    }

    fn num(v: &Value) -> f64 {
        v.as_f64().expect("number in fixture")
    }

    fn text(v: &Value) -> &str {
        v.as_str().expect("string in fixture")
    }

    #[test]
    fn path_table_has_katex_size() {
        let want = fixture()["count"].as_u64().expect("count in fixture") as usize;
        assert_eq!(crate::math::svg_data::PATHS.len(), want);
    }

    #[test]
    fn path_table_is_sorted_with_unique_names() {
        let table = crate::math::svg_data::PATHS;
        assert!(table.windows(2).all(|w| w[0].0 < w[1].0));
        let names: Vec<&str> = table.iter().map(|(name, _)| *name).collect();
        let binding = fixture();
        let want: Vec<&str> = binding["names"]
            .as_array()
            .expect("names in fixture")
            .iter()
            .map(text)
            .collect();
        assert_eq!(names, want);
    }

    #[test]
    fn path_lookup_finds_katex_data() {
        let spot = fixture()["spot"].clone();
        let spot = spot.as_object().expect("spot in fixture");
        assert!(!spot.is_empty());
        for (name, data) in spot {
            assert_eq!(
                crate::math::svg_data::path(name),
                Some(text(data)),
                "{name}"
            );
        }
        assert_eq!(crate::math::svg_data::path("nonesuch"), None);
        assert_eq!(crate::math::svg_data::path(""), None);
    }

    #[test]
    fn js_number_matches_javascript() {
        let cases: &[(f64, &str)] = &[
            (80.0, "80"),
            (0.5, "0.5"),
            (-0.5, "-0.5"),
            (0.0, "0"),
            (-0.0, "0"),
            (-1234.0, "-1234"),
            (1234.5, "1234.5"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (123456789012345680000.0, "123456789012345680000"),
            (1.5e21, "1.5e+21"),
            (0.000001, "0.000001"),
            (0.0000001, "1e-7"),
            (1.25e-7, "1.25e-7"),
            (0.00001234, "0.00001234"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
        ];
        for (x, want) in cases {
            assert_eq!(js_number(*x), *want, "{x:?}");
        }
    }

    #[test]
    fn sqrt_paths_match_katex() {
        let sizes = [
            ("sqrtMain", SqrtSize::Main),
            ("sqrtSize1", SqrtSize::Size1),
            ("sqrtSize2", SqrtSize::Size2),
            ("sqrtSize3", SqrtSize::Size3),
            ("sqrtSize4", SqrtSize::Size4),
            ("sqrtTall", SqrtSize::Tall),
        ];
        for case in cases("sqrt") {
            let name = text(&case["size"]);
            let size = sizes
                .iter()
                .find(|(n, _)| *n == name)
                .expect("known size")
                .1;
            let got = sqrt_path(size, num(&case["extra"]), num(&case["height"]));
            assert_eq!(got, text(&case["path"]), "{case}");
        }
    }

    #[test]
    fn inner_paths_match_katex() {
        for case in cases("inner") {
            let got = inner_path(text(&case["name"]), num(&case["height"]));
            assert_eq!(got, text(&case["path"]), "{case}");
        }
    }

    #[test]
    fn tall_delims_match_katex() {
        for case in cases("tall") {
            let got = tall_delim(text(&case["label"]), num(&case["mid"]));
            assert_eq!(got.as_deref(), Some(text(&case["path"])), "{case}");
        }
    }

    #[test]
    fn tall_delim_rejects_unknown_label() {
        assert_eq!(tall_delim("langle", 100.0), None);
    }

    #[test]
    fn phase_paths_match_katex() {
        for case in cases("phase") {
            assert_eq!(phase_path(num(&case["y"])), text(&case["path"]), "{case}");
        }
    }
}
