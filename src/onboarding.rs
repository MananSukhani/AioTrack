//! First-run setup and the short gesture tutorial.
//!
//! Four questions, then three quiet cards that teach the gesture surface.
//! Everything uses the system keyboard, so the flow feels native on Android.

use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Id, Sense, Stroke, StrokeKind, Ui};

use crate::android;
use crate::charts::ease_out;
use crate::icons::{self, Icon};
use crate::model::{parse_amount, AccountKind, Currency, Minor};
use crate::theme::*;
use crate::widgets;

#[derive(Clone, Debug)]
pub struct OnboardingState {
    pub step: usize,
    pub currency: Currency,
    pub budget_raw: String,
    pub reset_day: u8,
    pub account_name: String,
    pub opening_raw: String,
    pub kind: AccountKind,
    pub tutorial: usize,
    pub entered_at: f64,
    focus_budget: bool,
    focus_account: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnboardingEvent {
    None,
    Back,
    Next,
    Finish,
}

impl OnboardingState {
    pub fn new(currency: Currency, now: f64) -> Self {
        Self {
            step: 0,
            currency,
            budget_raw: String::new(),
            reset_day: 1,
            account_name: "Cash".to_owned(),
            opening_raw: String::new(),
            kind: AccountKind::Cash,
            tutorial: 0,
            entered_at: now,
            focus_budget: true,
            focus_account: true,
        }
    }

    pub fn advance(&mut self, now: f64) {
        self.step = (self.step + 1).min(4);
        if self.step == 4 {
            self.tutorial = 0;
        }
        self.entered_at = now;
        if self.step == 1 {
            self.focus_budget = true;
        }
        if self.step == 3 {
            self.focus_account = true;
        }
    }

    pub fn back(&mut self, now: f64) {
        if self.step == 4 && self.tutorial > 0 {
            self.tutorial -= 1;
            self.entered_at = now;
        } else if self.step > 0 {
            self.step -= 1;
            self.entered_at = now;
        }
    }

    pub fn budget(&self) -> Option<Minor> {
        parse_amount(&self.budget_raw, self.currency.decimals())
    }

    pub fn opening(&self) -> Option<Minor> {
        if self.opening_raw.trim().is_empty() {
            Some(0)
        } else {
            parse_amount(&self.opening_raw, self.currency.decimals())
        }
    }

    pub fn named(&self) -> bool {
        !self.account_name.trim().is_empty()
    }
}

pub fn render(ui: &mut Ui, flow: &mut OnboardingState, now: f64) -> OnboardingEvent {
    let mut event = OnboardingEvent::None;
    let screen = ui.available_rect_before_wrap();
    ui.painter().rect_filled(screen, 0, BLACK);

    let progress = ((now - flow.entered_at) as f32 / 0.30).clamp(0.0, 1.0);
    let ease = ease_out(progress);
    ui.set_opacity(0.15 + 0.85 * ease);
    let lift = (1.0 - ease) * 18.0;

    widgets::column(ui, |ui| {
        ui.add_space(46.0 + lift);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("AIOtrack")
                    .font(semibold(11.0))
                    .color(faint()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if flow.step > 0 {
                    if widgets::text_button(ui, "Back", muted()).clicked() {
                        event = OnboardingEvent::Back;
                    }
                }
            });
        });
        ui.add_space(24.0);

        match flow.step {
            0 => welcome(ui, flow, &mut event),
            1 => budget(ui, flow, &mut event),
            2 => currency(ui, flow, &mut event),
            3 => account(ui, flow, &mut event),
            _ => tutorial(ui, flow, &mut event),
        }
    });

    if progress < 1.0 {
        ui.ctx().request_repaint();
    }
    event
}

fn welcome(ui: &mut Ui, flow: &mut OnboardingState, event: &mut OnboardingEvent) {
    ui.add_space(50.0);
    ui.label(
        egui::RichText::new("Know what\ntoday costs.")
            .font(display_font(48.0))
            .color(WHITE)
            .line_height(Some(52.0)),
    );
    ui.add_space(20.0);
    ui.label(
        egui::RichText::new(
            "One number, one gesture, no clutter. AioTrack turns your monthly budget into a daily allowance and shows it on a quiet black canvas.",
        )
        .font(sans_font(14.0))
        .color(muted())
        .line_height(Some(21.0)),
    );
    ui.add_space(40.0);
    if widgets::white_button(ui, "Begin", true).clicked() {
        *event = OnboardingEvent::Next;
    }
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new("Private by design · everything stays on this device")
            .font(sans_font(10.5))
            .color(faint()),
    );
    let _ = flow;
}

