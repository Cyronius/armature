/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Convert SVG bearing commands to ordinary absolute path commands for path consumers that do
//! not understand `B` and `b`. The original `d` attribute stays intact in the DOM.

use std::fmt::Write;

use kurbo::{Arc, Point as KurboPoint, SvgArc, Vec2};
use svgtypes::{NumberListParser, PathParser, PathSegment};

#[derive(Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Default)]
struct BearingPath {
    output: String,
    current: Point,
    subpath_start: Point,
    bearing: f64,
    last_tangent: Option<f64>,
    last_quadratic_control: Option<Point>,
    has_moveto: bool,
}

impl BearingPath {
    fn relative_point(&self, x: f64, y: f64) -> Point {
        let (sin, cos) = self.bearing.to_radians().sin_cos();
        let sin = if sin.abs() < 1e-12 { 0.0 } else { sin };
        let cos = if cos.abs() < 1e-12 { 0.0 } else { cos };
        Point::new(
            self.current.x + x * cos + y * sin,
            self.current.y + x * sin + y * cos,
        )
    }

    fn point(&self, absolute: bool, x: f64, y: f64) -> Point {
        if absolute {
            Point::new(x, y)
        } else {
            self.relative_point(x, y)
        }
    }

    fn tangent(&mut self, from: Point, to: Point) {
        if from.x != to.x || from.y != to.y {
            self.last_tangent = Some((to.y - from.y).atan2(to.x - from.x).to_degrees());
        }
    }

    fn append(&mut self, segment: PathSegment) -> Option<()> {
        use PathSegment::*;
        if !self.has_moveto && !matches!(segment, MoveTo { .. }) {
            return None;
        }
        let start = self.current;
        match segment {
            MoveTo { abs, x, y } => {
                self.current = self.point(abs, x, y);
                self.subpath_start = self.current;
                self.has_moveto = true;
                self.last_quadratic_control = None;
                write!(self.output, "M {} {} ", self.current.x, self.current.y).ok()?;
            },
            LineTo { abs, x, y } => {
                self.current = self.point(abs, x, y);
                self.tangent(start, self.current);
                self.last_quadratic_control = None;
                write!(self.output, "L {} {} ", self.current.x, self.current.y).ok()?;
            },
            HorizontalLineTo { abs, x } => {
                self.current = if abs {
                    Point::new(x, start.y)
                } else {
                    self.relative_point(x, 0.0)
                };
                self.tangent(start, self.current);
                self.last_quadratic_control = None;
                write!(self.output, "L {} {} ", self.current.x, self.current.y).ok()?;
            },
            VerticalLineTo { abs, y } => {
                self.current = if abs {
                    Point::new(start.x, y)
                } else {
                    self.relative_point(0.0, y)
                };
                self.tangent(start, self.current);
                self.last_quadratic_control = None;
                write!(self.output, "L {} {} ", self.current.x, self.current.y).ok()?;
            },
            CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let control1 = self.point(abs, x1, y1);
                let control2 = self.point(abs, x2, y2);
                self.current = self.point(abs, x, y);
                self.tangent(control2, self.current);
                self.last_quadratic_control = None;
                write!(
                    self.output,
                    "C {} {} {} {} {} {} ",
                    control1.x, control1.y, control2.x, control2.y, self.current.x, self.current.y
                )
                .ok()?;
            },
            SmoothCurveTo { abs, x2, y2, x, y } => {
                let control2 = self.point(abs, x2, y2);
                self.current = self.point(abs, x, y);
                self.tangent(control2, self.current);
                self.last_quadratic_control = None;
                write!(
                    self.output,
                    "S {} {} {} {} ",
                    control2.x, control2.y, self.current.x, self.current.y
                )
                .ok()?;
            },
            Quadratic { abs, x1, y1, x, y } => {
                let control = self.point(abs, x1, y1);
                self.current = self.point(abs, x, y);
                self.tangent(control, self.current);
                self.last_quadratic_control = Some(control);
                write!(
                    self.output,
                    "Q {} {} {} {} ",
                    control.x, control.y, self.current.x, self.current.y
                )
                .ok()?;
            },
            SmoothQuadratic { abs, x, y } => {
                let control = self.last_quadratic_control.map_or(start, |previous| {
                    Point::new(2.0 * start.x - previous.x, 2.0 * start.y - previous.y)
                });
                self.current = self.point(abs, x, y);
                self.tangent(control, self.current);
                self.last_quadratic_control = Some(control);
                write!(self.output, "T {} {} ", self.current.x, self.current.y).ok()?;
            },
            EllipticalArc {
                abs,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                self.current = self.point(abs, x, y);
                self.last_quadratic_control = None;
                let rotation = x_axis_rotation + if abs { 0.0 } else { self.bearing };
                let svg_arc = SvgArc {
                    from: KurboPoint::new(start.x, start.y),
                    to: KurboPoint::new(self.current.x, self.current.y),
                    radii: Vec2::new(rx, ry),
                    x_rotation: rotation.to_radians(),
                    large_arc,
                    sweep,
                };
                if let Some(arc) = Arc::from_svg_arc(&svg_arc) {
                    let angle = arc.start_angle + arc.sweep_angle;
                    let (sin_angle, cos_angle) = angle.sin_cos();
                    let (sin_rotation, cos_rotation) = arc.x_rotation.sin_cos();
                    let direction = arc.sweep_angle.signum();
                    let dx = direction *
                        (-arc.radii.x * sin_angle * cos_rotation -
                            arc.radii.y * cos_angle * sin_rotation);
                    let dy = direction *
                        (-arc.radii.x * sin_angle * sin_rotation +
                            arc.radii.y * cos_angle * cos_rotation);
                    if dx != 0.0 || dy != 0.0 {
                        self.last_tangent = Some(dy.atan2(dx).to_degrees());
                    }
                } else {
                    self.tangent(start, self.current);
                }
                write!(
                    self.output,
                    "A {rx} {ry} {rotation} {} {} {} {} ",
                    u8::from(large_arc),
                    u8::from(sweep),
                    self.current.x,
                    self.current.y
                )
                .ok()?;
            },
            ClosePath { .. } => {
                self.current = self.subpath_start;
                self.tangent(start, self.current);
                self.last_quadratic_control = None;
                self.output.push_str("Z ");
            },
        }
        Some(())
    }

    fn append_chunk(&mut self, chunk: &str, after_bearing: bool) -> Option<()> {
        let source = if after_bearing {
            format!("M {} {} {chunk}", self.current.x, self.current.y)
        } else {
            chunk.to_owned()
        };
        let mut segments = PathParser::from(source.as_str());
        if after_bearing {
            // The parser requires a moveto at the start of each input. This synthetic one must
            // not reset the real subpath or the tangent used by a later relative bearing.
            segments.next()?.ok()?;
        }
        for segment in segments {
            self.append(segment.ok()?)?;
        }
        Some(())
    }
}

