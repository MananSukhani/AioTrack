//! Two painted shapes: the rising background wave and the monthly pie.
//!
//! Both take an explicit rect and progress so screens can choreograph them
//! without a second layout pass.

use eframe::egui::{pos2, vec2, Align2, Color32, Mesh, Rect, Shape, Stroke, Ui};

use crate::theme::*;

pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// A single soft wash that fills `fraction` of the screen from the bottom.
/// Alpha eases in from the crest so the colour never forms a hard band.
pub fn wave(ui: &Ui, rect: Rect, fraction: f32, time: f32) {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.002 || rect.height() <= 1.0 {
        return;
    }
    let color = wave_color(fraction);
    let surface = rect.bottom() - rect.height() * fraction;
    let height = rect.height() * fraction;

    // Alpha ramp: invisible just above the crest, a quiet wash at the bottom.
    let rows: [(f32, u8); 7] = [
        (-0.18, 0),
        (-0.06, 0),
        (0.0, 22),
        (0.20, 52),
        (0.48, 88),
        (0.76, 120),
        (1.0, 150),
    ];
    let columns = 64usize;
    let mut mesh = Mesh::default();
    for xi in 0..=columns {
        let t = xi as f32 / columns as f32;
        let x = rect.left() + rect.width() * t;
        let crest_y = surface + crest(t, 5.0, time * 0.9, 1.8);
        for (ri, (level, opacity)) in rows.iter().enumerate() {
            let y = if ri == rows.len() - 1 {
                rect.bottom() + 2.0
            } else {
                crest_y + level * height
            };
            mesh.colored_vertex(pos2(x, y), alpha(color, *opacity));
        }
    }
    for xi in 0..columns {
        for ri in 0..(rows.len() - 1) {
            let base = (xi * rows.len() + ri) as u32;
            let next = base + rows.len() as u32;
            mesh.add_triangle(base, base + 1, next);
            mesh.add_triangle(next, base + 1, next + 1);
        }
    }
    ui.painter().add(Shape::mesh(mesh));

    let mut crest_points = Vec::with_capacity(columns + 1);
    for xi in 0..=columns {
        let t = xi as f32 / columns as f32;
        crest_points.push(pos2(
            rect.left() + rect.width() * t,
            surface + crest(t, 5.0, time * 0.9, 1.8),
        ));
    }
    ui.painter().add(Shape::line(
        crest_points,
        Stroke::new(1.4_f32, alpha(color, 130)),
    ));
}

fn crest(t: f32, amplitude: f32, phase: f32, drift: f32) -> f32 {
    let a = (t * std::f32::consts::TAU * 1.05 + phase).sin();
    let b = (t * std::f32::consts::TAU * 0.58 + phase * 0.7 + drift).sin();
    amplitude * (a * 0.65 + b * 0.35)
}

/// A quiet donut: generous gaps, a recessed track and a hollow centre. The
/// colours come from `theme::pie_color`, never from the category chips.
pub fn pie(ui: &Ui, rect: Rect, slices: &[(Color32, i64)], total: i64, progress: f32) {
    let side = rect.width().min(rect.height());
    let outer = (side * 0.5 - 10.0).max(24.0);
    let eased = ease_out(progress);
    // The donut opens from a single radius: the hole grows while the sweep
    // travels around, so it reads as a line unfolding into a full circle.
    let inner = outer * 0.63 * eased;
    let center = rect.center();

    // Recessed track, visible as the gap between slices.
    ui.painter().circle_stroke(
        center,
        (outer + inner) * 0.5,
        Stroke::new(outer - inner, alpha(WHITE, 10)),
    );

    if total <= 0 || slices.is_empty() {
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            "No spending",
            crate::theme::sans_font(12.0),
            faint(),
        );
        return;
    }

    let gap: f32 = if slices.len() > 1 { 0.02 } else { 0.0 };
    let mut angle = -std::f32::consts::FRAC_PI_2;
    for (color, value) in slices {
        let sweep = (*value as f32 / total as f32) * std::f32::consts::TAU * eased;
        if sweep <= 0.0001 {
            continue;
        }
        let inset = gap.min(sweep * 0.16);
        draw_arc(
            ui,
            center,
            inner,
            outer,
            angle + inset * 0.5,
            angle + sweep - inset * 0.5,
            *color,
        );
        angle += sweep;
    }

    ui.painter()
        .circle_stroke(center, inner, Stroke::new(1.0_f32, alpha(WHITE, 22)));
    ui.painter()
        .circle_stroke(center, outer, Stroke::new(1.0_f32, alpha(WHITE, 14)));
}

fn draw_arc(
    ui: &Ui,
    center: eframe::egui::Pos2,
    inner: f32,
    outer: f32,
    start: f32,
    end: f32,
    color: Color32,
) {
    let steps = (((end - start) / std::f32::consts::TAU) * 180.0)
        .ceil()
        .max(4.0) as usize;
    let mut mesh = Mesh::default();
    for i in 0..=steps {
        let angle = start + (end - start) * (i as f32 / steps as f32);
        let direction = vec2(angle.cos(), angle.sin());
        mesh.colored_vertex(center + direction * inner, color);
        mesh.colored_vertex(center + direction * outer, color);
    }
    for i in 0..steps {
        let base = (i * 2) as u32;
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 2, base + 1, base + 3);
    }
    ui.painter().add(Shape::mesh(mesh));
}
