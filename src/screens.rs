//! The four surfaces of AioTrack: today, activity, analysis and settings.
//!
//! Home is a single number on black with a wave that rises as the day's budget
//! is spent. The other surfaces are quiet lists that share one rhythm. All
//! navigation is painted by hand so the app never shows a chrome-heavy bar
//! unless the person explicitly chooses buttons over gestures.

use std::sync::Arc;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, Galley, Id, Pos2, Rect,
    Sense, Stroke, StrokeKind, Ui, Vec2,
};

use crate::android::{self, Haptic};
use crate::charts;
use crate::icons::{self, Icon};
use crate::model::{ordinal, CategoryId, Currency, NavMode, Transaction, TxKind, YearMonth};
use crate::theme::*;
use crate::widgets::{self, Motion};

// ---------------------------------------------------------------------------
// Navigation types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Home,
    Ledger,
    Analysis,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsPage {
    Root,
    Navigation,
    Accounts,
    Categories,
    Currency,
    Backup,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    None,
    Home,
    OpenLedger,
    OpenAnalysis,
    OpenAdvanced,
    CloseAdvanced,
    OpenSettings,
    Settings(SettingsPage),
    Back,
    ChangeMonth(i32),
    SetCurrency(Currency),
    SetNav(NavMode),
    ToggleHaptics,
    NewAccount,
    EditAccount(u64),
    DeleteAccount(u64),
    NewCategory,
    EditCategory(u64),
    DeleteCategory(u64),
    OpenComposer(CategoryId),
    EditTransaction(u64),
    DeleteTransaction(u64),
    OpenBudget,
    Backup,
    Restore,
    OpenUrl(&'static str),
    ResetAll,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HomeEvent {
    None,
    Add(CategoryId),
    NewCategory,
    SetBudget,
}

// ---------------------------------------------------------------------------
// Home
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Radial {
    started: f64,
    anchor: Pos2,
    active: bool,
    selected: Option<usize>,
}

impl Radial {
    fn idle() -> Self {
        Self {
            started: 0.0,
            anchor: Pos2::ZERO,
            active: false,
            selected: None,
        }
    }
}

fn radial_id() -> Id {
    Id::new("aiotrack-radial")
}

pub fn radial_active(ctx: &egui::Context) -> bool {
    ctx.data(|data| {
        data.get_temp::<bool>(Id::new("aiotrack-radial-open"))
            .unwrap_or(false)
    })
}

pub fn close_radial(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        data.insert_temp(radial_id(), Radial::idle());
        data.insert_temp(Id::new("aiotrack-radial-open"), false);
    });
}

struct Slot {
    id: Option<CategoryId>,
    icon: Icon,
    label: String,
}

fn radial_slots(state: &crate::model::AppState) -> Vec<Slot> {
    let mut categories: Vec<&crate::model::Category> = state.sorted_categories(TxKind::Expense);
    // Usage first, then categories the person made themselves, then newest.
    // This guarantees a brand-new category is always reachable from the hold
    // menu even before it has been used.
    categories.sort_by(|a, b| {
        state
            .category_usage(b.id)
            .cmp(&state.category_usage(a.id))
            .then_with(|| a.builtin.cmp(&b.builtin))
            .then_with(|| b.id.cmp(&a.id))
    });
    let mut slots: Vec<Slot> = categories
        .into_iter()
        .take(6)
        .map(|category| Slot {
            id: Some(CategoryId(category.id)),
            icon: icons::category(category.icon),
            label: category.name.clone(),
        })
        .collect();
    slots.push(Slot {
        id: None,
        icon: Icon::Plus,
        label: "New".to_owned(),
    });
    slots
}

const ARC_START: f32 = -2.55;
const ARC_END: f32 = -0.59;

/// Radius, bubble radius and centre spacing, all fitted to the screen width.
fn radial_geometry(screen_width: f32, count: usize) -> (f32, f32, f32) {
    let radius = ((screen_width * 0.5 - 36.0) / 0.855).min(196.0).max(118.0);
    let span = ARC_END - ARC_START;
    let spacing = if count > 1 {
        radius * span / (count - 1) as f32
    } else {
        radius * span
    };
    let circle = (spacing * 0.40).clamp(19.0, 26.0);
    (radius, circle, spacing)
}

fn slot_pos(anchor: Pos2, index: usize, count: usize, radius: f32) -> Pos2 {
    let t = if count <= 1 {
        0.5
    } else {
        index as f32 / (count - 1) as f32
    };
    let angle = ARC_START + (ARC_END - ARC_START) * t;
    pos2(
        anchor.x + angle.cos() * radius,
        anchor.y + angle.sin() * radius,
    )
}

