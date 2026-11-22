//! Shared building blocks for the black-and-white interface.
//!
//! Everything is painted by hand so the app keeps one quiet rhythm. The only
//! stateful piece is the kinetic scroller used by the ledger and analysis
//! pages.

use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, CursorIcon, Frame, Id, Layout, Pos2,
    Rect, Response, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, Vec2,
};
use std::time::Duration;

use crate::icons::{self, Icon};
use crate::model::{transaction_time_label, Transaction};
use crate::theme::*;

// Motion ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub dx: f32,
    pub dy: f32,
    pub opacity: f32,
}

impl Motion {
    pub fn still() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            opacity: 1.0,
        }
    }

    /// Long, soft horizontal slide used for Analysis and Settings.
    pub fn from(progress: f32, direction: f32) -> Self {
        let t = progress.clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(5);
        Self {
            dx: direction * 58.0 * (1.0 - eased),
            dy: 0.0,
            opacity: t * t * (3.0 - 2.0 * t),
        }
    }

    /// Vertical lift used for the activity ledger.
    pub fn vertical(progress: f32, direction: f32) -> Self {
        let t = progress.clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(5);
        Self {
            dx: 0.0,
            dy: direction * 46.0 * (1.0 - eased),
            opacity: t * t * (3.0 - 2.0 * t),
        }
    }
}

// Layout ---------------------------------------------------------------------

pub fn column<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let available = ui.available_width();
    let width = (available - PAGE_PAD * 2.0)
        .clamp(220.0, CONTENT_MAX)
        .min(available);
    let side = ((available - width) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(side);
        ui.vertical(|ui| {
            ui.set_width(width);
            add(ui)
        })
        .inner
    })
    .inner
}

#[derive(Clone, Copy)]
struct ScrollState {
    offset: f32,
    velocity: f32,
    content: f32,
    touched: f64,
}

/// A vertical scroller with kinetic momentum and a rubber band at both ends.
pub fn smooth_scroll<R>(ui: &mut Ui, salt: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let ctx = ui.ctx().clone();
    let id = Id::new(("aiotrack-scroll", salt));
    let outer = ui.available_rect_before_wrap();
    if outer.height() <= 2.0 {
        return add(ui);
    }
    let dt = ctx.input(|i| i.stable_dt).clamp(1.0 / 240.0, 0.05) as f32;
    let now = ctx.input(|i| i.time);
    let drag = ui.interact(outer, Id::new((salt, "scroll-drag")), Sense::drag());

    let mut state = ctx
        .data(|data| data.get_temp::<ScrollState>(id))
        .unwrap_or(ScrollState {
            offset: 0.0,
            velocity: 0.0,
            content: outer.height(),
            touched: now,
        });

    let mut delta = 0.0f32;
    let dragging = drag.dragged();
    if dragging {
        let movement = -drag.drag_delta().y;
        delta += movement;
        let instant = (movement / dt).clamp(-7000.0, 7000.0);
        state.velocity = state.velocity * 0.45 + instant * 0.55;
        state.touched = now;
    } else {
        let wheel = ctx.input(|i| i.raw_scroll_delta.y);
        if wheel.abs() > 0.01 {
            delta -= wheel;
            state.velocity = 0.0;
            state.touched = now;
        }
    }

    let max_offset = (state.content - outer.height()).max(0.0);
    let mut offset = state.offset + delta;

    if !dragging && state.velocity.abs() > 6.0 {
        offset += state.velocity * dt;
        state.velocity *= (-5.2 * dt).exp();
        if state.velocity.abs() < 8.0 {
            state.velocity = 0.0;
        }
        ctx.request_repaint();
    }

    if dragging {
        if offset < 0.0 {
            offset *= 0.45;
        } else if offset > max_offset {
            offset = max_offset + (offset - max_offset) * 0.45;
        }
    } else {
        let settle = 1.0 - (-16.0 * dt).exp();
        if offset < 0.0 {
            offset += (0.0 - offset) * settle;
            state.velocity = 0.0;
        } else if offset > max_offset {
            offset += (max_offset - offset) * settle;
            state.velocity = 0.0;
        }
    }

    let moving = dragging || state.velocity.abs() > 0.1 || (offset - state.offset).abs() > 0.05;
    if moving {
        ctx.request_repaint();
    }
    state.offset = offset;

    let top = outer.top() - offset;
    let mut content = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                pos2(outer.left(), top),
                vec2(outer.width(), 100_000.0),
            ))
            .layout(Layout::top_down(Align::Min)),
    );
    content.set_clip_rect(outer.intersect(ui.clip_rect()));
    let out = add(&mut content);
    let height = (content.min_rect().bottom() - top).max(outer.height());
    state.content = height;

    // A hairline thumb, only while the surface is moving.
    let idle = (now - state.touched) as f32;
    let fade = (1.0 - idle / 0.8).clamp(0.0, 1.0);
    let max_offset = (height - outer.height()).max(0.0);
    if max_offset > 1.0 && fade > 0.01 {
        let track = outer.height() - 16.0;
        let thumb = (outer.height() * outer.height() / height).clamp(30.0, track);
        let t = (offset / max_offset).clamp(0.0, 1.0);
        let y = outer.top() + 8.0 + t * (track - thumb);
        let bar = Rect::from_min_size(pos2(outer.right() - 3.0, y), vec2(2.0, thumb));
        ui.painter().rect_filled(
            bar,
            CornerRadius::same(1),
            alpha(WHITE, (90.0 * fade) as u8),
        );
        ctx.request_repaint_after(Duration::from_millis(40));
    }

    ctx.data_mut(|data| data.insert_temp(id, state));
    ui.allocate_rect(outer, Sense::hover());
    out
}

