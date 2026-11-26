//! Focused sheets: the transaction composer, account and category editors,
//! the budget editor and the single confirmation dialog.
//!
//! Every text field is a normal egui field, which means the Android system
//! keyboard is used in the right mode: a number pad for amounts, a text pad
//! for names. There is no in-app keyboard.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, Id, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};

use crate::android::{self, Haptic};
use crate::icons::{self, Icon};
use crate::model::{
    now_minutes, ordinal, parse_amount, today, Account, AccountKind, AppState, Category,
    CategoryIcon, CategoryId, Currency, Minor, Transaction, TxKind,
};
use crate::theme::*;
use crate::widgets::{self, Motion};

// ---------------------------------------------------------------------------
// Transaction composer
// ---------------------------------------------------------------------------

pub struct Composer {
    pub editing: Option<u64>,
    pub kind: TxKind,
    pub category: CategoryId,
    pub account_id: u64,
    pub amount_raw: String,
    pub note: String,
    pub date_raw: String,
    pub time_raw: String,
    pub error: Option<String>,
    pub picking: bool,
    pub details: bool,
    focus_amount: bool,
}

impl Composer {
    pub fn new(state: &AppState, category: CategoryId) -> Self {
        let account_id = if state.account(state.last_account).is_some() {
            state.last_account
        } else {
            state.accounts.first().map(|a| a.id).unwrap_or(1)
        };
        Self {
            editing: None,
            kind: TxKind::Expense,
            category,
            account_id,
            amount_raw: String::new(),
            note: String::new(),
            date_raw: today().format("%Y-%m-%d").to_string(),
            time_raw: crate::model::format_minutes(now_minutes()),
            error: None,
            picking: false,
            details: false,
            focus_amount: true,
        }
    }

    pub fn edit(tx: &Transaction, state: &AppState) -> Self {
        Self {
            editing: Some(tx.id),
            kind: tx.kind,
            category: tx.category,
            account_id: tx.account_id,
            amount_raw: state.currency.plain(tx.amount.unsigned_abs()),
            note: tx.note.clone(),
            date_raw: tx.date.format("%Y-%m-%d").to_string(),
            time_raw: crate::model::format_minutes(crate::model::transaction_minutes(tx)),
            error: None,
            picking: false,
            details: true,
            focus_amount: false,
        }
    }

    pub fn amount(&self, currency: Currency) -> Option<Minor> {
        parse_amount(&self.amount_raw, currency.decimals())
    }

    pub fn date(&self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::parse_from_str(self.date_raw.trim(), "%Y-%m-%d").ok()
    }

