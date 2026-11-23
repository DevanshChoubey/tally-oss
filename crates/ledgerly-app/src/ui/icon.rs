//! Icons: the mock's stroke icons (`ICONS` in `design/mock-v2/index.html`, drawn by `.ico`: a
//! 24-unit view box shown at 16 points, stroke 1.75, round caps and joins, no fill).
//!
//! Each icon is its SVG path data, copied from the mock. [`parse`] turns path data into polylines
//! (arcs and curves are flattened), and [`paint`] strokes them into a square. The parser reads the
//! commands the mock uses: `M L H V C A Z` and their relative forms.

use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2};

/// Side of an icon, in points (`.ico` in the mock).
pub const ICON_SIZE: f32 = 16.0;
/// Stroke width in view-box units (`stroke-width:1.75` on a 24-unit box).
const STROKE_UNITS: f32 = 1.75;
/// The view box side the path data is written in.
const VIEW_BOX: f32 = 24.0;
/// Segments per flattened curve or arc.
const CURVE_STEPS: usize = 12;

/// The icons the app draws, each named as the mock names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// `home`.
    Home,
    /// `inbox` (review queue).
    Inbox,
    /// `sale`.
    Sale,
    /// `purchase`.
    Purchase,
    /// `pay`.
    Pay,
    /// `recv`.
    Recv,
    /// `chat` (plain-words entry).
    Chat,
    /// `book` (day book).
    Book,
    /// `scale` (balance sheet).
    Scale,
    /// `users` (masters).
    Users,
    /// `gst`.
    Gst,
    /// `bank`.
    Bank,
    /// `shield` (AI and privacy).
    Shield,
    /// `kbd` (keyboard map).
    Kbd,
    /// `building` (company).
    Building,
    /// `cal` (date).
    Cal,
    /// `lock`.
    Lock,
    /// `search`.
    Search,
    /// `cpu` (AI on this computer).
    Cpu,
    /// `cloud` (AI in the cloud).
    Cloud,
    /// `off` (AI off).
    Off,
    /// `sun` (light theme).
    Sun,
    /// `contrast` (dark and high-contrast themes).
    Contrast,
    /// `check`.
    Check,
    /// `chev` (breadcrumb separator).
    Chev,
    /// `spark` (AI).
    Spark,
    /// `file` (a voucher).
    File,
    /// `sliders` (a command).
    Sliders,
    /// `plus` (create).
    Plus,
    /// `upload` (export a file, import a file).
    Upload,
    /// `save` (the form's Save button).
    Save,
    /// `x` (a field's error, a close).
    X,
}