fn budget(ui: &mut Ui, flow: &mut OnboardingState, event: &mut OnboardingEvent) {
    widgets::overline(ui, "STEP 1 OF 3");
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Set a monthly\nbudget.")
            .font(display_font(38.0))
            .color(WHITE)
            .line_height(Some(42.0)),
    );
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new(
            "We use what's left of your budget and spread it over the days left in your cycle. Choose when the cycle starts again.",
        )
        .font(sans_font(13.0))
        .color(muted())
        .line_height(Some(19.0)),
    );
    ui.add_space(30.0);
    widgets::overline(ui, "AMOUNT");
    let id = Id::new("onboarding-budget");
    let response = widgets::field(
        ui,
        "onboarding-budget",
        &mut flow.budget_raw,
        "0",
        42.0,
        true,
        false,
    );
    if flow.focus_budget {
        ui.memory_mut(|memory| memory.request_focus(id));
        android::set_numeric_ime();
        flow.focus_budget = false;
    }
    if response.gained_focus() {
        android::set_numeric_ime();
    }
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(flow.currency.code())
            .font(semibold(10.5))
            .color(faint()),
    );

    ui.add_space(18.0);
    if crate::sheets::reset_day_row(ui, &mut flow.reset_day) {
        android::haptic(crate::android::Haptic::Tap);
    }

    ui.add_space(26.0);
    let valid = flow.budget().map(|value| value > 0).unwrap_or(false);
    if widgets::white_button(ui, "Continue", valid).clicked() && valid {
        android::hide_keyboard();
        *event = OnboardingEvent::Next;
    }
    ui.add_space(10.0);
    if widgets::text_button(ui, "Skip for now", muted()).clicked() {
        android::hide_keyboard();
        *event = OnboardingEvent::Next;
    }
}

fn currency(ui: &mut Ui, flow: &mut OnboardingState, event: &mut OnboardingEvent) {
    widgets::overline(ui, "STEP 2 OF 3");
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Choose a\ncurrency.")
            .font(display_font(38.0))
            .color(WHITE)
            .line_height(Some(42.0)),
    );
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("You can change this later in Settings.")
            .font(sans_font(13.0))
            .color(muted()),
    );
    ui.add_space(24.0);

    let width = ui.available_width();
    let cell = (width - 10.0) * 0.5;
    for pair in Currency::ALL.chunks(2) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for currency in pair {
                currency_cell(ui, *currency, flow.currency == *currency, cell, flow);
            }
        });
        ui.add_space(8.0);
    }

    ui.add_space(18.0);
    if widgets::white_button(ui, "Continue", true).clicked() {
        *event = OnboardingEvent::Next;
    }
}

fn currency_cell(
    ui: &mut Ui,
    currency: Currency,
    selected: bool,
    width: f32,
    flow: &mut OnboardingState,
) {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 58.0), Sense::click());
    let fill = if selected {
        WHITE
    } else if response.hovered() {
        alpha(WHITE, 8)
    } else {
        alpha(WHITE, 4)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(15), fill);
    ui.painter().text(
        pos2(rect.left() + 14.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        currency.code(),
        semibold(13.5),
        if selected { BLACK } else { ink() },
    );
    ui.painter().text(
        pos2(rect.left() + 14.0, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        currency.name(),
        sans_font(10.0),
        if selected { alpha(BLACK, 150) } else { faint() },
    );
    if response.clicked() {
        flow.currency = currency;
        android::haptic(crate::android::Haptic::Tap);
    }
}

fn account(ui: &mut Ui, flow: &mut OnboardingState, event: &mut OnboardingEvent) {
    widgets::overline(ui, "STEP 3 OF 3");
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Where does your\nmoney live?")
            .font(display_font(36.0))
            .color(WHITE)
            .line_height(Some(40.0)),
    );
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("Add a starting account. More can follow any time.")
            .font(sans_font(13.0))
            .color(muted()),
    );
    ui.add_space(24.0);

    widgets::overline(ui, "NAME");
    let name_id = Id::new("onboarding-account");
    let name = widgets::field(
        ui,
        "onboarding-account",
        &mut flow.account_name,
        "Cash",
        24.0,
        false,
        false,
    );
    if flow.focus_account {
        ui.memory_mut(|memory| memory.request_focus(name_id));
        android::set_text_ime();
        flow.focus_account = false;
    }
    if name.gained_focus() {
        android::set_text_ime();
    }

    ui.add_space(18.0);
    widgets::overline(ui, "TYPE");
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for kind in AccountKind::ALL {
            if kind_chip(ui, kind.label(), flow.kind == kind).clicked() {
                flow.kind = kind;
            }
        }
    });

    ui.add_space(18.0);
    widgets::overline(ui, "OPENING BALANCE");
    let opening = widgets::field(
        ui,
        "onboarding-opening",
        &mut flow.opening_raw,
        "0",
        24.0,
        true,
        false,
    );
    if opening.gained_focus() {
        android::set_numeric_ime();
    }
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(flow.currency.code())
            .font(semibold(10.5))
            .color(faint()),
    );

    ui.add_space(30.0);
    if widgets::white_button(ui, "Continue", flow.named()).clicked() && flow.named() {
        android::hide_keyboard();
        *event = OnboardingEvent::Next;
    }
    ui.add_space(10.0);
    if widgets::text_button(ui, "Use a simple Cash account", muted()).clicked() {
        android::hide_keyboard();
        flow.account_name = "Cash".to_owned();
        flow.opening_raw.clear();
        *event = OnboardingEvent::Next;
    }
}

