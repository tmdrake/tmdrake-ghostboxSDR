//! Phosphor ghost-box HUD: night-vision green on black.

use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Ui, Vec2};

pub const PHOSPHOR: Color32 = Color32::from_rgb(0, 255, 106);
pub const PHOSPHOR_DIM: Color32 = Color32::from_rgb(40, 140, 70);
pub const PHOSPHOR_HOT: Color32 = Color32::from_rgb(190, 255, 210);
pub const VOID: Color32 = Color32::from_rgb(3, 6, 4);
pub const PANEL: Color32 = Color32::from_rgb(6, 12, 8);
pub const SCOPE: Color32 = Color32::from_rgb(4, 10, 6);
pub const AMBER: Color32 = Color32::from_rgb(255, 176, 32);
pub const ALERT: Color32 = Color32::from_rgb(255, 56, 56);
pub const CYAN: Color32 = Color32::from_rgb(0, 220, 255);
pub const MAGENTA: Color32 = Color32::from_rgb(255, 64, 180);

/// Overlay colors for the four contacts (A–D).
pub const CONTACT: [Color32; 4] = [PHOSPHOR, CYAN, MAGENTA, AMBER];

fn stroke(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.dark_mode = true;
    v.override_text_color = Some(PHOSPHOR_HOT);
    v.weak_text_color = Some(PHOSPHOR_DIM);
    v.hyperlink_color = CYAN;
    v.warn_fg_color = AMBER;
    v.error_fg_color = ALERT;
    v.faint_bg_color = Color32::from_rgb(8, 18, 10);
    v.extreme_bg_color = VOID;
    v.code_bg_color = Color32::from_rgb(8, 22, 12);
    v.window_fill = PANEL;
    v.panel_fill = PANEL;
    v.window_stroke = stroke(1.0, Color32::from_rgb(0, 70, 32));
    v.window_corner_radius = CornerRadius::same(2);
    v.menu_corner_radius = CornerRadius::same(2);
    v.selection.bg_fill = Color32::from_rgb(0, 70, 32);
    v.selection.stroke = stroke(1.0, PHOSPHOR);
    v.slider_trailing_fill = true;

    let stroke_dim = stroke(1.0, Color32::from_rgb(0, 80, 36));
    let stroke_hot = stroke(1.0, PHOSPHOR);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = stroke_dim;
    v.widgets.noninteractive.fg_stroke = stroke(1.0, PHOSPHOR_DIM);
    v.widgets.inactive.bg_fill = Color32::from_rgb(8, 20, 12);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(8, 20, 12);
    v.widgets.inactive.bg_stroke = stroke_dim;
    v.widgets.inactive.fg_stroke = stroke(1.0, PHOSPHOR);
    v.widgets.hovered.bg_fill = Color32::from_rgb(12, 36, 18);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(12, 36, 18);
    v.widgets.hovered.bg_stroke = stroke_hot;
    v.widgets.hovered.fg_stroke = stroke(1.0, PHOSPHOR_HOT);
    v.widgets.active.bg_fill = Color32::from_rgb(0, 50, 24);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0, 50, 24);
    v.widgets.active.bg_stroke = stroke_hot;
    v.widgets.active.fg_stroke = stroke(1.0, PHOSPHOR_HOT);
    v.widgets.open.bg_fill = Color32::from_rgb(8, 28, 14);
    v.widgets.open.weak_bg_fill = Color32::from_rgb(8, 28, 14);
    v.widgets.open.bg_stroke = stroke_hot;
    v.widgets.open.fg_stroke = stroke(1.0, PHOSPHOR_HOT);

    ctx.set_visuals(v);
}