fn slot_at(anchor: Pos2, count: usize, pointer: Pos2, screen_width: f32) -> Option<usize> {
    let (radius, _, spacing) = radial_geometry(screen_width, count);
    let reach = (spacing * 0.52).clamp(28.0, 44.0);
    let mut best: Option<(usize, f32)> = None;
    for index in 0..count {
        let distance = slot_pos(anchor, index, count, radius).distance(pointer);
        if distance < reach && best.map(|(_, d)| distance < d).unwrap_or(true) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

pub fn home(ui: &mut Ui, state: &crate::model::AppState, wave: f32, now: f64) -> HomeEvent {
    let rect = ui.available_rect_before_wrap();
    charts::wave(ui, rect, wave, now as f32);

    let buttons = state.nav == NavMode::Buttons;
    let button_offset = if buttons { 120.0 } else { 74.0 };
    let button_center = pos2(rect.center().x, rect.bottom() - button_offset - 34.0);

    // The single number -----------------------------------------------------
    let has_budget = state.monthly_budget > 0;
    let available = state.available_today();
    let text = if has_budget {
        state.currency.format(available)
    } else {
        "Set a budget".to_owned()
    };
    let max_width = (rect.width() - 52.0).max(120.0);
    let start_size = if has_budget { 78.0 } else { 44.0 };
    let galley = fit_text(
        ui.painter(),
        &text,
        max_width,
        start_size,
        if has_budget { 38.0 } else { 30.0 },
        display(),
        WHITE,
    );
    let center_y = rect.center().y - 18.0;
    ui.painter().galley(
        pos2(
            rect.center().x - galley.size().x * 0.5,
            center_y - galley.size().y * 0.5,
        ),
        galley,
        WHITE,
    );
    ui.painter().text(
        pos2(rect.center().x, center_y + 52.0),
        Align2::CENTER_TOP,
        if has_budget {
            "LEFT TO SPEND TODAY"
        } else {
            "TAP TO CHOOSE A MONTHLY BUDGET"
        },
        semibold(10.5),
        alpha(WHITE, 150),
    );

    let mut event = HomeEvent::None;
    if !has_budget {
        let hit =
            Rect::from_center_size(pos2(rect.center().x, center_y + 12.0), vec2(300.0, 120.0));
        let response = ui.interact(hit, Id::new("budget-prompt"), Sense::click());
        if response.clicked() {
            event = HomeEvent::SetBudget;
            tap(state);
        }
    }

    // Hold-to-add -----------------------------------------------------------
    let mut radial = ui
        .ctx()
        .data(|data| data.get_temp::<Radial>(radial_id()))
        .unwrap_or_else(Radial::idle);
    let slots = radial_slots(state);

    let button = Rect::from_center_size(button_center, Vec2::splat(72.0));
    let response = ui.interact(button, Id::new("aiotrack-add"), Sense::click_and_drag());

    if response.is_pointer_button_down_on() && radial.started == 0.0 {
        radial.started = now;
        radial.anchor = ui
            .ctx()
            .input(|input| input.pointer.interact_pos())
            .unwrap_or(button_center);
    }
    if radial.started > 0.0 && !radial.active && now - radial.started > 0.15 {
        radial.active = true;
        radial.anchor = button_center;
        if state.haptics {
            android::haptic(Haptic::LongPress);
        }
    }

    if radial.active {
        let pointer = ui
            .ctx()
            .input(|input| input.pointer.interact_pos())
            .unwrap_or(radial.anchor);
        let selected = slot_at(radial.anchor, slots.len(), pointer, rect.width());
        if selected != radial.selected {
            radial.selected = selected;
            if state.haptics && selected.is_some() {
                android::haptic(Haptic::Tap);
            }
        }
    }

    if ui.ctx().input(|input| input.pointer.any_released()) {
        if radial.active {
            let choice = radial
                .selected
                .and_then(|index| slots.get(index))
                .and_then(|slot| slot.id);
            if radial.selected.is_some() {
                if state.haptics {
                    android::haptic(Haptic::Confirm);
                }
                event = match choice {
                    Some(id) => HomeEvent::Add(id),
                    None => HomeEvent::NewCategory,
                };
            }
            radial = Radial::idle();
        } else if response.clicked() {
            event = HomeEvent::Add(state.last_category);
            tap(state);
            radial = Radial::idle();
        }
        radial.started = 0.0;
    }

    ui.ctx().data_mut(|data| {
        data.insert_temp(radial_id(), radial);
        data.insert_temp(Id::new("aiotrack-radial-open"), radial.active);
    });

    if radial.active {
        paint_radial(ui, &slots, radial, state);
        ui.ctx().request_repaint();
    }

    // The control itself, drawn last so it stays crisp over the veil.
    let pressed = response.is_pointer_button_down_on();
    let radius = 34.0 - if pressed { 2.0 } else { 0.0 };
    if radial.active {
        ui.painter().circle_stroke(
            button_center,
            radius + 5.0,
            Stroke::new(1.0_f32, alpha(WHITE, 90)),
        );
    }
    ui.painter()
        .circle_filled(button_center, radius, Color32::from_rgb(245, 245, 245));
    icons::paint_center(
        ui.painter(),
        Icon::Plus,
        button_center,
        30.0,
        if pressed {
            Color32::from_rgb(40, 40, 40)
        } else {
            BLACK
        },
    );
    if !radial.active && state.transactions.is_empty() {
        ui.painter().text(
            pos2(button_center.x, button_center.y - 58.0),
            Align2::CENTER_BOTTOM,
            "HOLD TO ADD",
            semibold(9.5),
            alpha(WHITE, 95),
        );
    }

    if radial.active || radial.started > 0.0 {
        ui.ctx().request_repaint();
    }
    // Keep the wave drifting even when the screen is completely idle.
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));

    if buttons {
        if let Some(page) = bottom_nav(ui.ctx(), Page::Home) {
            ui.ctx()
                .data_mut(|data| data.insert_temp(Id::new("aiotrack-nav"), page));
        }
    }

    event
}