fn kind_chip(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), medium(12.5), ink());
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(28.0, 18.0), Sense::click());
    let fill = if selected {
        alpha(WHITE, 16)
    } else if response.hovered() {
        alpha(WHITE, 7)
    } else {
        alpha(WHITE, 4)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(14), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(14),
        Stroke::new(1.0_f32, if selected { alpha(WHITE, 100) } else { line() }),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        rect.center() - galley.size() * 0.5,
        galley,
        if selected { ink() } else { muted() },
    );
    response
}

struct TutorialCard {
    icon: Icon,
    title: &'static str,
    body: &'static str,
}

const TUTORIAL: [TutorialCard; 3] = [
    TutorialCard {
        icon: Icon::Plus,
        title: "Hold to add",
        body: "Press and hold the + on the home screen. Your most-used categories appear around your thumb. Slide to one and let go.",
    },
    TutorialCard {
        icon: Icon::ArrowRight,
        title: "Swipe to move",
        body: "From home, swipe up for activity, right for analysis and left for settings. Swipe down from activity, or right from the others, to come back.",
    },
    TutorialCard {
        icon: Icon::TrendUp,
        title: "Watch the wave",
        body: "The wave rises as you spend the day's allowance. Green is calm, orange is close, red means today's budget is gone.",
    },
];

fn tutorial(ui: &mut Ui, flow: &mut OnboardingState, event: &mut OnboardingEvent) {
    let card = &TUTORIAL[flow.tutorial.min(TUTORIAL.len() - 1)];
    widgets::overline(ui, "A QUICK TOUR");
    ui.add_space(26.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 92.0), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 40.0, alpha(WHITE, 8));
    icons::paint_center(ui.painter(), card.icon, rect.center(), 38.0, WHITE);

    ui.add_space(20.0);
    ui.label(
        egui::RichText::new(card.title)
            .font(display_font(36.0))
            .color(WHITE),
    );
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new(card.body)
            .font(sans_font(14.0))
            .color(muted())
            .line_height(Some(21.0)),
    );

    ui.add_space(30.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 7.0;
        for index in 0..TUTORIAL.len() {
            let (dot, _) = ui.allocate_exact_size(vec2(18.0, 4.0), Sense::hover());
            ui.painter().rect_filled(
                dot,
                CornerRadius::same(2),
                if index == flow.tutorial {
                    WHITE
                } else {
                    alpha(WHITE, 24)
                },
            );
        }
    });

    ui.add_space(34.0);
    let last = flow.tutorial + 1 >= TUTORIAL.len();
    if widgets::white_button(ui, if last { "Start tracking" } else { "Next" }, true).clicked() {
        if last {
            *event = OnboardingEvent::Finish;
        } else {
            flow.tutorial += 1;
            flow.entered_at = ui.ctx().input(|input| input.time);
        }
    }
    if flow.tutorial > 0 {
        ui.add_space(8.0);
        if widgets::text_button(ui, "Previous", muted()).clicked() {
            flow.tutorial -= 1;
            flow.entered_at = ui.ctx().input(|input| input.time);
        }
    }
}