fn is_path_command(byte: u8) -> bool {
    matches!(
        byte,
        b'M' | b'm' |
            b'Z' |
            b'z' |
            b'L' |
            b'l' |
            b'H' |
            b'h' |
            b'V' |
            b'v' |
            b'C' |
            b'c' |
            b'S' |
            b's' |
            b'Q' |
            b'q' |
            b'T' |
            b't' |
            b'A' |
            b'a' |
            b'B' |
            b'b'
    )
}

/// Return path data without bearing commands, or `None` when there is no bearing command or the
/// path cannot be parsed. A relative bearing uses the tangent of the preceding drawn segment.
pub(crate) fn expand_bearing(data: &str) -> Option<String> {
    let bytes = data.as_bytes();
    if !bytes.iter().any(|byte| matches!(*byte, b'B' | b'b')) {
        return None;
    }

    let mut path = BearingPath::default();
    let mut cursor = 0;
    let mut after_bearing = false;
    while let Some(offset) = bytes[cursor..]
        .iter()
        .position(|byte| matches!(*byte, b'B' | b'b'))
    {
        let bearing_at = cursor + offset;
        path.append_chunk(&data[cursor..bearing_at], after_bearing)?;
        if !path.has_moveto {
            return None;
        }
        let command = bytes[bearing_at];
        let angle_start = bearing_at + 1;
        let angle_end = bytes[angle_start..]
            .iter()
            .position(|byte| is_path_command(*byte))
            .map_or(bytes.len(), |offset| angle_start + offset);
        let angles: Vec<f64> = NumberListParser::from(&data[angle_start..angle_end])
            .collect::<Result<_, _>>()
            .ok()?;
        let angle = *angles.last()?;
        path.bearing = if command == b'B' {
            angle
        } else {
            path.last_tangent.unwrap_or(0.0) + angle
        };
        cursor = angle_end;
        after_bearing = true;
    }
    path.append_chunk(&data[cursor..], after_bearing)?;
    Some(path.output.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::expand_bearing;

    #[test]
    fn absolute_and_relative_bearings_draw_the_same_square() {
        let absolute = expand_bearing("M 20 150 B -90 h 120 B 0 h 140 B 90 h 120 z").unwrap();
        let relative = expand_bearing("M 20 150 b -90 h 120 b 90 h 140 b 90 h 120 z").unwrap();
        for path in [absolute, relative] {
            assert!(path.starts_with("M 20 150 L 20 30 L 160 30 L 160 150 Z"));
        }
    }

    #[test]
    fn zero_bearing_preserves_relative_moves() {
        let path = expand_bearing("M 25 50 h 10 B 0 m 10 0 h 10 b 0 m 10 0 h 10").unwrap();
        assert_eq!(path, "M 25 50 L 35 50 M 45 50 L 55 50 M 65 50 L 75 50");
    }

    #[test]
    fn relative_bearing_uses_elliptical_arc_end_tangent() {
        let path = expand_bearing("M 0 0 A 10 10 0 0 1 10 10 b 0 h 10").unwrap();
        assert!(path.ends_with("L 10 20"));
    }
}