    pub fn time(&self) -> Option<u32> {
        crate::model::parse_minutes(&self.time_raw)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComposerAction {
    None,
    Close,
    Save,
    Delete,
    NewCategory,
}

pub fn render_composer(ui: &mut Ui, form: &mut Composer, state: &AppState) -> ComposerAction {
    let mut action = ComposerAction::None;
    widgets::sheet_page(ui, "composer", Motion::still(), |ui| {
        ui.horizontal(|ui| {
            if widgets::icon_button(ui, Icon::Close, 42.0, ink()).clicked() {
                android::hide_keyboard();
                action = ComposerAction::Close;
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(if form.editing.is_some() {
                    "Edit entry"
                } else {
                    "New entry"
                })
                .font(semibold(18.0))
                .color(ink()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::text_button(ui, "Save", ink()).clicked() {
                    android::hide_keyboard();
                    action = ComposerAction::Save;
                }
            });
        });

        ui.add_space(26.0);

        // Amount ------------------------------------------------------------
        let amount_id = Id::new(("composer-amount", form.editing));
        let response = widgets::field(
            ui,
            ("composer-amount", form.editing),
            &mut form.amount_raw,
            "0",
            54.0,
            true,
            true,
        );
        if form.focus_amount {
            ui.memory_mut(|memory| memory.request_focus(amount_id));
            android::set_numeric_ime();
            form.focus_amount = false;
        }
        if response.gained_focus() {
            android::set_numeric_ime();
        }
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(state.currency.code())
                    .font(semibold(10.0))
                    .color(faint()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::text_button(
                    ui,
                    if form.kind == TxKind::Expense {
                        "Expense"
                    } else {
                        "Income"
                    },
                    muted(),
                )
                .clicked()
                {
                    form.kind = match form.kind {
                        TxKind::Expense => TxKind::Income,
                        TxKind::Income => TxKind::Expense,
                    };
                    let fallback = state
                        .sorted_categories(form.kind)
                        .first()
                        .map(|category| CategoryId(category.id));
                    if let Some(id) = fallback {
                        form.category = id;
                    }
                    tap(state);
                }
            });
        });

        ui.add_space(18.0);

        // Category ----------------------------------------------------------
        let selected = state.category(form.category.0);
        let (chip_rect, chip_response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
        if chip_response.hovered() {
            ui.painter()
                .rect_filled(chip_rect, CornerRadius::same(16), alpha(WHITE, 6));
        }
        ui.painter().rect_stroke(
            chip_rect,
            CornerRadius::same(16),
            Stroke::new(1.0_f32, line()),
            StrokeKind::Inside,
        );
        if let Some(category) = selected {
            let color = category_color(category.color);
            ui.painter().circle_filled(
                pos2(chip_rect.left() + 28.0, chip_rect.center().y),
                17.0,
                alpha(color, 28),
            );
            icons::paint_center(
                ui.painter(),
                icons::category(category.icon),
                pos2(chip_rect.left() + 28.0, chip_rect.center().y),
                17.0,
                color,
            );
            ui.painter().text(
                pos2(chip_rect.left() + 54.0, chip_rect.center().y - 9.0),
                Align2::LEFT_CENTER,
                &category.name,
                medium(15.0),
                ink(),
            );
            ui.painter().text(
                pos2(chip_rect.left() + 54.0, chip_rect.center().y + 11.0),
                Align2::LEFT_CENTER,
                "Tap to change",
                sans_font(11.0),
                faint(),
            );
        } else {
            ui.painter().text(
                chip_rect.center(),
                Align2::CENTER_CENTER,
                "Choose a category",
                medium(14.0),
                muted(),
            );
        }
        icons::paint_center(
            ui.painter(),
            Icon::ChevronRight,
            pos2(chip_rect.right() - 22.0, chip_rect.center().y),
            15.0,
            faint(),
        );
        if chip_response.clicked() {
            form.picking = !form.picking;
            tap(state);
        }

        if form.picking {
            ui.add_space(10.0);
            category_grid(ui, form, state);
        }

        ui.add_space(14.0);
        if widgets::text_button(
            ui,
            if form.details {
                "Hide details"
            } else {
                "Add name, date or account"
            },
            muted(),
        )
        .clicked()
        {
            form.details = !form.details;
            tap(state);
        }

        if form.details {
            ui.add_space(14.0);
            widgets::overline(ui, "NAME");
            let note_id = Id::new(("composer-note", form.editing));
            let note = widgets::field(
                ui,
                ("composer-note", form.editing),
                &mut form.note,
                "Optional",
                17.0,
                false,
                false,
            );
            if note.gained_focus() {
                android::set_text_ime();
            }
            let _ = note_id;

            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 12.0) * 0.5);
                    widgets::overline(ui, "DATE");
                    let date = widgets::field(
                        ui,
                        ("composer-date", form.editing),
                        &mut form.date_raw,
                        "YYYY-MM-DD",
                        15.0,
                        false,
                        false,
                    );
                    if date.gained_focus() {
                        android::set_text_ime();
                    }
                });
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 12.0) * 0.5);
                    widgets::overline(ui, "TIME");
                    let time = widgets::field(
                        ui,
                        ("composer-time", form.editing),
                        &mut form.time_raw,
                        "HH:MM",
                        15.0,
                        false,
                        false,
                    );
                    if time.gained_focus() {
                        android::set_text_ime();
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if small_chip(
                    ui,
                    "Today",
                    form.date_raw == today().format("%Y-%m-%d").to_string(),
                )
                .clicked()
                {
                    form.date_raw = today().format("%Y-%m-%d").to_string();
                    tap(state);
                }
                if small_chip(
                    ui,
                    "Yesterday",
                    form.date_raw
                        == (today() - chrono::Duration::days(1))
                            .format("%Y-%m-%d")
                            .to_string(),
                )
                .clicked()
                {
                    form.date_raw = (today() - chrono::Duration::days(1))
                        .format("%Y-%m-%d")
                        .to_string();
                    tap(state);
                }
                if small_chip(ui, "Now", false).clicked() {
                    form.time_raw = crate::model::format_minutes(now_minutes());
                    tap(state);
                }
            });

            ui.add_space(18.0);
            widgets::overline(ui, "ACCOUNT");
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for account in &state.accounts {
                    if account_chip(ui, account, form.account_id == account.id).clicked() {
                        form.account_id = account.id;
                        tap(state);
                    }
                }
            });
        }

        if let Some(error) = &form.error {
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new(error)
                    .font(sans_font(12.0))
                    .color(bad()),
            );
        }

        ui.add_space(26.0);
        if widgets::white_button(ui, "Save entry", true).clicked() {
            android::hide_keyboard();
            action = ComposerAction::Save;
        }
        if form.editing.is_some() {
            ui.add_space(10.0);
            if widgets::text_button(ui, "Delete entry", bad()).clicked() {
                action = ComposerAction::Delete;
            }
        }
    });
    action
}