fn paint_radial(ui: &Ui, slots: &[Slot], radial: Radial, state: &crate::model::AppState) {
    let screen = ui.ctx().screen_rect();
    ui.painter()
        .rect_filled(screen, 0, Color32::from_black_alpha(205));

    let count = slots.len();
    let (radius, circle, _) = radial_geometry(screen.width(), count);
    for (index, slot) in slots.iter().enumerate() {
        let center = slot_pos(radial.anchor, index, count, radius);
        let selected = radial.selected == Some(index);
        let r = if selected { circle + 4.0 } else { circle };
        if selected {
            ui.painter()
                .circle_filled(center, r + 7.0, alpha(WHITE, 26));
            ui.painter().circle_filled(center, r, WHITE);
        } else {
            ui.painter()
                .circle_filled(center, r, Color32::from_rgba_unmultiplied(20, 20, 22, 242));
            ui.painter()
                .circle_stroke(center, r, Stroke::new(1.0_f32, alpha(WHITE, 38)));
        }
        icons::paint_center(
            ui.painter(),
            slot.icon,
            center,
            r * if selected { 1.05 } else { 0.95 },
            if selected { BLACK } else { WHITE },
        );
    }

    // One label for the current choice, above the whole arc.
    let (label, color) = match radial.selected.and_then(|index| slots.get(index)) {
        Some(slot) => (slot.label.as_str(), WHITE),
        None => ("SLIDE TO A CATEGORY", alpha(WHITE, 120)),
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), semibold(12.0), color);
    let pill = Rect::from_center_size(
        pos2(radial.anchor.x, radial.anchor.y - radius - circle - 30.0),
        galley.size() + vec2(34.0, 22.0),
    );
    ui.painter().rect_filled(
        pill,
        CornerRadius::same(16),
        Color32::from_rgba_unmultiplied(18, 18, 20, 245),
    );
    ui.painter().rect_stroke(
        pill,
        CornerRadius::same(16),
        Stroke::new(1.0_f32, alpha(WHITE, 30)),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        pos2(
            pill.center().x - galley.size().x * 0.5,
            pill.center().y - galley.size().y * 0.5,
        ),
        galley,
        color,
    );

    if radial.selected.is_some() {
        ui.painter().text(
            pos2(radial.anchor.x, screen.bottom() - 34.0),
            Align2::CENTER_BOTTOM,
            "RELEASE TO CHOOSE",
            semibold(9.5),
            alpha(WHITE, 130),
        );
    }
    let _ = state;
}

fn tap(state: &crate::model::AppState) {
    if state.haptics {
        android::haptic(Haptic::Tap);
    }
}

fn fit_text(
    painter: &egui::Painter,
    text: &str,
    max_width: f32,
    start: f32,
    floor: f32,
    family: egui::FontFamily,
    color: Color32,
) -> Arc<Galley> {
    let mut size = start;
    loop {
        let galley =
            painter.layout_no_wrap(text.to_owned(), FontId::new(size, family.clone()), color);
        if galley.size().x <= max_width || size <= floor {
            return galley;
        }
        size = (size - 3.0).max(floor);
    }
}

// ---------------------------------------------------------------------------
// Activity
// ---------------------------------------------------------------------------

pub fn ledger(ui: &mut Ui, state: &crate::model::AppState, motion: Motion, now: f64) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "ledger", motion, |ui| {
        widgets::overline(ui, "ACTIVITY");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            widgets::title(ui, "Everything");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::muted(ui, format!("{} entries", state.transactions.len()), 12.0);
            });
        });
        ui.add_space(14.0);

        if state.transactions.is_empty() {
            widgets::empty(
                ui,
                Icon::Activity,
                "Nothing here yet",
                "Hold the + on the home screen to add your first expense.",
            );
        } else {
            let mut last_date = None;
            for tx in state.recent(400) {
                if last_date != Some(tx.date) {
                    if last_date.is_some() {
                        ui.add_space(16.0);
                    }
                    last_date = Some(tx.date);
                    day_header(ui, state, tx.date);
                }
                let category = state.category_name(tx.category);
                let account = state.account_name(tx.account_id);
                let amount = tx_amount(state, tx);
                if widgets::ledger_row(ui, tx, category, account, &amount).clicked() {
                    tap(state);
                    action = Action::EditTransaction(tx.id);
                }
            }
        }
        let _ = now;
    });

    if let Some(page) = bottom_for(ui.ctx(), Page::Ledger, state) {
        action = page_action(page);
    }
    action
}

fn day_header(ui: &mut Ui, state: &crate::model::AppState, date: chrono::NaiveDate) {
    let spent: i64 = state
        .transactions
        .iter()
        .filter(|tx| tx.date == date && tx.kind == TxKind::Expense)
        .map(|tx| tx.amount)
        .sum();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(crate::model::friendly_date(date))
                .font(semibold(12.0))
                .color(ink()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            widgets::muted(ui, state.currency.format_abs(spent), 11.5);
        });
    });
    ui.add_space(2.0);
    widgets::hairline(ui, 0.0);
    ui.add_space(4.0);
}