impl Icon {
    /// The SVG path data, as the mock's `ICONS` table gives it.
    #[must_use]
    pub fn path(self) -> &'static str {
        match self {
            Icon::Home => "M3 11l9-7 9 7v9a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z",
            Icon::Inbox => "M4 13l2-8h12l2 8 M4 13v6h16v-6 M4 13h5l1 2h4l1-2h5",
            Icon::Sale => "M6 3h9l4 4v14H6z M14 3v5h5 M9 13h7 M9 17h5",
            Icon::Purchase => {
                "M5 4h14v16l-2.5-1.5L14 20l-2-1.5L10 20l-2.5-1.5L5 20z M9 9h6 M9 13h6"
            }
            Icon::Pay => "M3 7h18v12H3z M3 11h18 M15 15h3",
            Icon::Recv => "M12 3v10 M8 9l4 4 4-4 M4 15v4h16v-4",
            Icon::Chat => "M4 5h16v11H9l-5 4z M8 9h8 M8 12h5",
            Icon::Book => "M4 5a2 2 0 0 1 2-2h13v15H6a2 2 0 0 0-2 2z M4 20V5 M8 7h7 M8 11h5",
            Icon::Scale => {
                "M12 4v16 M7 20h10 M6 4h12 M3 11l3-7 3 7a3 3 0 0 1-6 0z M15 11l3-7 3 7a3 3 0 0 1-6 0z"
            }
            Icon::Users => {
                "M9 11a3.5 3.5 0 1 0 0-7a3.5 3.5 0 0 0 0 7z M3 20a6 6 0 0 1 12 0 M16 4.5a3.5 3.5 0 0 1 0 6.5 M18 14a6 6 0 0 1 3 6"
            }
            Icon::Gst => {
                "M19 5L5 19 M7 4.5a2.5 2.5 0 1 0 0 5a2.5 2.5 0 0 0 0-5z M17 14.5a2.5 2.5 0 1 0 0 5a2.5 2.5 0 0 0 0-5z"
            }
            Icon::Bank => "M3 9l9-5 9 5 M5 10v7 M10 10v7 M14 10v7 M19 10v7 M3 20h18",
            Icon::Shield => "M12 3l8 3v6c0 4.5-3.4 8-8 9-4.6-1-8-4.5-8-9V6z M9 12l2 2 4-4",
            Icon::Kbd => "M3 6h18v12H3z M7 10h.01 M11 10h.01 M15 10h.01 M18 10h.01 M7 14h10",
            Icon::Building => "M5 21V4h10v17 M15 9h4v12 M8 8h4 M8 12h4 M8 16h4 M3 21h18",
            Icon::Cal => "M4 6h16v14H4z M4 10h16 M8 3v4 M16 3v4",
            Icon::Lock => "M6 11h12v9H6z M8 11V8a4 4 0 0 1 8 0v3",
            Icon::Search => "M11 4a7 7 0 1 0 0 14a7 7 0 0 0 0-14z M20 20l-4-4",
            Icon::Cpu => {
                "M7 7h10v10H7z M10 10h4v4h-4z M9 3v4 M15 3v4 M9 17v4 M15 17v4 M3 9h4 M3 15h4 M17 9h4 M17 15h4"
            }
            Icon::Cloud => "M7 18a4.5 4.5 0 0 1-.5-9A6 6 0 0 1 18 10a4 4 0 0 1-1 8z",
            Icon::Off => "M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18z M6 6l12 12",
            Icon::Sun => {
                "M12 8a4 4 0 1 0 0 8a4 4 0 0 0 0-8z M12 2v2 M12 20v2 M4.9 4.9l1.4 1.4 M17.7 17.7l1.4 1.4 M2 12h2 M20 12h2 M4.9 19.1l1.4-1.4 M17.7 6.3l1.4-1.4"
            }
            Icon::Contrast => {
                "M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18z M12 3v18 M12 7h4 M12 11h6 M12 15h5"
            }
            Icon::Check => "M5 12l5 5 9-10",
            Icon::Chev => "M9 6l6 6-6 6",
            Icon::Spark => {
                "M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z M19 15.5l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z"
            }
            Icon::File => "M6 3h9l4 4v14H6z M14 3v5h5",
            Icon::Sliders => {
                "M4 6h9 M17 6h3 M15 4v4 M4 12h3 M11 12h9 M9 10v4 M4 18h11 M19 18h1 M17 16v4"
            }
            Icon::Plus => "M12 5v14 M5 12h14",
            Icon::Upload => "M12 16V4 M7 9l5-5 5 5 M4 20h16",
            Icon::Save => "M5 4h11l3 3v13H5z M8 4v5h7V4 M8 20v-6h8v6",
            Icon::X => "M6 6l12 12 M18 6L6 18",
        }
    }
}

/// Reads the numbers of SVG path data: signed decimals such as `-1.5`, `.01` or `0-7` (two
/// numbers), separated by spaces, commas or a sign.
struct Numbers<'a> {
    rest: &'a str,
}

impl Numbers<'_> {
    /// The next number, or `None` when the next token is a command letter or the data ended.
    fn next_number(&mut self) -> Option<f32> {
        self.rest = self.rest.trim_start_matches([' ', ',']);
        let bytes = self.rest.as_bytes();
        let mut end = 0;
        let mut seen_dot = false;
        let mut seen_digit = false;
        if let Some(b'-' | b'+') = bytes.first() {
            end = 1;
        }
        while let Some(&b) = bytes.get(end) {
            match b {
                b'0'..=b'9' => seen_digit = true,
                b'.' if !seen_dot => seen_dot = true,
                _ => break,
            }
            end += 1;
        }
        if !seen_digit {
            return None;
        }
        let (number, rest) = self.rest.split_at(end);
        self.rest = rest;
        number.parse().ok()
    }

    /// The next arc flag: a single `0` or `1`, which SVG lets touch the next number.
    fn next_flag(&mut self) -> Option<bool> {
        self.rest = self.rest.trim_start_matches([' ', ',']);
        let mut chars = self.rest.chars();
        let flag = match chars.next() {
            Some('0') => false,
            Some('1') => true,
            _ => return None,
        };
        self.rest = chars.as_str();
        Some(flag)
    }
}