fn category_grid(ui: &mut Ui, form: &mut Composer, state: &AppState) {
    let categories: Vec<&Category> = state.categories_for(form.kind);
    let width = ui.available_width();
    let cell = (width - 16.0) / 3.0;
    let mut index = 0;
    while index < categories.len() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for _ in 0..3 {
                if let Some(category) = categories.get(index) {
                    let selected = form.category == CategoryId(category.id);
                    if pick_cell(ui, category, selected, cell) {
                        form.category = CategoryId(category.id);
                        form.picking = false;
                        tap(state);
                    }
                }
                index += 1;
            }
        });
        ui.add_space(8.0);
    }
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(vec2(width, 48.0), Sense::click());
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(14),
            Stroke::new(1.0_f32, line()),
            StrokeKind::Inside,
        );
        icons::paint_center(
            ui.painter(),
            Icon::Plus,
            pos2(rect.left() + 24.0, rect.center().y),
            15.0,
            muted(),
        );
        ui.painter().text(
            pos2(rect.left() + 42.0, rect.center().y),
            Align2::LEFT_CENTER,
            "Create a category",
            medium(13.0),
            muted(),
        );
        if response.clicked() {
            ui.ctx()
                .data_mut(|data| data.insert_temp(Id::new("composer-new-category"), true));
        }
    });
}

fn pick_cell(ui: &mut Ui, category: &Category, selected: bool, width: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 78.0), Sense::click());
    let fill = if selected {
        alpha(WHITE, 14)
    } else if response.hovered() {
        alpha(WHITE, 6)
    } else {
        alpha(WHITE, 3)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(16), fill);
    if selected {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(16),
            Stroke::new(1.0_f32, alpha(WHITE, 120)),
            StrokeKind::Inside,
        );
    }
    let color = if selected {
        WHITE
    } else {
        category_color(category.color)
    };
    icons::paint_center(
        ui.painter(),
        icons::category(category.icon),
        pos2(rect.center().x, rect.top() + 27.0),
        20.0,
        color,
    );
    ui.painter().text(
        pos2(rect.center().x, rect.top() + 46.0),
        Align2::CENTER_TOP,
        &category.name,
        medium(11.0),
        if selected { WHITE } else { muted() },
    );
    response.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