fn tx_amount(state: &crate::model::AppState, tx: &Transaction) -> String {
    match tx.kind {
        TxKind::Expense => format!("-{}", state.currency.format_abs(tx.amount)),
        TxKind::Income => format!("+{}", state.currency.format_abs(tx.amount)),
    }
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

pub fn analysis(
    ui: &mut Ui,
    state: &crate::model::AppState,
    month: YearMonth,
    motion: Motion,
    chart_progress: f32,
) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "analysis", motion, |ui| {
        // The month switcher spans the full page width: arrows in the two
        // corners, month in the optical centre.
        month_changer(ui, month, &mut action);
        ui.add_space(18.0);

        ui.vertical_centered(|ui| {
            if widgets::text_button(ui, "Advanced analysis  ›", muted()).clicked() {
                tap(state);
                action = Action::OpenAdvanced;
            }
            ui.add_space(14.0);

            let totals = state.totals_for(month);
            widgets::overline(ui, "SPENT THIS MONTH");
            ui.add_space(4.0);
            let total = state.currency.format_abs(totals.spent);
            widgets::big(ui, total, 52.0);
            if totals.income > 0 {
                ui.add_space(2.0);
                widgets::muted(
                    ui,
                    format!("{} came in", state.currency.format_abs(totals.income)),
                    12.0,
                );
            }
            ui.add_space(26.0);

            let slices_data = state.category_totals(month, TxKind::Expense);
            if slices_data.is_empty() {
                widgets::empty(
                    ui,
                    Icon::Insights,
                    "A quiet month",
                    "Add an expense and its shape will appear here.",
                );
            } else {
                let width = ui.available_width().min(250.0);
                let (rect, _) = ui.allocate_exact_size(vec2(width, width), Sense::hover());
                let slices: Vec<(Color32, i64)> = slices_data
                    .iter()
                    .enumerate()
                    .map(|(index, (_, value))| (pie_color(index), *value))
                    .collect();
                charts::pie(ui, rect, &slices, totals.spent.max(1), chart_progress);
                ui.add_space(22.0);

                for (index, (id, value)) in slices_data.iter().enumerate() {
                    let category = state.category(id.0);
                    let name = category.map(|c| c.name.as_str()).unwrap_or("Other");
                    let color = pie_color(index);
                    let share = *value as f32 * 100.0 / totals.spent.max(1) as f32;
                    legend_row(
                        ui,
                        color,
                        name,
                        &format!("{:.0}%", share),
                        &state.currency.format_abs(*value),
                    );
                }
            }
        });
    });

    if let Some(page) = bottom_for(ui.ctx(), Page::Analysis, state) {
        action = page_action(page);
    }
    action
}

fn month_changer(ui: &mut Ui, month: YearMonth, action: &mut Action) {
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if widgets::icon_button(ui, Icon::ChevronLeft, 40.0, ink()).clicked() {
            *action = Action::ChangeMonth(-1);
        }
        let middle = (width - 92.0).max(80.0);
        ui.allocate_ui_with_layout(
            vec2(middle, 56.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(month.short_label())
                        .font(semibold(13.0))
                        .color(muted()),
                );
                ui.label(
                    egui::RichText::new(month.year.to_string())
                        .font(bold(24.0))
                        .color(ink()),
                );
            },
        );
        let future = month.index() >= YearMonth::current().index();
        if !future {
            if widgets::icon_button(ui, Icon::ChevronRight, 40.0, ink()).clicked() {
                *action = Action::ChangeMonth(1);
            }
        } else {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            icons::paint_center(
                ui.painter(),
                Icon::ChevronRight,
                rect.center(),
                17.0,
                alpha(WHITE, 24),
            );
        }
    });
}

fn legend_row(ui: &mut Ui, color: Color32, name: &str, share: &str, amount: &str) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 38.0), Sense::hover());
    ui.painter()
        .circle_filled(pos2(rect.left() + 5.0, rect.center().y), 4.0, color);
    ui.painter().text(
        pos2(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        medium(13.5),
        ink(),
    );
    ui.painter().text(
        pos2(rect.right() - 76.0, rect.center().y),
        Align2::RIGHT_CENTER,
        share,
        sans_font(11.5),
        faint(),
    );
    ui.painter().text(
        pos2(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        amount,
        semibold(13.5),
        ink(),
    );
}

// ---------------------------------------------------------------------------
// Advanced analysis
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TrendBucket {
    label: String,
    slices: Vec<(CategoryId, i64)>,
}

fn trend_color(id: CategoryId) -> Color32 {
    pie_color((id.0 as usize + 9) % 10)
}

fn monthly_trend(state: &crate::model::AppState, count: usize) -> Vec<TrendBucket> {
    let current = YearMonth::current();
    (0..count)
        .rev()
        .map(|i| {
            let month = current.shifted(-(i as i32));
            TrendBucket {
                label: month.short_label().to_owned(),
                slices: state.category_totals(month, TxKind::Expense),
            }
        })
        .collect()
}

fn weekly_trend(state: &crate::model::AppState, count: usize) -> Vec<TrendBucket> {
    let now = crate::model::today();
    let mut buckets: Vec<Vec<(CategoryId, i64)>> = vec![Vec::new(); count];
    for tx in &state.transactions {
        if tx.kind != TxKind::Expense {
            continue;
        }
        let days = (now - tx.date).num_days();
        if days < 0 {
            continue;
        }
        let index = (days / 7) as usize;
        if index >= count {
            continue;
        }
        if let Some(entry) = buckets[index].iter_mut().find(|(id, _)| *id == tx.category) {
            entry.1 += tx.amount;
        } else {
            buckets[index].push((tx.category, tx.amount));
        }
    }
    buckets
        .into_iter()
        .enumerate()
        .map(|(index, slices)| TrendBucket {
            label: format!("W{}", count - index),
            slices,
        })
        .collect()
}

fn stacked_bars(ui: &mut Ui, buckets: &[TrendBucket], height: f32, progress: f32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height + 22.0), Sense::hover());
    let totals: Vec<i64> = buckets
        .iter()
        .map(|bucket| bucket.slices.iter().map(|(_, value)| *value).sum())
        .collect();
    let max = totals.iter().copied().max().unwrap_or(1).max(1);
    let gap = 12.0_f32;
    let bar_width = ((rect.width() - gap * (buckets.len().saturating_sub(1)) as f32)
        / buckets.len().max(1) as f32)
        .max(8.0);
    let plot_height = height - 10.0;

    for (index, bucket) in buckets.iter().enumerate() {
        let x = rect.left() + index as f32 * (bar_width + gap);
        let delay = index as f32 * 0.035;
        let bar_progress = charts::ease_out(((progress - delay) / (1.0 - delay)).clamp(0.0, 1.0));
        let mut top = rect.top() + plot_height;
        for (id, value) in &bucket.slices {
            let slice_height = (*value as f32 / max as f32) * plot_height * bar_progress;
            if slice_height <= 0.2 {
                continue;
            }
            let segment = Rect::from_min_max(pos2(x, top - slice_height), pos2(x + bar_width, top));
            ui.painter()
                .rect_filled(segment, CornerRadius::same(2), trend_color(*id));
            top -= slice_height;
        }
        ui.painter().text(
            pos2(x + bar_width * 0.5, rect.top() + height + 4.0),
            Align2::CENTER_TOP,
            &bucket.label,
            sans_font(10.0),
            faint(),
        );
    }
    ui.painter().line_segment(
        [
            pos2(rect.left(), rect.top() + plot_height),
            pos2(rect.right(), rect.top() + plot_height),
        ],
        Stroke::new(1.0_f32, line_soft()),
    );
}