/// Current scroll offset of a `smooth_scroll` surface, used for pull-to-go-home.
pub fn scroll_offset(ctx: &egui::Context, salt: &str) -> f32 {
    ctx.data(|data| {
        data.get_temp::<ScrollState>(Id::new(("aiotrack-scroll", salt)))
            .map(|state| state.offset)
            .unwrap_or(0.0)
    })
}

/// A page with a status-bar-friendly top, room for the bottom control and a
/// subtle horizontal entrance.
pub fn page<R>(ui: &mut Ui, salt: &str, motion: Motion, add: impl FnOnce(&mut Ui) -> R) -> R {
    let outer = ui.available_rect_before_wrap();
    let rect = outer.translate(vec2(motion.dx, motion.dy));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(outer.intersect(ui.clip_rect()));
    child.set_opacity(motion.opacity);
    let out = smooth_scroll(&mut child, salt, |ui| {
        column(ui, |ui| {
            ui.add_space(TOP_PAD);
            let out = add(ui);
            ui.add_space(BOTTOM_PAD + 82.0);
            out
        })
    });
    ui.allocate_rect(outer, Sense::hover());
    out
}

/// A full-screen sheet: same rhythm, no bottom navigation.
pub fn sheet_page<R>(ui: &mut Ui, salt: &str, motion: Motion, add: impl FnOnce(&mut Ui) -> R) -> R {
    let outer = ui.available_rect_before_wrap();
    let rect = outer.translate(vec2(motion.dx, motion.dy));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(outer.intersect(ui.clip_rect()));
    child.set_opacity(motion.opacity);
    let out = smooth_scroll(&mut child, salt, |ui| {
        column(ui, |ui| {
            ui.add_space(TOP_PAD);
            let out = add(ui);
            ui.add_space(BOTTOM_PAD + 24.0);
            out
        })
    });
    ui.allocate_rect(outer, Sense::hover());
    out
}

pub fn gap(ui: &mut Ui, height: f32) {
    ui.add_space(height);
}

pub fn hairline(ui: &mut Ui, inset: f32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(
        Rect::from_min_max(
            pos2(rect.left() + inset, rect.top()),
            pos2(rect.right(), rect.bottom()),
        ),
        0,
        line(),
    );
}