fn account_chip(ui: &mut Ui, account: &Account, selected: bool) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(account.name.clone(), medium(12.5), ink());
    let width = galley.size().x + 56.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 38.0), Sense::click());
    let fill = if selected {
        alpha(WHITE, 18)
    } else if response.hovered() {
        alpha(WHITE, 7)
    } else {
        alpha(WHITE, 4)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(19), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(19),
        Stroke::new(1.0_f32, if selected { alpha(WHITE, 100) } else { line() }),
        StrokeKind::Inside,
    );
    icons::paint_center(
        ui.painter(),
        icons::account_kind(account.kind),
        pos2(rect.left() + 20.0, rect.center().y),
        14.0,
        if selected { WHITE } else { muted() },
    );
    ui.painter().galley(
        pos2(rect.left() + 34.0, rect.center().y - galley.size().y * 0.5),
        galley,
        if selected { WHITE } else { muted() },
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

fn small_chip(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), medium(11.5), ink());
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(24.0, 14.0), Sense::click());
    let fill = if selected {
        alpha(WHITE, 16)
    } else if response.hovered() {
        alpha(WHITE, 7)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    ui.painter().galley(
        rect.center() - galley.size() * 0.5,
        galley,
        if selected { ink() } else { muted() },
    );
    response
}

fn tap(state: &AppState) {
    if state.haptics {
        android::haptic(Haptic::Tap);
    }
}

/// The composer can ask for a brand new category while it stays open.
pub fn take_new_category_request(ctx: &egui::Context) -> bool {
    ctx.data_mut(|data| {
        let key = Id::new("composer-new-category");
        let value = data.get_temp::<bool>(key).unwrap_or(false);
        data.remove::<bool>(key);
        value
    })
}

// ---------------------------------------------------------------------------
// Account editor
// ---------------------------------------------------------------------------

pub struct AccountForm {
    pub editing: Option<u64>,
    pub name: String,
    pub kind: AccountKind,
    pub color: u8,
    pub opening_raw: String,
    pub error: Option<String>,
    focus_name: bool,
}

impl AccountForm {
    pub fn new(_currency: Currency) -> Self {
        Self {
            editing: None,
            name: String::new(),
            kind: AccountKind::Cash,
            color: 0,
            opening_raw: "0".to_owned(),
            error: None,
            focus_name: true,
        }
    }

    pub fn edit(account: &Account, currency: Currency) -> Self {
        Self {
            editing: Some(account.id),
            name: account.name.clone(),
            kind: account.kind,
            color: account.color,
            opening_raw: currency.plain(account.opening.unsigned_abs()),
            error: None,
            focus_name: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccountAction {
    None,
    Close,
    Save,
    Delete,
}

pub fn render_account_editor(
    ui: &mut Ui,
    form: &mut AccountForm,
    currency: Currency,
) -> AccountAction {
    let mut action = AccountAction::None;
    widgets::sheet_page(ui, "account-editor", Motion::still(), |ui| {
        ui.horizontal(|ui| {
            if widgets::icon_button(ui, Icon::Close, 42.0, ink()).clicked() {
                android::hide_keyboard();
                action = AccountAction::Close;
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(if form.editing.is_some() {
                    "Edit account"
                } else {
                    "New account"
                })
                .font(semibold(18.0))
                .color(ink()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::text_button(ui, "Save", ink()).clicked() {
                    android::hide_keyboard();
                    action = AccountAction::Save;
                }
            });
        });
        ui.add_space(24.0);

        widgets::overline(ui, "NAME");
        let name_id = Id::new(("account-name", form.editing));
        let name = widgets::field(
            ui,
            ("account-name", form.editing),
            &mut form.name,
            "Everyday cash",
            24.0,
            false,
            false,
        );
        if form.focus_name {
            ui.memory_mut(|memory| memory.request_focus(name_id));
            android::set_text_ime();
            form.focus_name = false;
        }
        if name.gained_focus() {
            android::set_text_ime();
        }

        ui.add_space(22.0);
        widgets::overline(ui, "TYPE");
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for kind in AccountKind::ALL {
                if type_chip(ui, kind.label(), form.kind == kind).clicked() {
                    form.kind = kind;
                    form.color = match kind {
                        AccountKind::Cash => 0,
                        AccountKind::Bank => 1,
                        AccountKind::Card => 2,
                        AccountKind::Savings => 3,
                    };
                }
            }
        });

        ui.add_space(22.0);
        widgets::overline(ui, "COLOUR");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for index in 0..6u8 {
                let color = account_color(index);
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
                ui.painter().circle_filled(rect.center(), 14.0, color);
                if form.color == index {
                    ui.painter()
                        .circle_stroke(rect.center(), 17.0, Stroke::new(1.5_f32, WHITE));
                }
                if response.clicked() {
                    form.color = index;
                }
            }
        });

        ui.add_space(22.0);
        widgets::overline(ui, "OPENING BALANCE");
        let opening = widgets::field(
            ui,
            ("account-opening", form.editing),
            &mut form.opening_raw,
            "0",
            26.0,
            true,
            false,
        );
        if opening.gained_focus() {
            android::set_numeric_ime();
        }
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(format!("In {}", currency.code()))
                .font(sans_font(11.0))
                .color(faint()),
        );

