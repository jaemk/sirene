use super::*;

fn t(
    pin: (f64, f64),
    size: (f64, f64),
    loc: (f64, f64),
    angle: f64,
    flip: (bool, bool),
) -> Transform {
    Transform {
        pin_x: pin.0,
        pin_y: pin.1,
        w: size.0,
        h: size.1,
        loc_x: loc.0,
        loc_y: loc.1,
        angle,
        flip_x: flip.0,
        flip_y: flip.1,
    }
}

fn close(a: (f64, f64), b: (f64, f64)) {
    assert!(
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9,
        "{a:?} != {b:?}"
    );
}

#[test]
fn the_origin_of_a_shape_is_the_pin_less_the_locpin() {
    let m = Mat::shape(&t((5.0, 3.0), (2.0, 1.0), (1.0, 0.5), 0.0, (false, false)));
    close(m.apply((0.0, 0.0)), (4.0, 2.5));
    close(m.apply((2.0, 1.0)), (6.0, 3.5));
    close(m.apply((1.0, 0.5)), (5.0, 3.0));
}

#[test]
fn rotation_is_counterclockwise_about_the_pin() {
    let m = Mat::shape(&t(
        (5.0, 3.0),
        (2.0, 1.0),
        (1.0, 0.5),
        std::f64::consts::FRAC_PI_2,
        (false, false),
    ));
    // The local point right of the pin by 1 goes up by 1.
    close(m.apply((2.0, 0.5)), (5.0, 4.0));
    // The local origin is (-1, -0.5) from the pin; rotated 90 degrees it is (0.5, -1).
    close(m.apply((0.0, 0.0)), (5.5, 2.0));
}

#[test]
fn flips_mirror_the_shape_before_the_pin_offset() {
    let m = Mat::shape(&t((5.0, 3.0), (2.0, 1.0), (0.0, 0.0), 0.0, (true, false)));
    // x' = 2 - x, then minus a LocPinX of 0.
    close(m.apply((0.0, 0.0)), (7.0, 3.0));
    close(m.apply((2.0, 1.0)), (5.0, 4.0));
    let both = Mat::shape(&t((0.0, 0.0), (2.0, 1.0), (0.0, 0.0), 0.0, (true, true)));
    close(both.apply((0.5, 0.25)), (1.5, 0.75));
}

#[test]
fn a_child_transform_follows_its_parent() {
    let group = Mat::shape(&t(
        (10.0, 10.0),
        (4.0, 4.0),
        (2.0, 2.0),
        0.0,
        (false, false),
    ));
    let child = Mat::shape(&t((1.0, 1.0), (1.0, 1.0), (0.5, 0.5), 0.0, (false, false)));
    let total = group.after(&child);
    // The child's origin is (0.5, 0.5) in the group's frame; the group's origin is (8, 8).
    close(total.apply((0.0, 0.0)), (8.5, 8.5));
}

#[test]
fn colors() {
    assert_eq!(PALETTE.len(), 24);
    assert_eq!(
        rgba([1, 2, 3], 0.0),
        Color {
            r: 1,
            g: 2,
            b: 3,
            a: 255
        }
    );
    assert_eq!(rgba([1, 2, 3], 1.0).a, 0);
    assert_eq!(rgba([1, 2, 3], 0.5).a, 128);
}