fn trend_legend(ui: &mut Ui, state: &crate::model::AppState, buckets: &[TrendBucket]) {
    let mut ids: Vec<CategoryId> = Vec::new();
    for bucket in buckets {
        for (id, _) in &bucket.slices {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
    }
    ids.sort_by_key(|id| id.0);
    let width = ui.available_width();
    let cell = (width - 12.0) * 0.5;
    for pair in ids.chunks(2) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for id in pair {
                let (rect, _) = ui.allocate_exact_size(vec2(cell, 30.0), Sense::hover());
                ui.painter().circle_filled(
                    pos2(rect.left() + 5.0, rect.center().y),
                    4.0,
                    trend_color(*id),
                );
                ui.painter().text(
                    pos2(rect.left() + 18.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    state.category_name(*id),
                    medium(12.0),
                    muted(),
                );
            }
        });
    }
}

pub fn advanced(
    ui: &mut Ui,
    state: &crate::model::AppState,
    motion: Motion,
    chart_progress: f32,
) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "advanced", motion, |ui| {
        if widgets::title_row(ui, "Trends", true) {
            action = Action::CloseAdvanced;
        }
        ui.add_space(10.0);
        widgets::muted(
            ui,
            "Where the money went, month by month and week by week. Each bar is split by category.",
            12.5,
        );
        ui.add_space(18.0);

        let monthly = monthly_trend(state, 6);
        widgets::overline(ui, "MONTHLY SPEND");
        ui.add_space(10.0);
        stacked_bars(ui, &monthly, 120.0, chart_progress);

        ui.add_space(20.0);
        let weekly = weekly_trend(state, 8);
        widgets::overline(ui, "WEEKLY SPEND");
        ui.add_space(10.0);
        stacked_bars(ui, &weekly, 120.0, chart_progress);

        ui.add_space(18.0);
        widgets::overline(ui, "CATEGORIES");
        ui.add_space(4.0);
        let mut all = monthly.clone();
        all.extend(weekly.clone());
        trend_legend(ui, state, &all);
    });

    if let Some(page) = bottom_for(ui.ctx(), Page::Analysis, state) {
        action = page_action(page);
    }
    action
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

pub fn settings(
    ui: &mut Ui,
    state: &crate::model::AppState,
    page: SettingsPage,
    motion: Motion,
    now: f64,
) -> Action {
    let mut action = match page {
        SettingsPage::Root => settings_root(ui, state, motion),
        SettingsPage::Navigation => settings_navigation(ui, state, motion),
        SettingsPage::Accounts => settings_accounts(ui, state, motion),
        SettingsPage::Categories => settings_categories(ui, state, motion),
        SettingsPage::Currency => settings_currency(ui, state, motion),
        SettingsPage::Backup => settings_backup(ui, state, motion),
        SettingsPage::About => settings_about(ui, state, motion),
    };
    if let Some(target) = bottom_for(ui.ctx(), Page::Settings, state) {
        action = page_action(target);
    }
    let _ = now;
    action
}