        if let Some(error) = &form.error {
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new(error)
                    .font(sans_font(12.0))
                    .color(bad()),
            );
        }

        ui.add_space(26.0);
        if widgets::white_button(ui, "Save account", true).clicked() {
            android::hide_keyboard();
            action = AccountAction::Save;
        }
        if form.editing.is_some() {
            ui.add_space(10.0);
            if widgets::text_button(ui, "Delete account", bad()).clicked() {
                action = AccountAction::Delete;
            }
        }
    });
    action
}

fn type_chip(ui: &mut Ui, label: &str, selected: bool) -> Response {
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

// ---------------------------------------------------------------------------
// Category editor
// ---------------------------------------------------------------------------

pub struct CategoryForm {
    pub editing: Option<u64>,
    pub name: String,
    pub icon: CategoryIcon,
    pub kind: TxKind,
    pub error: Option<String>,
    focus_name: bool,
}

impl CategoryForm {
    pub fn new(kind: TxKind) -> Self {
        Self {
            editing: None,
            name: String::new(),
            icon: CategoryIcon::Tag,
            kind,
            error: None,
            focus_name: true,
        }
    }

    pub fn edit(category: &Category) -> Self {
        Self {
            editing: Some(category.id),
            name: category.name.clone(),
            icon: category.icon,
            kind: category.kind,
            error: None,
            focus_name: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CategoryAction {
    None,
    Close,
    Save,
    Delete,
}

pub fn render_category_editor(
    ui: &mut Ui,
    form: &mut CategoryForm,
    state: &AppState,
) -> CategoryAction {
    let mut action = CategoryAction::None;
    widgets::sheet_page(ui, "category-editor", Motion::still(), |ui| {
        ui.horizontal(|ui| {
            if widgets::icon_button(ui, Icon::Close, 42.0, ink()).clicked() {
                android::hide_keyboard();
                action = CategoryAction::Close;
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(if form.editing.is_some() {
                    "Edit category"
                } else {
                    "New category"
                })
                .font(semibold(18.0))
                .color(ink()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::text_button(ui, "Save", ink()).clicked() {
                    android::hide_keyboard();
                    action = CategoryAction::Save;
                }
            });
        });
        ui.add_space(24.0);

        widgets::overline(ui, "NAME");
        let name_id = Id::new(("category-name", form.editing));
        let name = widgets::field(
            ui,
            ("category-name", form.editing),
            &mut form.name,
            "Coffee",
            24.0,
            false,
            false,
        );
        if form.focus_name {
            ui.memory_mut(|memory| memory.request_focus(name_id));
            android::set_text_ime();
            form.focus_name = false;
        }
        if name.gained_focus() {
            android::set_text_ime();
        }

        ui.add_space(20.0);
        widgets::overline(ui, "TYPE");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if type_chip(ui, "Spending", form.kind == TxKind::Expense).clicked() {
                form.kind = TxKind::Expense;
            }
            if type_chip(ui, "Income", form.kind == TxKind::Income).clicked() {
                form.kind = TxKind::Income;
            }
        });

        ui.add_space(20.0);
        widgets::overline(ui, "ICON");
        ui.add_space(8.0);
        let width = ui.available_width();
        let cell = (width - 4.0 * 10.0) / 5.0;
        let icons_list = CategoryIcon::ALL;
        let mut index = 0;
        while index < icons_list.len() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                for _ in 0..5 {
                    if let Some(icon) = icons_list.get(index) {
                        let selected = form.icon == *icon;
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(cell, 54.0), Sense::click());
                        let fill = if selected {
                            WHITE
                        } else if response.hovered() {
                            alpha(WHITE, 7)
                        } else {
                            alpha(WHITE, 3)
                        };
                        ui.painter().rect_filled(rect, CornerRadius::same(14), fill);
                        icons::paint_center(
                            ui.painter(),
                            icons::category(*icon),
                            rect.center(),
                            20.0,
                            if selected { BLACK } else { muted() },
                        );
                        if response.clicked() {
                            form.icon = *icon;
                            tap(state);
                        }
                    }
                    index += 1;
                }
            });
            ui.add_space(10.0);
        }

        if let Some(error) = &form.error {
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new(error)
                    .font(sans_font(12.0))
                    .color(bad()),
            );
        }

        ui.add_space(24.0);
        if widgets::white_button(ui, "Save category", true).clicked() {
            android::hide_keyboard();
            action = CategoryAction::Save;
        }
        if let Some(id) = form.editing {
            if state.category(id).map(|c| !c.builtin).unwrap_or(false) {
                ui.add_space(10.0);
                if widgets::text_button(ui, "Delete category", bad()).clicked() {
                    action = CategoryAction::Delete;
                }
            }
        }
    });
    action
}