/// Parses SVG path data into polylines in view-box units, one per subpath. A closed subpath ends
/// at its start point. Unknown commands end the parse; what was read so far is returned.
#[must_use]
pub fn parse(data: &str) -> Vec<Vec<Pos2>> {
    let mut lines: Vec<Vec<Pos2>> = Vec::new();
    let mut current: Vec<Pos2> = Vec::new();
    let mut pen = Pos2::ZERO;
    let mut start = Pos2::ZERO;
    let mut nums = Numbers { rest: data };
    let mut command: Option<char> = None;
    loop {
        nums.rest = nums.rest.trim_start_matches([' ', ',']);
        let Some(next) = nums.rest.chars().next() else {
            break;
        };
        if next.is_ascii_alphabetic() {
            command = Some(next);
            nums.rest = nums.rest.get(next.len_utf8()..).unwrap_or("");
            if matches!(next, 'z' | 'Z') {
                current.push(start);
                pen = start;
                continue;
            }
        }
        let Some(cmd) = command else {
            break;
        };
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { pen } else { Pos2::ZERO };
        let read = match cmd.to_ascii_uppercase() {
            'M' => nums.next_number().zip(nums.next_number()).map(|(x, y)| {
                if current.len() > 1 {
                    lines.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                pen = base + vec2(x, y);
                start = pen;
                current.push(pen);
                // Further pairs after a move are line-tos (SVG 1.1, 8.3.2).
                command = Some(if rel { 'l' } else { 'L' });
            }),
            'L' => nums.next_number().zip(nums.next_number()).map(|(x, y)| {
                pen = base + vec2(x, y);
                current.push(pen);
            }),
            'H' => nums.next_number().map(|x| {
                pen.x = if rel { pen.x + x } else { x };
                current.push(pen);
            }),
            'V' => nums.next_number().map(|y| {
                pen.y = if rel { pen.y + y } else { y };
                current.push(pen);
            }),
            'C' => read_cubic(&mut nums, base).map(|(c1, c2, end)| {
                let from = pen;
                for step in 1..=CURVE_STEPS {
                    current.push(cubic_at(from, c1, c2, end, step_t(step)));
                }
                pen = end;
            }),
            'A' => read_arc(&mut nums, base).map(|arc| {
                current.extend(flatten_arc(pen, &arc));
                pen = arc.end;
            }),
            _ => None,
        };
        if read.is_none() {
            break;
        }
    }
    if current.len() > 1 {
        lines.push(current);
    }
    lines
}

/// The fraction of the way along a curve at `step` of [`CURVE_STEPS`].
#[allow(
    clippy::cast_precision_loss,
    reason = "step and CURVE_STEPS are at most 12, exact in f32"
)]
fn step_t(step: usize) -> f32 {
    step as f32 / CURVE_STEPS as f32
}

/// Reads a cubic Bézier's two control points and end point.
fn read_cubic(nums: &mut Numbers<'_>, base: Pos2) -> Option<(Pos2, Pos2, Pos2)> {
    let mut point = || {
        nums.next_number()
            .zip(nums.next_number())
            .map(|(x, y)| base + vec2(x, y))
    };
    Some((point()?, point()?, point()?))
}

/// The point at `t` on a cubic Bézier.
#[allow(
    clippy::float_arithmetic,
    clippy::many_single_char_names,
    reason = "the textbook Bernstein form"
)]
fn cubic_at(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let a = u * u * u;
    let b = 3.0 * u * u * t;
    let c = 3.0 * u * t * t;
    let d = t * t * t;
    pos2(
        a * p0.x + b * p1.x + c * p2.x + d * p3.x,
        a * p0.y + b * p1.y + c * p2.y + d * p3.y,
    )
}

/// An elliptical arc command's parameters (SVG `A rx ry rotation large sweep x y`).
struct Arc {
    rx: f32,
    ry: f32,
    rotation: f32,
    large: bool,
    sweep: bool,
    end: Pos2,
}