fn settings_root(ui: &mut Ui, state: &crate::model::AppState, motion: Motion) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "settings", motion, |ui| {
        widgets::overline(ui, "AIOtrack");
        ui.add_space(6.0);
        widgets::title(ui, "Settings");
        ui.add_space(18.0);

        if widgets::row(
            ui,
            Icon::Sliders,
            "Navigation",
            Some(state.nav.label()),
            false,
        )
        .clicked()
        {
            tap(state);
            action = Action::Settings(SettingsPage::Navigation);
        }
        if widgets::row(
            ui,
            Icon::Accounts,
            "Accounts",
            Some(&format!("{}", state.accounts.len())),
            false,
        )
        .clicked()
        {
            tap(state);
            action = Action::Settings(SettingsPage::Accounts);
        }
        if widgets::row(
            ui,
            Icon::Tag,
            "Categories",
            Some(&format!("{}", state.categories.len())),
            false,
        )
        .clicked()
        {
            tap(state);
            action = Action::Settings(SettingsPage::Categories);
        }
        if widgets::row(
            ui,
            Icon::Coins,
            "Currency",
            Some(state.currency.code()),
            false,
        )
        .clicked()
        {
            tap(state);
            action = Action::Settings(SettingsPage::Currency);
        }
        if widgets::row(ui, Icon::Database, "Backup & restore", None, false).clicked() {
            tap(state);
            action = Action::Settings(SettingsPage::Backup);
        }
        if widgets::row(ui, Icon::Info, "About", None, false).clicked() {
            tap(state);
            action = Action::Settings(SettingsPage::About);
        }

        ui.add_space(20.0);
        widgets::overline(ui, "TODAY");
        ui.add_space(6.0);
        widgets::muted(
            ui,
            format!(
                "{} a day · {} left · {} days to go",
                state.currency.format_abs(state.available_today()),
                state.currency.format_abs(state.remaining_budget()),
                state.budget_days_left()
            ),
            12.0,
        );
        ui.add_space(8.0);
        if widgets::row(
            ui,
            Icon::Calendar,
            "Budget resets",
            Some(&format!("the {}", ordinal(state.budget_reset_day))),
            false,
        )
        .clicked()
        {
            tap(state);
            action = Action::OpenBudget;
        }
        if widgets::text_button(ui, "Edit budget & reset day", ink()).clicked() {
            tap(state);
            action = Action::OpenBudget;
        }
    });
    action
}

fn settings_navigation(ui: &mut Ui, state: &crate::model::AppState, motion: Motion) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "settings-nav", motion, |ui| {
        if widgets::title_row(ui, "Navigation", true) {
            action = Action::Back;
        }
        ui.add_space(12.0);
        widgets::muted(
            ui,
            "Choose how you move between the four pages. Every page stays reachable either way.",
            12.5,
        );
        ui.add_space(18.0);

        if option_card(
            ui,
            Icon::ArrowRight,
            "Gestures",
            "Swipe up, left or right from the home screen. Swipe down from activity or right from the others to come back.",
            state.nav == NavMode::Gestures,
        ) {
            tap(state);
            action = Action::SetNav(NavMode::Gestures);
        }
        ui.add_space(10.0);
        if option_card(
            ui,
            Icon::ListFilter,
            "Buttons",
            "A quiet bar along the bottom with every page one tap away.",
            state.nav == NavMode::Buttons,
        ) {
            tap(state);
            action = Action::SetNav(NavMode::Buttons);
        }

        ui.add_space(24.0);
        widgets::overline(ui, "FEEDBACK");
        ui.add_space(6.0);
        if widgets::row(
            ui,
            Icon::Sparkle,
            "Haptics",
            Some(if state.haptics { "On" } else { "Off" }),
            false,
        )
        .clicked()
        {
            tap(state);
            action = Action::ToggleHaptics;
        }
    });
    action
}

fn option_card(ui: &mut Ui, icon: Icon, title: &str, body: &str, selected: bool) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 104.0), Sense::click());
    let fill = if selected {
        alpha(WHITE, 12)
    } else if response.hovered() {
        alpha(WHITE, 5)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(20), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(20),
        Stroke::new(1.0_f32, if selected { alpha(WHITE, 110) } else { line() }),
        StrokeKind::Inside,
    );
    icons::paint(
        ui.painter(),
        icon,
        Rect::from_center_size(
            pos2(rect.left() + 34.0, rect.top() + 34.0),
            Vec2::splat(22.0),
        ),
        if selected { WHITE } else { muted() },
        1.6,
    );
    ui.painter().text(
        pos2(rect.left() + 60.0, rect.top() + 26.0),
        Align2::LEFT_TOP,
        title,
        semibold(16.0),
        ink(),
    );
    let galley = ui
        .painter()
        .layout(body.to_owned(), sans_font(12.0), muted(), width - 86.0);
    ui.painter()
        .galley(pos2(rect.left() + 60.0, rect.top() + 50.0), galley, muted());
    if selected {
        icons::paint_center(
            ui.painter(),
            Icon::CheckCircle,
            pos2(rect.right() - 26.0, rect.top() + 28.0),
            18.0,
            WHITE,
        );
    }
    response.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

fn settings_accounts(ui: &mut Ui, state: &crate::model::AppState, motion: Motion) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "settings-accounts", motion, |ui| {
        if widgets::title_row(ui, "Accounts", true) {
            action = Action::Back;
        }
        ui.add_space(10.0);
        widgets::muted(
            ui,
            "Money lives in accounts. Balances follow every entry you add.",
            12.5,
        );
        ui.add_space(18.0);

        for account in &state.accounts {
            if account_row(ui, state, account) {
                tap(state);
                action = Action::EditAccount(account.id);
            }
            ui.add_space(4.0);
        }

        ui.add_space(14.0);
        if widgets::white_button(ui, "Add account", true).clicked() {
            tap(state);
            action = Action::NewAccount;
        }
    });
    action
}