// ---------------------------------------------------------------------------
// Budget editor
// ---------------------------------------------------------------------------

pub struct BudgetForm {
    pub raw: String,
    pub reset_day: u8,
    pub error: Option<String>,
    focus: bool,
}

impl BudgetForm {
    pub fn new(monthly: Minor, reset_day: u8, currency: Currency) -> Self {
        Self {
            raw: if monthly > 0 {
                currency.plain(monthly.unsigned_abs())
            } else {
                String::new()
            },
            reset_day: reset_day.clamp(1, 28),
            error: None,
            focus: monthly == 0,
        }
    }

    pub fn amount(&self, currency: Currency) -> Option<Minor> {
        parse_amount(&self.raw, currency.decimals())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BudgetAction {
    None,
    Close,
    Save,
}

pub fn render_budget(ui: &mut Ui, form: &mut BudgetForm, state: &AppState) -> BudgetAction {
    let mut action = BudgetAction::None;
    widgets::sheet_page(ui, "budget", Motion::still(), |ui| {
        ui.horizontal(|ui| {
            if widgets::icon_button(ui, Icon::Close, 42.0, ink()).clicked() {
                android::hide_keyboard();
                action = BudgetAction::Close;
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Monthly budget")
                    .font(semibold(18.0))
                    .color(ink()),
            );
        });
        ui.add_space(30.0);
        ui.label(
            egui::RichText::new("How much do you have to spend this month?")
                .font(sans_font(13.0))
                .color(muted()),
        );
        ui.add_space(14.0);
        let id = Id::new("budget-amount");
        let response = widgets::field(ui, "budget-amount", &mut form.raw, "0", 48.0, true, false);
        if form.focus {
            ui.memory_mut(|memory| memory.request_focus(id));
            android::set_numeric_ime();
            form.focus = false;
        }
        if response.gained_focus() {
            android::set_numeric_ime();
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(state.currency.code())
                .font(semibold(10.5))
                .color(faint()),
        );
        ui.add_space(22.0);
        reset_day_row(ui, &mut form.reset_day);
        ui.add_space(12.0);
        widgets::muted(
            ui,
            "Your allowance is the budget still left, spread over the days left in this cycle. Earlier spending doesn't get charged to today again.",
            12.0,
        );

        if let Some(error) = &form.error {
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new(error)
                    .font(sans_font(12.0))
                    .color(bad()),
            );
        }

        ui.add_space(28.0);
        if widgets::white_button(ui, "Save budget", true).clicked() {
            android::hide_keyboard();
            action = BudgetAction::Save;
        }
    });
    action
}

/// Shared by the budget sheet and onboarding: asks which day starts the cycle.
pub fn reset_day_row(ui: &mut Ui, day: &mut u8) -> bool {
    let mut changed = false;
    widgets::overline(ui, "BUDGET RESETS ON");
    ui.add_space(6.0);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 56.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(16), alpha(WHITE, 4));
    let centers = [
        (
            pos2(rect.left() + 28.0, rect.center().y),
            Icon::ChevronLeft,
            -1i8,
        ),
        (
            pos2(rect.right() - 28.0, rect.center().y),
            Icon::ChevronRight,
            1i8,
        ),
    ];
    for (center, icon, delta) in centers {
        let hit = Rect::from_center_size(center, Vec2::splat(44.0));
        let response = ui.interact(hit, Id::new(("reset-day-step", delta)), Sense::click());
        icons::paint_center(
            ui.painter(),
            icon,
            center,
            16.0,
            if response.hovered() { ink() } else { muted() },
        );
        if response.clicked() {
            *day = (*day as i16 + delta as i16).clamp(1, 28) as u8;
            changed = true;
        }
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("the {} of each month", ordinal(*day)),
        semibold(14.0),
        ink(),
    );
    changed
}