// Type -----------------------------------------------------------------------

pub fn overline(ui: &mut Ui, text: impl Into<String>) {
    ui.label(
        RichText::new(text.into().to_uppercase())
            .font(semibold(FS_MICRO))
            .color(faint())
            .line_height(Some(14.0)),
    );
}

pub fn muted(ui: &mut Ui, text: impl Into<String>, size: f32) {
    ui.label(
        RichText::new(text.into())
            .font(sans_font(size))
            .color(crate::theme::muted()),
    );
}

pub fn title(ui: &mut Ui, text: impl Into<String>) {
    ui.label(
        RichText::new(text.into())
            .font(semibold(FS_HEADLINE))
            .color(ink())
            .line_height(Some(FS_HEADLINE + 4.0)),
    );
}

pub fn big(ui: &mut Ui, text: impl Into<String>, size: f32) {
    ui.label(
        RichText::new(text.into())
            .font(display_font(size))
            .color(ink())
            .line_height(Some(size + 6.0)),
    );
}

/// A page title with an optional back control.
pub fn title_row(ui: &mut Ui, label: &str, back: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        if back {
            if icon_button(ui, Icon::Back, 42.0, ink()).clicked() {
                clicked = true;
            }
            ui.add_space(4.0);
        }
        ui.label(
            RichText::new(label)
                .font(semibold(FS_HEADLINE))
                .color(ink())
                .line_height(Some(FS_HEADLINE + 4.0)),
        );
    });
    clicked
}

// Buttons --------------------------------------------------------------------

pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if response.is_pointer_button_down_on() {
        ui.painter()
            .circle_filled(rect.center(), size * 0.5, alpha(WHITE, 18));
    } else if response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), size * 0.5, alpha(WHITE, 9));
    }
    icons::paint(
        ui.painter(),
        icon,
        Rect::from_center_size(rect.center(), Vec2::splat(size * 0.44)),
        color,
        1.6,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A white pill with black text — the single primary action.
pub fn white_button(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 54.0), Sense::click());
    let press = response.is_pointer_button_down_on();
    let fill = if !enabled {
        raised()
    } else if press {
        Color32::from_rgb(220, 220, 220)
    } else {
        WHITE
    };
    let rect = rect.shrink(if press { 1.5 } else { 0.0 });
    ui.painter().rect_filled(rect, CornerRadius::same(27), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        semibold(15.0),
        if enabled { BLACK } else { faint() },
    );
    response.on_hover_cursor(if enabled {
        CursorIcon::PointingHand
    } else {
        CursorIcon::Default
    })
}