/// Reads one arc command's seven parameters.
fn read_arc(nums: &mut Numbers<'_>, base: Pos2) -> Option<Arc> {
    let rx = nums.next_number()?;
    let ry = nums.next_number()?;
    let rotation = nums.next_number()?;
    let large = nums.next_flag()?;
    let sweep = nums.next_flag()?;
    let x = nums.next_number()?;
    let y = nums.next_number()?;
    Some(Arc {
        rx,
        ry,
        rotation,
        large,
        sweep,
        end: base + vec2(x, y),
    })
}

/// The points of an arc from `from`, by the endpoint-to-centre conversion of SVG 1.1, appendix
/// F.6.5, with the radii scaled up when they are too small (F.6.6). The start point is not
/// included; the end point is.
#[allow(clippy::float_arithmetic, reason = "geometry")]
fn flatten_arc(from: Pos2, arc: &Arc) -> Vec<Pos2> {
    let mut rx = arc.rx.abs();
    let mut ry = arc.ry.abs();
    if rx == 0.0 || ry == 0.0 || from == arc.end {
        return vec![arc.end];
    }
    let (sin, cos) = arc.rotation.to_radians().sin_cos();
    let dx = (from.x - arc.end.x) / 2.0;
    let dy = (from.y - arc.end.y) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let scale = lambda.sqrt();
        rx *= scale;
        ry *= scale;
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut coef = if den == 0.0 { 0.0 } else { (num / den).sqrt() };
    if arc.large == arc.sweep {
        coef = -coef;
    }
    let cx1 = coef * rx * y1 / ry;
    let cy1 = -coef * ry * x1 / rx;
    let cx = cos * cx1 - sin * cy1 + f32::midpoint(from.x, arc.end.x);
    let cy = sin * cx1 + cos * cy1 + f32::midpoint(from.y, arc.end.y);
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let theta1 = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut delta = angle(
        (x1 - cx1) / rx,
        (y1 - cy1) / ry,
        (-x1 - cx1) / rx,
        (-y1 - cy1) / ry,
    );
    let full = std::f32::consts::TAU;
    if !arc.sweep && delta > 0.0 {
        delta -= full;
    } else if arc.sweep && delta < 0.0 {
        delta += full;
    }
    let mut out = Vec::with_capacity(CURVE_STEPS);
    for step in 1..=CURVE_STEPS {
        if step == CURVE_STEPS {
            out.push(arc.end);
            break;
        }
        let theta = theta1 + delta * step_t(step);
        let (s, c) = theta.sin_cos();
        out.push(pos2(
            cx + cos * rx * c - sin * ry * s,
            cy + sin * rx * c + cos * ry * s,
        ));
    }
    out
}

/// Strokes `icon` into `rect` (a square) in `color`, scaled from the 24-unit view box.
#[allow(clippy::float_arithmetic, reason = "scaling the view box")]
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let scale = rect.width() / VIEW_BOX;
    let stroke = Stroke::new(STROKE_UNITS * scale, color);
    for line in parse(icon.path()) {
        let points: Vec<Pos2> = line
            .iter()
            .map(|p| rect.min + vec2(p.x * scale, p.y * scale))
            .collect();
        if let [only] = points.as_slice() {
            // A dot such as `h.01`: a round cap of the stroke width.
            painter.circle_filled(*only, stroke.width / 2.0, color);
        } else if points.len() == 2 && points.first() == points.get(1) {
            if let Some(dot) = points.first() {
                painter.circle_filled(*dot, stroke.width / 2.0, color);
            }
        } else {
            painter.add(egui::Shape::line(points, stroke));
            for end in [line.first(), line.last()].into_iter().flatten() {
                // Round caps and joins at the ends (`stroke-linecap:round`).
                painter.circle_filled(
                    rect.min + vec2(end.x * scale, end.y * scale),
                    stroke.width / 2.0,
                    color,
                );
            }
        }
    }
}

/// Allocates an icon-sized square and strokes `icon` in it.
pub fn icon(ui: &mut Ui, icon: Icon, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(ICON_SIZE), Sense::hover());
    paint(ui.painter(), rect, icon, color);
    response
}