// ---------------------------------------------------------------------------
// Confirmation
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfirmResult {
    None,
    Cancelled,
    Confirmed,
}

pub fn confirm_dialog(
    ctx: &egui::Context,
    title: &str,
    body: &str,
    label: &str,
    danger: bool,
) -> ConfirmResult {
    let mut result = ConfirmResult::None;
    egui::Area::new(Id::new("aiotrack-confirm"))
        .order(egui::Order::Foreground)
        .fixed_pos(ctx.screen_rect().min)
        .interactable(true)
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::hover());
            ui.painter()
                .rect_filled(rect, 0, Color32::from_black_alpha(220));

            let card = Rect::from_center_size(screen.center(), vec2(320.0, 250.0));
            ui.painter()
                .rect_filled(card, CornerRadius::same(24), raised());
            ui.painter().text(
                pos2(card.left() + 24.0, card.top() + 26.0),
                Align2::LEFT_TOP,
                title,
                semibold(19.0),
                ink(),
            );
            let galley = ui.painter().layout(
                body.to_owned(),
                sans_font(12.5),
                muted(),
                card.width() - 48.0,
            );
            ui.painter()
                .galley(pos2(card.left() + 24.0, card.top() + 60.0), galley, muted());

            let cancel = Rect::from_min_size(
                pos2(card.left() + 20.0, card.bottom() - 68.0),
                vec2(126.0, 46.0),
            );
            let confirm = Rect::from_min_size(
                pos2(card.right() - 146.0, card.bottom() - 68.0),
                vec2(126.0, 46.0),
            );

            let cancel_response = ui.interact(cancel, Id::new("confirm-cancel"), Sense::click());
            ui.painter().rect_stroke(
                cancel,
                CornerRadius::same(23),
                Stroke::new(1.0_f32, line()),
                StrokeKind::Inside,
            );
            ui.painter().text(
                cancel.center(),
                Align2::CENTER_CENTER,
                "Cancel",
                medium(13.0),
                muted(),
            );
            if cancel_response.clicked() {
                result = ConfirmResult::Cancelled;
            }

            let confirm_response = ui.interact(confirm, Id::new("confirm-ok"), Sense::click());
            ui.painter().rect_filled(
                confirm,
                CornerRadius::same(23),
                if danger { bad() } else { WHITE },
            );
            ui.painter().text(
                confirm.center(),
                Align2::CENTER_CENTER,
                label,
                semibold(13.0),
                if danger { WHITE } else { BLACK },
            );
            if confirm_response.clicked() {
                result = ConfirmResult::Confirmed;
            }
        });
    result
}