pub fn hud_frame(painter: &egui::Painter, rect: Rect, label: &str) {
    painter.rect_stroke(
        rect,
        0.0,
        stroke(1.0, Color32::from_rgb(0, 90, 40)),
        egui::StrokeKind::Inside,
    );
    let l = 14.0;
    let s = stroke(2.0, PHOSPHOR);
    let (x0, y0, x1, y1) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    painter.line_segment([Pos2::new(x0, y0 + l), Pos2::new(x0, y0)], s);
    painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0 + l, y0)], s);
    painter.line_segment([Pos2::new(x1 - l, y0), Pos2::new(x1, y0)], s);
    painter.line_segment([Pos2::new(x1, y0), Pos2::new(x1, y0 + l)], s);
    painter.line_segment([Pos2::new(x0, y1 - l), Pos2::new(x0, y1)], s);
    painter.line_segment([Pos2::new(x0, y1), Pos2::new(x0 + l, y1)], s);
    painter.line_segment([Pos2::new(x1 - l, y1), Pos2::new(x1, y1)], s);
    painter.line_segment([Pos2::new(x1, y1), Pos2::new(x1, y1 - l)], s);
    if !label.is_empty() {
        painter.text(
            rect.left_top() + Vec2::new(10.0, 4.0),
            egui::Align2::LEFT_TOP,
            label,
            FontId::monospace(11.0),
            PHOSPHOR,
        );
    }
}

pub fn scanlines(painter: &egui::Painter, rect: Rect) {
    let mut y = rect.top();
    let line = Color32::from_rgba_unmultiplied(0, 255, 80, 12);
    while y < rect.bottom() {
        painter.hline(rect.x_range(), y, stroke(1.0, line));
        y += 3.0;
    }
}

/// Stick figure drawn at the top of a contact marker.
pub fn sls_figure(painter: &egui::Painter, x: f32, top: f32, color: Color32, scale: f32) {
    let s = scale;
    let head = Pos2::new(x, top + 7.0 * s);
    painter.circle_stroke(head, 4.0 * s, stroke(1.4, color));
    let neck = Pos2::new(x, top + 12.0 * s);
    let hip = Pos2::new(x, top + 26.0 * s);
    let l_arm = Pos2::new(x - 9.0 * s, top + 18.0 * s);
    let r_arm = Pos2::new(x + 9.0 * s, top + 18.0 * s);
    let l_leg = Pos2::new(x - 7.0 * s, top + 36.0 * s);
    let r_leg = Pos2::new(x + 7.0 * s, top + 36.0 * s);
    let st = stroke(1.5, color);
    painter.line_segment([neck, hip], st);
    painter.line_segment([Pos2::new(x, top + 16.0 * s), l_arm], st);
    painter.line_segment([Pos2::new(x, top + 16.0 * s), r_arm], st);
    painter.line_segment([hip, l_leg], st);
    painter.line_segment([hip, r_leg], st);
    for p in [neck, l_arm, r_arm, hip, l_leg, r_leg] {
        painter.circle_filled(p, 1.6 * s, color);
    }
}

pub fn emf_bar(ui: &mut Ui, rms: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(90.0, 14.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 1.0, VOID);
    painter.rect_stroke(
        rect,
        1.0,
        stroke(1.0, PHOSPHOR_DIM),
        egui::StrokeKind::Inside,
    );
    let t = (rms * 8.0).clamp(0.0, 1.0);
    let n = 8;
    let gap = 2.0;
    let w = (rect.width() - 4.0 - gap * (n as f32 - 1.0)) / n as f32;
    let lit = ((t * n as f32).ceil() as usize).min(n);
    for i in 0..n {
        let x = rect.left() + 2.0 + i as f32 * (w + gap);
        let cell = Rect::from_min_size(Pos2::new(x, rect.top() + 3.0), Vec2::new(w, 8.0));
        let color = if i < lit {
            if i >= 6 {
                ALERT
            } else if i >= 4 {
                AMBER
            } else {
                PHOSPHOR
            }
        } else {
            Color32::from_rgb(12, 28, 14)
        };
        painter.rect_filled(cell, 0.0, color);
    }
}

pub fn live_badge(ui: &mut Ui, sweeping: bool, time: f64) {
    let blink = ((time * 2.4) as i64) % 2 == 0;
    let (text, color) = if sweeping {
        (
            "SWEEP LIVE",
            if blink {
                ALERT
            } else {
                Color32::from_rgb(80, 16, 16)
            },
        )
    } else {
        ("HOLD", PHOSPHOR_DIM)
    };
    ui.label(
        egui::RichText::new(text)
            .monospace()
            .size(12.0)
            .color(color)
            .strong(),
    );
}