pub fn ghost_button(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 52.0), Sense::click());
    let fill = if response.is_pointer_button_down_on() {
        alpha(WHITE, 14)
    } else if response.hovered() {
        alpha(WHITE, 7)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(26), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(26),
        Stroke::new(1.0_f32, line()),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        semibold(14.0),
        if enabled { ink() } else { faint() },
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn text_button(ui: &mut Ui, label: &str, color: Color32) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), semibold(13.0), color);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(16.0, 12.0), Sense::click());
    ui.painter().galley(
        rect.center() - galley.size() * 0.5,
        galley,
        if response.hovered() {
            mix(color, WHITE, 0.35)
        } else {
            color
        },
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A tappable settings row with a leading glyph, a title, an optional value
/// and a trailing chevron.
pub fn row(ui: &mut Ui, icon: Icon, label: &str, value: Option<&str>, danger: bool) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 62.0), Sense::click());
    if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(14), alpha(WHITE, 10));
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(14), alpha(WHITE, 5));
    }
    let color = if danger { bad() } else { ink() };
    icons::paint(
        ui.painter(),
        icon,
        Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), Vec2::splat(18.0)),
        color,
        1.6,
    );
    ui.painter().text(
        pos2(rect.left() + 48.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        medium(15.0),
        color,
    );
    if let Some(value) = value {
        let galley =
            ui.painter()
                .layout_no_wrap(value.to_owned(), sans_font(13.0), crate::theme::muted());
        ui.painter().galley(
            pos2(
                rect.right() - 32.0 - galley.size().x,
                rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            crate::theme::muted(),
        );
    }
    icons::paint_center(
        ui.painter(),
        Icon::ChevronRight,
        pos2(rect.right() - 18.0, rect.center().y),
        15.0,
        faint(),
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A plain text field with a hairline underline. `numeric` only filters the
/// characters; the Android IME type is set by the platform layer when the
/// field gains focus.
pub fn field(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    text: &mut String,
    hint: &str,
    size: f32,
    numeric: bool,
    center: bool,
) -> Response {
    let font = if numeric {
        display_font(size)
    } else {
        semibold(size)
    };
    let mut edit = TextEdit::singleline(text)
        .id(Id::new(id))
        .hint_text(RichText::new(hint).font(font.clone()).color(faint()))
        .font(font)
        .text_color(ink())
        .frame(false)
        .desired_width(f32::INFINITY);
    if numeric {
        edit = edit.char_limit(14);
    }
    if center {
        edit = edit.horizontal_align(Align::Center);
    }
    let response = ui.add(edit);
    if numeric {
        sanitize_amount(text);
    }
    let y = response.rect.bottom() + 6.0;
    ui.painter().line_segment(
        [
            pos2(response.rect.left(), y),
            pos2(response.rect.right(), y),
        ],
        Stroke::new(1.0_f32, if response.has_focus() { WHITE } else { line() }),
    );
    response
}

pub fn sanitize_amount(text: &mut String) {
    let mut out = String::with_capacity(text.len());
    let mut dot = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            if let Some(index) = out.find('.') {
                if out.len() - index - 1 >= 2 {
                    continue;
                }
            }
            if out == "0" {
                out.clear();
            }
            out.push(ch);
        } else if (ch == '.' || ch == ',') && !dot {
            dot = true;
            if out.is_empty() {
                out.push('0');
            }
            out.push('.');
        }
    }
    if out != *text {
        *text = out;
    }
}

// Ledger ---------------------------------------------------------------------

fn truncate(ui: &Ui, text: &str, font_id: eframe::egui::FontId, max_width: f32) -> String {
    if max_width <= 20.0 {
        return text.to_owned();
    }
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font_id.clone(), ink());
    if galley.size().x <= max_width {
        return text.to_owned();
    }
    let mut out = text.to_owned();
    while out.chars().count() > 3 {
        out.pop();
        let galley = ui
            .painter()
            .layout_no_wrap(format!("{}…", out), font_id.clone(), ink());
        if galley.size().x <= max_width {
            return format!("{}…", out);
        }
    }
    "…".to_owned()
}

pub fn ledger_row(
    ui: &mut Ui,
    tx: &Transaction,
    category: &str,
    account: &str,
    amount: &str,
) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 66.0), Sense::click());
    if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(14), alpha(WHITE, 10));
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(14), alpha(WHITE, 5));
    }

    let primary = if tx.note.trim().is_empty() {
        category.to_owned()
    } else {
        tx.note.clone()
    };
    ui.painter().text(
        pos2(rect.left() + 2.0, rect.center().y - 11.0),
        Align2::LEFT_CENTER,
        truncate(ui, &primary, semibold(15.0), rect.width() - 150.0),
        semibold(15.0),
        ink(),
    );
    let secondary = if tx.note.trim().is_empty() {
        format!("{} · {}", transaction_time_label(tx), account)
    } else {
        format!(
            "{} · {} · {}",
            category,
            transaction_time_label(tx),
            account
        )
    };
    ui.painter().text(
        pos2(rect.left() + 2.0, rect.center().y + 12.0),
        Align2::LEFT_CENTER,
        truncate(ui, &secondary, sans_font(12.0), rect.width() - 130.0),
        sans_font(12.0),
        crate::theme::muted(),
    );
    ui.painter().text(
        pos2(rect.right() - 2.0, rect.center().y),
        Align2::RIGHT_CENTER,
        amount,
        semibold(15.0),
        ink(),
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

// Fixed controls -------------------------------------------------------------

/// A white circular control anchored above the system navigation bar.
pub fn bottom_circle(ctx: &egui::Context, id: &str, icon: Icon, size: f32, offset: f32) -> bool {
    let mut clicked = false;
    egui::Area::new(Id::new(("bottom-circle", id)))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -offset))
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
            let press = response.is_pointer_button_down_on();
            let radius = size * 0.5 - if press { 1.5 } else { 0.0 };
            let fill = if press {
                Color32::from_rgb(225, 225, 225)
            } else {
                WHITE
            };
            ui.painter().circle_filled(rect.center(), radius, fill);
            icons::paint_center(ui.painter(), icon, rect.center(), size * 0.38, BLACK);
            if response.clicked() {
                clicked = true;
            }
            response.on_hover_cursor(CursorIcon::PointingHand);
        });
    clicked
}