fn account_row(
    ui: &mut Ui,
    state: &crate::model::AppState,
    account: &crate::model::Account,
) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 70.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(16), alpha(WHITE, 5));
    }
    let color = account_color(account.color);
    ui.painter().circle_filled(
        pos2(rect.left() + 22.0, rect.center().y),
        17.0,
        alpha(color, 30),
    );
    icons::paint_center(
        ui.painter(),
        icons::account_kind(account.kind),
        pos2(rect.left() + 22.0, rect.center().y),
        17.0,
        color,
    );
    ui.painter().text(
        pos2(rect.left() + 50.0, rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        &account.name,
        medium(15.0),
        ink(),
    );
    ui.painter().text(
        pos2(rect.left() + 50.0, rect.center().y + 11.0),
        Align2::LEFT_CENTER,
        account.kind.label(),
        sans_font(11.5),
        faint(),
    );
    ui.painter().text(
        pos2(rect.right() - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        state.currency.format(state.balance(account.id)),
        semibold(14.0),
        ink(),
    );
    response.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

fn settings_categories(ui: &mut Ui, state: &crate::model::AppState, motion: Motion) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "settings-categories", motion, |ui| {
        if widgets::title_row(ui, "Categories", true) {
            action = Action::Back;
        }
        ui.add_space(10.0);
        widgets::muted(
            ui,
            "Your most-used categories sit closest to your thumb when you hold the add control.",
            12.5,
        );
        ui.add_space(18.0);

        for kind in [TxKind::Expense, TxKind::Income] {
            widgets::overline(
                ui,
                if kind == TxKind::Expense {
                    "SPENDING"
                } else {
                    "INCOME"
                },
            );
            ui.add_space(4.0);
            for category in state.categories_for(kind) {
                if category_row(ui, state, category) {
                    tap(state);
                    action = Action::EditCategory(category.id);
                }
            }
            ui.add_space(12.0);
        }

        if widgets::white_button(ui, "New category", true).clicked() {
            tap(state);
            action = Action::NewCategory;
        }
    });
    action
}

fn category_row(
    ui: &mut Ui,
    state: &crate::model::AppState,
    category: &crate::model::Category,
) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 54.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(14), alpha(WHITE, 5));
    }
    let color = category_color(category.color);
    ui.painter().circle_filled(
        pos2(rect.left() + 19.0, rect.center().y),
        15.0,
        alpha(color, 26),
    );
    icons::paint_center(
        ui.painter(),
        icons::category(category.icon),
        pos2(rect.left() + 19.0, rect.center().y),
        15.0,
        color,
    );
    ui.painter().text(
        pos2(rect.left() + 44.0, rect.center().y),
        Align2::LEFT_CENTER,
        &category.name,
        medium(14.5),
        ink(),
    );
    let used = state.category_usage(category.id);
    ui.painter().text(
        pos2(rect.right() - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        if used == 1 {
            "1 entry".to_owned()
        } else {
            format!("{used} entries")
        },
        sans_font(11.5),
        faint(),
    );
    response.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

fn settings_currency(ui: &mut Ui, state: &crate::model::AppState, motion: Motion) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "settings-currency", motion, |ui| {
        if widgets::title_row(ui, "Currency", true) {
            action = Action::Back;
        }
        ui.add_space(10.0);
        widgets::muted(
            ui,
            "Amounts are stored as plain numbers and displayed with this symbol.",
            12.5,
        );
        ui.add_space(18.0);

        let width = ui.available_width();
        let cell = (width - 10.0) * 0.5;
        for pair in Currency::ALL.chunks(2) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                for currency in pair {
                    currency_cell(
                        ui,
                        *currency,
                        state.currency == *currency,
                        cell,
                        &mut action,
                    );
                }
            });
        }
    });
    action
}

fn currency_cell(ui: &mut Ui, currency: Currency, selected: bool, width: f32, action: &mut Action) {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 64.0), Sense::click());
    let fill = if selected {
        WHITE
    } else if response.hovered() {
        alpha(WHITE, 8)
    } else {
        alpha(WHITE, 4)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(16), fill);
    ui.painter().text(
        pos2(rect.left() + 16.0, rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        currency.code(),
        semibold(14.0),
        if selected { BLACK } else { ink() },
    );
    ui.painter().text(
        pos2(rect.left() + 16.0, rect.center().y + 10.0),
        Align2::LEFT_CENTER,
        currency.name(),
        sans_font(10.5),
        if selected { alpha(BLACK, 150) } else { faint() },
    );
    ui.painter().text(
        pos2(rect.right() - 16.0, rect.center().y),
        Align2::RIGHT_CENTER,
        currency.symbol().trim(),
        semibold(16.0),
        if selected { BLACK } else { muted() },
    );
    if response.clicked() {
        *action = Action::SetCurrency(currency);
    }
}