pub fn toast(ctx: &egui::Context, message: &str, started: f64, now: f64) {
    let age = now - started;
    if age > 2.6 {
        return;
    }
    ctx.request_repaint_after(Duration::from_millis(40));
    let fade = if age < 0.18 {
        (age / 0.18) as f32
    } else if age > 2.1 {
        ((2.6 - age) / 0.5).clamp(0.0, 1.0) as f32
    } else {
        1.0
    };
    let ease = 1.0 - (1.0 - fade).powi(3);
    let a = (ease * 255.0) as u8;
    let rise = (1.0 - ease) * 12.0;
    egui::Area::new(Id::new("aiotrack-toast"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -108.0 + rise))
        .interactable(false)
        .show(ctx, |ui| {
            let galley =
                ui.painter()
                    .layout_no_wrap(message.to_owned(), medium(13.0), alpha(WHITE, a));
            let width = galley.size().x + 44.0;
            let (rect, _) = ui.allocate_exact_size(vec2(width, 42.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(21), alpha(raised(), a.min(250)));
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(21),
                Stroke::new(1.0_f32, alpha(line(), a)),
                StrokeKind::Inside,
            );
            ui.painter().galley(
                pos2(rect.left() + 22.0, rect.center().y - galley.size().y * 0.5),
                galley,
                alpha(WHITE, a),
            );
        });
}

// Misc -----------------------------------------------------------------------

pub fn empty(ui: &mut Ui, icon: Icon, title_text: &str, body: &str) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 180.0), Sense::hover());
    icons::paint_center(
        ui.painter(),
        icon,
        pos2(rect.center().x, rect.top() + 36.0),
        26.0,
        faint(),
    );
    ui.painter().text(
        pos2(rect.center().x, rect.top() + 74.0),
        Align2::CENTER_TOP,
        title_text,
        semibold(16.0),
        ink(),
    );
    let galley = ui.painter().layout(
        body.to_owned(),
        sans_font(12.5),
        crate::theme::muted(),
        width - 64.0,
    );
    ui.painter().galley(
        pos2(rect.center().x - galley.size().x * 0.5, rect.top() + 102.0),
        galley,
        crate::theme::muted(),
    );
}

pub fn dot(ui: &mut Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().circle_filled(rect.center(), size * 0.5, color);
}

pub fn soft_shadow(painter: &egui::Painter, rect: Rect, radius: u8) {
    for (spread, a, dy) in [(8.0, 22u8, 3.0), (3.0, 36, 2.0)] {
        painter.rect_filled(
            rect.translate(vec2(0.0, dy)).expand(spread),
            CornerRadius::same(radius),
            Color32::from_rgba_unmultiplied(0, 0, 0, a),
        );
    }
}

#[allow(dead_code)]
pub fn point(x: f32, y: f32) -> Pos2 {
    pos2(x, y)
}

#[allow(dead_code)]
pub fn frame_black() -> Frame {
    Frame::new().fill(BLACK)
}