fn settings_backup(ui: &mut Ui, state: &crate::model::AppState, motion: Motion) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "settings-backup", motion, |ui| {
        if widgets::title_row(ui, "Backup & restore", true) {
            action = Action::Back;
        }
        ui.add_space(10.0);
        widgets::muted(
            ui,
            "A backup is a single file with every account, category and transaction. It never leaves your device unless you move it yourself.",
            12.5,
        );
        ui.add_space(22.0);

        widgets::overline(ui, "BACKUP");
        ui.add_space(8.0);
        if widgets::white_button(ui, "Create a backup", true).clicked() {
            tap(state);
            action = Action::Backup;
        }
        ui.add_space(10.0);
        if widgets::ghost_button(ui, "Restore the latest backup", true).clicked() {
            tap(state);
            action = Action::Restore;
        }

        ui.add_space(30.0);
        widgets::overline(ui, "DANGER");
        ui.add_space(8.0);
        if widgets::text_button(ui, "Erase all data", bad()).clicked() {
            action = Action::ResetAll;
        }
    });
    action
}

fn settings_about(ui: &mut Ui, _state: &crate::model::AppState, motion: Motion) -> Action {
    let mut action = Action::None;
    widgets::page(ui, "settings-about", motion, |ui| {
        if widgets::title_row(ui, "About", true) {
            action = Action::Back;
        }
        ui.add_space(18.0);
        widgets::big(ui, "AioTrack", 40.0);
        ui.add_space(4.0);
        widgets::muted(
            ui,
            "A calm, private way to see what today actually costs.",
            13.0,
        );
        ui.add_space(24.0);

        widgets::overline(ui, "CREDITS");
        ui.add_space(6.0);
        widgets::row(ui, Icon::User, "Made by Manan Sukhani", None, false);
        ui.add_space(20.0);

        widgets::overline(ui, "LINKS");
        ui.add_space(6.0);
        if widgets::row(ui, Icon::Coins, "Buy me a coffee", Some("Support"), false).clicked() {
            tap(_state);
            action = Action::OpenUrl("https://www.buymeacoffee.com/manansukhani");
        }
        if widgets::row(
            ui,
            Icon::BadgeCheck,
            "GitHub profile",
            Some("@MananSukhani"),
            false,
        )
        .clicked()
        {
            tap(_state);
            action = Action::OpenUrl("https://github.com/MananSukhani");
        }
        if widgets::row(
            ui,
            Icon::Database,
            "Project repository",
            Some("AioTrack"),
            false,
        )
        .clicked()
        {
            tap(_state);
            action = Action::OpenUrl("https://github.com/MananSukhani/AioTrack");
        }

        ui.add_space(26.0);
        widgets::muted(ui, "Version 1.0.0 · Built in Rust", 11.5);
        ui.add_space(6.0);
        widgets::muted(ui, "Your data stays on this device.", 11.5);
    });
    action
}

// ---------------------------------------------------------------------------
// Bottom controls
// ---------------------------------------------------------------------------

fn bottom_for(ctx: &egui::Context, page: Page, state: &crate::model::AppState) -> Option<Page> {
    if state.nav == NavMode::Buttons {
        bottom_nav(ctx, page)
    } else {
        if widgets::bottom_circle(ctx, "home", Icon::Home, 56.0, 72.0) {
            if state.haptics {
                android::haptic(Haptic::Tap);
            }
            Some(Page::Home)
        } else {
            None
        }
    }
}

fn page_action(page: Page) -> Action {
    match page {
        Page::Home => Action::Home,
        Page::Ledger => Action::OpenLedger,
        Page::Analysis => Action::OpenAnalysis,
        Page::Settings => Action::OpenSettings,
    }
}

/// A quiet four-slot bar for people who prefer buttons to gestures.
pub fn bottom_nav(ctx: &egui::Context, current: Page) -> Option<Page> {
    let mut chosen = None;
    egui::Area::new(Id::new("aiotrack-bottom-nav"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, 0.0))
        .interactable(true)
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            let width = screen.width();
            let (rect, _) = ui.allocate_exact_size(vec2(width, 66.0), Sense::hover());
            ui.painter().rect_filled(rect, 0, BLACK);
            ui.painter().rect_filled(
                Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 1.0)),
                0,
                line_soft(),
            );
            let entries = [
                (Page::Home, Icon::Home, "Today"),
                (Page::Ledger, Icon::Activity, "Activity"),
                (Page::Analysis, Icon::Insights, "Analysis"),
                (Page::Settings, Icon::Settings, "Settings"),
            ];
            let slot = width / 4.0;
            for (index, (page, icon, label)) in entries.iter().enumerate() {
                let center = pos2(rect.left() + slot * (index as f32 + 0.5), rect.top() + 25.0);
                let hit = Rect::from_center_size(center, vec2(slot, 58.0));
                let response = ui.interact(hit, Id::new(("nav", index)), Sense::click());
                let active = *page == current;
                let color = if active {
                    WHITE
                } else if response.hovered() {
                    muted()
                } else {
                    faint()
                };
                icons::paint_center(ui.painter(), *icon, center, 21.0, color);
                ui.painter().text(
                    pos2(center.x, center.y + 17.0),
                    Align2::CENTER_TOP,
                    *label,
                    if active {
                        semibold(9.5)
                    } else {
                        sans_font(9.5)
                    },
                    color,
                );
                if response.clicked() {
                    chosen = Some(*page);
                }
            }
        });
    chosen
}

/// Reads and clears a bottom-navigation choice made inside the home surface.
pub fn take_nav(ctx: &egui::Context) -> Option<Page> {
    ctx.data_mut(|data| {
        let key = Id::new("aiotrack-nav");
        let value = data.get_temp::<Page>(key);
        data.remove::<Page>(key);
        value
    })
}
