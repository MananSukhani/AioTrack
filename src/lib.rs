//! AioTrack — a quiet, private daily expense tracker.
//!
//! One state file, four surfaces and a handful of gestures. This module owns
//! navigation, persistence, overlays and the Android entry point; every pixel
//! lives in `screens`, `sheets`, `onboarding` or `widgets`.

pub mod android;
pub mod charts;
pub mod icon_assets;
pub mod icons;
pub mod model;
pub mod onboarding;
pub mod screens;
pub mod sheets;
pub mod theme;
pub mod widgets;

use std::path::{Path, PathBuf};

use chrono::Local;
use eframe::egui::{self, Frame};
use model::{
    now_timestamp, parse_amount, Account, AppState, CategoryId, Transaction, TxKind, YearMonth,
};
use onboarding::{OnboardingEvent, OnboardingState};
use screens::{Action, HomeEvent, Page, SettingsPage};
use sheets::{
    AccountAction, AccountForm, BudgetAction, BudgetForm, CategoryAction, CategoryForm, Composer,
    ComposerAction, ConfirmResult,
};

// ---------------------------------------------------------------------------
// Overlays and dialogs
// ---------------------------------------------------------------------------

enum Overlay {
    None,
    Composer(Composer),
    Account(AccountForm),
    Category {
        form: CategoryForm,
        compose_after: bool,
    },
    Budget(BudgetForm),
}

#[derive(Clone, Copy, Debug)]
enum ConfirmAction {
    DeleteTransaction(u64),
    DeleteAccount(u64),
    DeleteCategory(u64),
    ResetAll,
}

struct ConfirmState {
    title: String,
    body: String,
    label: String,
    danger: bool,
    action: ConfirmAction,
}

impl ConfirmState {
    fn delete_transaction() -> Self {
        Self {
            title: "Delete entry?".to_owned(),
            body:
                "This entry will be removed from your activity and balances. It cannot be undone."
                    .to_owned(),
            label: "Delete".to_owned(),
            danger: true,
            action: ConfirmAction::DeleteTransaction(0),
        }
    }

    fn delete_account(id: u64, name: &str, count: usize) -> Self {
        let body = if count > 0 {
            format!(
                "Deleting “{name}” also removes its {count} {} from your activity.",
                if count == 1 { "entry" } else { "entries" }
            )
        } else {
            format!("Deleting “{name}” cannot be undone.")
        };
        Self {
            title: "Delete account?".to_owned(),
            body,
            label: "Delete".to_owned(),
            danger: true,
            action: ConfirmAction::DeleteAccount(id),
        }
    }

    fn delete_category(id: u64, name: &str) -> Self {
        Self {
            title: "Delete category?".to_owned(),
            body: format!("Entries in “{name}” will move to Other. This cannot be undone."),
            label: "Delete".to_owned(),
            danger: true,
            action: ConfirmAction::DeleteCategory(id),
        }
    }

    fn reset() -> Self {
        Self {
            title: "Start over?".to_owned(),
            body: "Every account, category and entry will be erased. A single Cash account will remain. This cannot be undone."
                .to_owned(),
            label: "Erase everything".to_owned(),
            danger: true,
            action: ConfirmAction::ResetAll,
        }
    }
}

const REVEAL: f64 = 0.44;

/// A pointer stroke we are tracking for navigation. Keeping our own start and
/// last position is important: egui clears `press_origin` on release, so a
/// quick flick that begins and ends inside one frame would otherwise vanish.
#[derive(Clone, Copy, Debug)]
struct Gesture {
    start: egui::Pos2,
}

/// The only legal navigation: Home reaches every page; every other page has
/// exactly one way out — back to Home.
fn swipe_target(page: Page, delta: egui::Vec2) -> Option<Page> {
    let min_horizontal = if page == Page::Home { 92.0 } else { 70.0 };
    let horizontal = delta.x.abs() > min_horizontal && delta.x.abs() > delta.y.abs() * 1.25;
    let vertical = delta.y.abs() > 104.0 && delta.y.abs() > delta.x.abs() * 1.25;

    if horizontal {
        if delta.x > 0.0 {
            // Swipe right: Home → Analysis, any other page → Home.
            Some(if page == Page::Home {
                Page::Analysis
            } else {
                Page::Home
            })
        } else if page == Page::Home {
            // Swipe left: Home → Settings.
            Some(Page::Settings)
        } else {
            // On a non-Home page a left swipe is also "take me back".
            Some(Page::Home)
        }
    } else if vertical && page == Page::Home && delta.y < 0.0 {
        // Swipe up: Home → Activity.
        Some(Page::Ledger)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct AioApp {
    state: AppState,
    page: Page,
    settings_page: SettingsPage,
    analysis_advanced: bool,
    month: YearMonth,
    overlay: Overlay,
    /// A composer that stepped aside while a new category was created.
    suspended: Option<Composer>,
    confirm: Option<ConfirmState>,
    toast: Option<(String, f64)>,
    path: Option<PathBuf>,
    entered_at: f64,
    chart_at: f64,
    transition: f32,
    onboarding: Option<OnboardingState>,
    wave: f32,
    ui_scale: f32,
    gesture: Option<Gesture>,
}

impl AioApp {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut state = path.as_deref().and_then(load_state).unwrap_or_default();
        state.normalize();
        theme::install(&cc.egui_ctx);
        Self {
            state,
            page: Page::Home,
            settings_page: SettingsPage::Root,
            analysis_advanced: false,
            month: YearMonth::current(),
            overlay: Overlay::None,
            suspended: None,
            confirm: None,
            toast: None,
            path,
            entered_at: 0.0,
            chart_at: 0.0,
            transition: 0.0,
            onboarding: None,
            wave: 0.0,
            ui_scale: 1.0,
            gesture: None,
        }
    }

    // Persistence -----------------------------------------------------------

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(json) = serde_json::to_string_pretty(&self.state) else {
            return;
        };
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }

    fn after_change(&mut self) {
        self.state.normalize();
        self.save();
    }

    fn notify(&mut self, message: impl Into<String>, now: f64) {
        self.toast = Some((message.into(), now));
    }

    fn backup_dir(&self) -> Option<PathBuf> {
        self.path
            .as_ref()
            .and_then(|path| path.parent())
            .map(|parent| parent.join("backups"))
    }

    fn create_backup(&mut self, now: f64) {
        let Some(dir) = self.backup_dir() else {
            self.notify("No storage available for a backup", now);
            return;
        };
        if std::fs::create_dir_all(&dir).is_err() {
            self.notify("Could not create the backup folder", now);
            return;
        }
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let file = dir.join(format!("aiotrack-{stamp}.json"));
        let Ok(json) = serde_json::to_string_pretty(&self.state) else {
            self.notify("Could not write the backup", now);
            return;
        };
        match std::fs::write(&file, json) {
            Ok(()) => self.notify("Backup saved", now),
            Err(_) => self.notify("Could not write the backup", now),
        }
    }

    fn restore_latest(&mut self, now: f64) {
        let Some(dir) = self.backup_dir() else {
            self.notify("No backups found", now);
            return;
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            self.notify("No backups found", now);
            return;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .collect();
        files.sort();
        let Some(latest) = files.last() else {
            self.notify("No backups found", now);
            return;
        };
        let Ok(text) = std::fs::read_to_string(latest) else {
            self.notify("Could not read the backup", now);
            return;
        };
        let Ok(mut restored) = serde_json::from_str::<AppState>(&text) else {
            self.notify("That backup could not be read", now);
            return;
        };
        restored.onboarded = true;
        restored.normalize();
        self.state = restored;
        self.save();
        self.wave = 0.0;
        self.enter(now);
        self.notify("Latest backup restored", now);
    }

    // Motion ----------------------------------------------------------------

    fn enter(&mut self, now: f64) {
        self.entered_at = now;
        self.chart_at = now;
    }

    fn reveal(&self, now: f64) -> f32 {
        charts::ease_out(((now - self.entered_at) / REVEAL) as f32)
    }

    fn animate_wave(&mut self, ctx: &egui::Context) {
        let target = self.state.spent_fraction_today();
        let dt = ctx.input(|input| input.stable_dt).clamp(1.0 / 240.0, 0.05) as f32;
        let speed = 1.0 - (-7.0 * dt).exp();
        self.wave += (target - self.wave) * speed;
        if (target - self.wave).abs() > 0.0008 {
            ctx.request_repaint();
        } else {
            self.wave = target;
        }
    }

    // Navigation ------------------------------------------------------------

    fn go(&mut self, page: Page, direction: f32, now: f64) {
        if self.page != page {
            self.page = page;
            self.transition = direction;
            self.enter(now);
        }
    }

    fn go_home(&mut self, now: f64) {
        if self.page != Page::Home {
            self.page = Page::Home;
            self.transition = -1.0;
            self.enter(now);
        }
        self.settings_page = SettingsPage::Root;
        self.analysis_advanced = false;
    }

    fn open_settings(&mut self, sub: SettingsPage, now: f64) {
        if self.page != Page::Settings {
            self.page = Page::Settings;
            self.transition = -1.0;
            self.enter(now);
        }
        self.settings_page = sub;
    }

    fn handle_gestures(&mut self, ctx: &egui::Context, now: f64) {
        // The radial menu is only meaningful on Home. Ignoring it on other
        // pages also avoids a stale flag blocking navigation after a swipe.
        if !matches!(self.overlay, Overlay::None)
            || (self.page == Page::Home && screens::radial_active(ctx))
            || (self.page == Page::Ledger && screens::row_hold_active(ctx))
        {
            self.gesture = None;
            return;
        }

        let events = ctx.input(|input| input.events.clone());
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos, pressed: true, ..
                } => {
                    // Home reserves the strip around the add control. Other
                    // pages only reserve the small Home/nav control itself, so
                    // a back-swipe works even while the ledger is scrolled.
                    let screen = ctx.screen_rect();
                    let allowed = if self.page == Page::Home {
                        pos.y < screen.bottom() - 176.0
                    } else if self.state.nav == model::NavMode::Buttons {
                        pos.y < screen.bottom() - 90.0
                    } else {
                        let home = egui::pos2(screen.center().x, screen.bottom() - 100.0);
                        (pos - home).length() > 48.0
                    };
                    self.gesture = if allowed {
                        Some(Gesture { start: pos })
                    } else {
                        None
                    };
                }
                egui::Event::PointerMoved(_) => {}
                egui::Event::PointerButton {
                    pos,
                    pressed: false,
                    ..
                } => {
                    let Some(gesture) = self.gesture.take() else {
                        continue;
                    };
                    // The release event carries the final position, which is
                    // essential when press, move and release share one frame.
                    let delta = pos - gesture.start;
                    let pull_down_home = self.page == Page::Ledger
                        && delta.y > 104.0
                        && delta.y > delta.x.abs() * 1.25
                        && widgets::scroll_offset(ctx, "ledger") <= 2.0;
                    if pull_down_home {
                        self.go_home(now);
                        haptic(&self.state, android::Haptic::Tap);
                    } else if let Some(target) = swipe_target(self.page, delta) {
                        let direction = match target {
                            Page::Analysis => 1.0,
                            Page::Settings => -1.0,
                            Page::Ledger => 0.0,
                            Page::Home => -1.0,
                        };
                        match target {
                            Page::Home => self.go_home(now),
                            Page::Settings => self.open_settings(SettingsPage::Root, now),
                            Page::Analysis | Page::Ledger => {
                                if target == Page::Analysis {
                                    self.analysis_advanced = false;
                                }
                                self.page = target;
                                self.transition = direction;
                                self.enter(now);
                            }
                        }
                        haptic(&self.state, android::Haptic::Tap);
                    }
                }
                egui::Event::PointerGone => self.gesture = None,
                _ => {}
            }
        }
    }

    // Onboarding ------------------------------------------------------------

    fn finish_onboarding(&mut self, flow: OnboardingState, now: f64) {
        self.state.currency = flow.currency;
        self.state.budget_reset_day = flow.reset_day;
        if let Some(budget) = flow.budget() {
            self.state.monthly_budget = budget;
        }
        if flow.named() {
            if let Some(account) = self.state.accounts.first_mut() {
                account.name = flow.account_name.trim().to_owned();
                account.kind = flow.kind;
                account.opening = flow.opening().unwrap_or(0);
            }
        }
        self.state.onboarded = true;
        self.state.normalize();
        self.save();
        self.enter(now);
        self.notify("Welcome to AioTrack", now);
    }

    // Mutations -------------------------------------------------------------

    fn commit_composer(&mut self, form: &Composer) -> Result<(), String> {
        let amount = form
            .amount(self.state.currency)
            .ok_or_else(|| "Enter an amount.".to_owned())?;
        if amount <= 0 {
            return Err("Enter an amount greater than zero.".to_owned());
        }
        if self.state.account(form.account_id).is_none() {
            return Err("Choose an account for this entry.".to_owned());
        }
        let Some(category) = self.state.category(form.category.0) else {
            return Err("Choose a category.".to_owned());
        };
        if category.kind != form.kind {
            return Err("Pick a category that matches the entry type.".to_owned());
        }
        let date = form
            .date()
            .ok_or_else(|| "Use a date like 2026-11-22.".to_owned())?;
        let time = form
            .time()
            .ok_or_else(|| "Use a time like 18:30.".to_owned())?;

        if let Some(id) = form.editing {
            let Some(tx) = self.state.transactions.iter_mut().find(|tx| tx.id == id) else {
                return Err("This entry no longer exists.".to_owned());
            };
            tx.account_id = form.account_id;
            tx.kind = form.kind;
            tx.amount = amount;
            tx.category = form.category;
            tx.note = form.note.trim().to_owned();
            tx.date = date;
            tx.time = Some(time);
        } else {
            let id = self.state.next_tx_id;
            self.state.next_tx_id += 1;
            self.state.transactions.push(Transaction {
                id,
                account_id: form.account_id,
                kind: form.kind,
                amount,
                category: form.category,
                note: form.note.trim().to_owned(),
                date,
                created_at: now_timestamp(),
                time: Some(time),
            });
        }
        if form.kind == TxKind::Expense {
            self.state.last_category = form.category;
        }
        self.state.last_account = form.account_id;
        self.after_change();
        Ok(())
    }

    fn commit_account(&mut self, form: &AccountForm) -> Result<(), String> {
        let name = form.name.trim();
        if name.is_empty() {
            return Err("Give this account a name.".to_owned());
        }
        let opening = if form.opening_raw.trim().is_empty() {
            0
        } else {
            parse_amount(&form.opening_raw, self.state.currency.decimals())
                .ok_or_else(|| "Enter a valid opening balance.".to_owned())?
        };
        if let Some(id) = form.editing {
            let Some(account) = self
                .state
                .accounts
                .iter_mut()
                .find(|account| account.id == id)
            else {
                return Err("This account no longer exists.".to_owned());
            };
            account.name = name.to_owned();
            account.kind = form.kind;
            account.color = form.color;
            account.opening = opening;
        } else {
            let id = self.state.next_account_id;
            self.state.next_account_id += 1;
            self.state.accounts.push(Account {
                id,
                name: name.to_owned(),
                kind: form.kind,
                color: form.color,
                opening,
                created_at: now_timestamp(),
            });
        }
        self.after_change();
        Ok(())
    }

    fn commit_category(&mut self, form: &CategoryForm) -> Result<u64, String> {
        let name = form.name.trim();
        if name.is_empty() {
            return Err("Give this category a name.".to_owned());
        }
        if self.state.categories_for(form.kind).iter().any(|category| {
            category.name.eq_ignore_ascii_case(name) && Some(category.id) != form.editing
        }) {
            return Err("That category already exists.".to_owned());
        }
        if let Some(id) = form.editing {
            let Some(category) = self
                .state
                .categories
                .iter_mut()
                .find(|category| category.id == id)
            else {
                return Err("This category no longer exists.".to_owned());
            };
            category.name = name.to_owned();
            category.icon = form.icon;
            category.kind = form.kind;
            Ok(id)
        } else {
            let id = self.state.add_category(name, form.icon, form.kind);
            Ok(id)
        }
    }

    fn apply_confirm(&mut self, action: ConfirmAction, now: f64) {
        match action {
            ConfirmAction::DeleteTransaction(id) => {
                self.state.transactions.retain(|tx| tx.id != id);
                self.overlay = Overlay::None;
                self.after_change();
                self.notify("Entry deleted", now);
                haptic(&self.state, android::Haptic::Reject);
            }
            ConfirmAction::DeleteAccount(id) => {
                let name = self.state.account_name(id).to_owned();
                self.state.delete_account(id);
                self.overlay = Overlay::None;
                self.after_change();
                self.notify(format!("“{name}” deleted"), now);
            }
            ConfirmAction::DeleteCategory(id) => {
                let name = self.state.category_name(CategoryId(id)).to_owned();
                self.state.delete_category(id);
                self.overlay = Overlay::None;
                self.after_change();
                self.notify(format!("“{name}” deleted"), now);
            }
            ConfirmAction::ResetAll => {
                let currency = self.state.currency;
                let nav = self.state.nav;
                let haptics = self.state.haptics;
                self.state = AppState::default();
                self.state.currency = currency;
                self.state.nav = nav;
                self.state.haptics = haptics;
                self.state.onboarded = true;
                self.overlay = Overlay::None;
                self.suspended = None;
                self.page = Page::Home;
                self.settings_page = SettingsPage::Root;
                self.month = YearMonth::current();
                self.wave = 0.0;
                self.enter(now);
                self.after_change();
                self.notify("All data erased", now);
            }
        }
    }

    // Action dispatch -------------------------------------------------------

    fn apply_action(&mut self, action: Action, now: f64) {
        match action {
            Action::None => {}
            Action::Home => self.go_home(now),
            Action::OpenLedger => self.go(Page::Ledger, 0.0, now),
            Action::OpenAnalysis => {
                self.analysis_advanced = false;
                self.go(Page::Analysis, 1.0, now)
            }
            Action::OpenAdvanced => {
                self.analysis_advanced = true;
                self.enter(now);
            }
            Action::CloseAdvanced => {
                self.analysis_advanced = false;
                self.enter(now);
            }
            Action::OpenSettings => self.open_settings(SettingsPage::Root, now),
            Action::Settings(sub) => {
                self.open_settings(sub, now);
            }
            Action::Back => {
                if self.settings_page != SettingsPage::Root {
                    self.settings_page = SettingsPage::Root;
                    self.enter(now);
                } else {
                    self.go_home(now);
                }
            }
            Action::ChangeMonth(delta) => {
                let target = self.month.shifted(delta);
                if target.index() <= YearMonth::current().index() {
                    self.month = target;
                    self.enter(now);
                }
            }
            Action::SetCurrency(currency) => {
                if self.state.currency != currency {
                    self.state.currency = currency;
                    self.save();
                    self.notify(format!("Currency set to {}", currency.code()), now);
                }
            }
            Action::SetNav(nav) => {
                self.state.nav = nav;
                self.save();
            }
            Action::ToggleHaptics => {
                self.state.haptics = !self.state.haptics;
                self.save();
                haptic(&self.state, android::Haptic::Tap);
            }
            Action::NewAccount => {
                self.overlay = Overlay::Account(AccountForm::new(self.state.currency));
                self.enter(now);
            }
            Action::EditAccount(id) => {
                if let Some(account) = self.state.account(id) {
                    self.overlay =
                        Overlay::Account(AccountForm::edit(account, self.state.currency));
                    self.enter(now);
                }
            }
            Action::DeleteAccount(id) => {
                if self.state.accounts.len() <= 1 {
                    self.notify("Keep at least one account", now);
                } else {
                    let name = self.state.account_name(id).to_owned();
                    let count = self.state.account_transaction_count(id);
                    self.confirm = Some(ConfirmState::delete_account(id, &name, count));
                }
            }
            Action::NewCategory => {
                self.overlay = Overlay::Category {
                    form: CategoryForm::new(TxKind::Expense),
                    compose_after: false,
                };
                self.enter(now);
            }
            Action::EditCategory(id) => {
                if let Some(category) = self.state.category(id) {
                    self.overlay = Overlay::Category {
                        form: CategoryForm::edit(category),
                        compose_after: false,
                    };
                    self.enter(now);
                }
            }
            Action::DeleteCategory(id) => {
                let name = self.state.category_name(CategoryId(id)).to_owned();
                self.confirm = Some(ConfirmState::delete_category(id, &name));
            }
            Action::OpenComposer(category) => {
                self.overlay = Overlay::Composer(Composer::new(&self.state, category));
                self.enter(now);
            }
            Action::EditTransaction(id) => {
                if let Some(tx) = self.state.transactions.iter().find(|tx| tx.id == id) {
                    self.overlay = Overlay::Composer(Composer::edit(tx, &self.state));
                    self.enter(now);
                }
            }
            Action::DeleteTransaction(id) => {
                let mut confirm = ConfirmState::delete_transaction();
                confirm.action = ConfirmAction::DeleteTransaction(id);
                self.confirm = Some(confirm);
            }
            Action::OpenBudget => {
                self.overlay = Overlay::Budget(BudgetForm::new(
                    self.state.monthly_budget,
                    self.state.budget_reset_day,
                    self.state.currency,
                ));
                self.enter(now);
            }
            Action::Backup => self.create_backup(now),
            Action::Restore => self.restore_latest(now),
            Action::OpenUrl(url) => {
                android::open_url(url);
                self.notify("Opening link", now);
            }
            Action::ResetAll => self.confirm = Some(ConfirmState::reset()),
        }
    }

    fn handle_home(&mut self, event: HomeEvent, now: f64) {
        match event {
            HomeEvent::None => {}
            HomeEvent::Add(category) => {
                let category = if self
                    .state
                    .category(category.0)
                    .map(|c| c.kind == TxKind::Expense)
                    .unwrap_or(false)
                {
                    category
                } else {
                    self.state
                        .sorted_categories(TxKind::Expense)
                        .first()
                        .map(|c| CategoryId(c.id))
                        .unwrap_or(CategoryId(1))
                };
                self.overlay = Overlay::Composer(Composer::new(&self.state, category));
                self.enter(now);
            }
            HomeEvent::NewCategory => {
                self.overlay = Overlay::Category {
                    form: CategoryForm::new(TxKind::Expense),
                    compose_after: true,
                };
                self.enter(now);
            }
            HomeEvent::SetBudget => {
                self.overlay = Overlay::Budget(BudgetForm::new(
                    self.state.monthly_budget,
                    self.state.budget_reset_day,
                    self.state.currency,
                ));
                self.enter(now);
            }
        }
    }
}

fn haptic(state: &AppState, kind: android::Haptic) {
    if state.haptics {
        android::haptic(kind);
    }
}

/// Load a state file, tolerating the legacy category names.
fn load_state(path: &Path) -> Option<AppState> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut state: AppState = serde_json::from_str(&text).ok()?;
    state.normalize();
    Some(state)
}

// ---------------------------------------------------------------------------
// Layout smoke tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod ui_smoke {
    use super::*;
    use eframe::egui::{self, vec2, Frame, Rect};

    fn frame(ctx: &egui::Context, mut add: impl FnMut(&mut egui::Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(393.0, 851.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(theme::bg()))
                .show(ctx, |ui| add(ui));
        });
    }

    fn sample_state() -> AppState {
        let mut state = AppState::default();
        state.currency = model::Currency::Inr;
        state.onboarded = true;
        state.monthly_budget = 62_000_00;
        state.accounts.push(Account {
            id: 2,
            name: "Everyday Bank".to_owned(),
            kind: model::AccountKind::Bank,
            color: 1,
            opening: 25_000_00,
            created_at: now_timestamp(),
        });
        state.transactions.push(Transaction {
            id: 1,
            account_id: 1,
            kind: TxKind::Expense,
            amount: 129_900,
            category: CategoryId(1),
            note: "Dinner with the family".to_owned(),
            date: model::today(),
            created_at: now_timestamp(),
            time: Some(20 * 60 + 15),
        });
        state.transactions.push(Transaction {
            id: 2,
            account_id: 2,
            kind: TxKind::Income,
            amount: 8_500_00,
            category: CategoryId(11),
            note: String::new(),
            date: model::today(),
            created_at: now_timestamp(),
            time: Some(9 * 60),
        });
        state.normalize();
        state
    }

    #[test]
    fn every_screen_lays_out() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let state = sample_state();
        for _ in 0..2 {
            frame(&ctx, |ui| {
                let _ = screens::home(ui, &state, 0.4, 1.0);
            });
            frame(&ctx, |ui| {
                let _ = screens::ledger(ui, &state, widgets::Motion::still(), 1.0);
            });
            frame(&ctx, |ui| {
                let _ = screens::analysis(
                    ui,
                    &state,
                    YearMonth::current(),
                    widgets::Motion::still(),
                    1.0,
                );
            });
            frame(&ctx, |ui| {
                let _ = screens::advanced(ui, &state, widgets::Motion::still(), 1.0);
            });
            for page in [
                SettingsPage::Root,
                SettingsPage::Navigation,
                SettingsPage::Accounts,
                SettingsPage::Categories,
                SettingsPage::Currency,
                SettingsPage::Backup,
                SettingsPage::About,
            ] {
                frame(&ctx, |ui| {
                    let _ = screens::settings(ui, &state, page, widgets::Motion::still(), 1.0);
                });
            }
        }
    }

    #[test]
    fn onboarding_lays_out() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut flow = OnboardingState::new(model::Currency::Inr, 0.0);
        for step in 0..5 {
            flow.step = step;
            flow.tutorial = 0;
            for _ in 0..2 {
                frame(&ctx, |ui| {
                    let _ = onboarding::render(ui, &mut flow, 1.0);
                });
            }
        }
        for tutorial in 0..3 {
            flow.step = 4;
            flow.tutorial = tutorial;
            frame(&ctx, |ui| {
                let _ = onboarding::render(ui, &mut flow, 1.0);
            });
        }
    }

    #[test]
    fn sheets_lay_out() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let state = sample_state();
        let mut form = Composer::new(&state, CategoryId(1));
        form.amount_raw = "1234.50".to_owned();
        for _ in 0..2 {
            frame(&ctx, |ui| {
                let _ = sheets::render_composer(ui, &mut form, &state);
            });
        }
        form.details = true;
        form.picking = true;
        frame(&ctx, |ui| {
            let _ = sheets::render_composer(ui, &mut form, &state);
        });
        let mut account = AccountForm::edit(&state.accounts[0], state.currency);
        frame(&ctx, |ui| {
            let _ = sheets::render_account_editor(ui, &mut account, state.currency);
        });
        let mut category = CategoryForm::new(TxKind::Expense);
        frame(&ctx, |ui| {
            let _ = sheets::render_category_editor(ui, &mut category, &state);
        });
        let mut budget =
            BudgetForm::new(state.monthly_budget, state.budget_reset_day, state.currency);
        frame(&ctx, |ui| {
            let _ = sheets::render_budget(ui, &mut budget, &state);
        });
        frame(&ctx, |ui| {
            let _ = sheets::confirm_dialog(
                ui.ctx(),
                "Delete entry?",
                "This entry will be removed from your activity.",
                "Delete",
                true,
            );
        });
    }

    #[test]
    fn budget_maths_flow() {
        let state = sample_state();
        assert!(state.monthly_budget > 0);
        let left = state.budget_days_left() as i64;
        let earlier = state.spent_before_on(model::today());
        assert_eq!(
            state.daily_budget(),
            (state.monthly_budget - earlier).max(0) / left
        );
        assert_eq!(
            state.available_today(),
            state.daily_budget() - state.spent_today()
        );
    }

    #[test]
    fn nav_modes_cover_every_page() {
        let state = AppState::default();
        assert_eq!(state.nav, model::NavMode::Gestures);
        assert_eq!(state.nav.toggled(), model::NavMode::Buttons);
        let pages = [Page::Home, Page::Ledger, Page::Analysis, Page::Settings];
        assert_eq!(pages.len(), 4);
    }

    #[test]
    fn gestures_only_leave_home_to_pages_and_return_home() {
        use egui::vec2;

        // From Home every surface is one swipe away.
        assert_eq!(
            swipe_target(Page::Home, vec2(140.0, 8.0)),
            Some(Page::Analysis)
        );
        assert_eq!(
            swipe_target(Page::Home, vec2(-140.0, 8.0)),
            Some(Page::Settings)
        );
        assert_eq!(
            swipe_target(Page::Home, vec2(8.0, -150.0)),
            Some(Page::Ledger)
        );

        // From any other page either horizontal swipe returns Home.
        for page in [Page::Ledger, Page::Analysis, Page::Settings] {
            assert_eq!(swipe_target(page, vec2(140.0, 8.0)), Some(Page::Home));
            assert_eq!(swipe_target(page, vec2(-140.0, 8.0)), Some(Page::Home));
            assert_eq!(swipe_target(page, vec2(80.0, 4.0)), Some(Page::Home));
            assert_eq!(swipe_target(page, vec2(8.0, -150.0)), None);
        }

        // Short or diagonal strokes never navigate.
        assert_eq!(swipe_target(Page::Home, vec2(40.0, 0.0)), None);
        assert_eq!(swipe_target(Page::Home, vec2(120.0, 110.0)), None);
    }
}

// ---------------------------------------------------------------------------
// UI loop
// ---------------------------------------------------------------------------

impl eframe::App for AioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|input| input.time);

        // Fit typography to the viewport. Android phones report logical dp,
        // so a 360dp screen gets a smaller scale than the 400dp design size.
        let target_scale = (ctx.screen_rect().width() / 400.0).clamp(0.72, 1.2);
        if (target_scale - self.ui_scale).abs() > 0.01 {
            theme::set_scale(target_scale);
            theme::install(ctx);
            self.ui_scale = target_scale;
        }

        self.animate_wave(ctx);

        // First run ----------------------------------------------------------
        if !self.state.onboarded {
            if self.onboarding.is_none() {
                self.onboarding = Some(OnboardingState::new(self.state.currency, now));
            }
            let mut event = OnboardingEvent::None;
            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(theme::bg()))
                .show(ctx, |ui| {
                    if let Some(flow) = &mut self.onboarding {
                        event = onboarding::render(ui, flow, now);
                    }
                });
            match event {
                OnboardingEvent::None => {}
                OnboardingEvent::Next => {
                    if let Some(flow) = &mut self.onboarding {
                        flow.advance(now);
                    }
                }
                OnboardingEvent::Back => {
                    if let Some(flow) = &mut self.onboarding {
                        flow.back(now);
                    }
                }
                OnboardingEvent::Finish => {
                    if let Some(flow) = self.onboarding.take() {
                        android::hide_keyboard();
                        self.finish_onboarding(flow, now);
                    }
                }
            }
            if let Some((message, started)) = self.toast.clone() {
                widgets::toast(ctx, &message, started, now);
            }
            return;
        }
        if self.onboarding.is_some() {
            self.onboarding = None;
        }

        self.handle_gestures(ctx, now);

        let reveal = self.reveal(now);
        if reveal < 1.0 {
            ctx.request_repaint();
        }
        let chart_progress = charts::ease_out(((now - self.chart_at) / 0.9) as f32);
        let motion = if self.page == Page::Ledger && matches!(self.overlay, Overlay::None) {
            widgets::Motion::vertical(reveal, 1.0)
        } else {
            widgets::Motion::from(reveal, self.transition)
        };

        let mut action = Action::None;
        let mut home_event = HomeEvent::None;
        let mut composer_action = ComposerAction::None;
        let mut account_action = AccountAction::None;
        let mut category_action = CategoryAction::None;
        let mut budget_action = BudgetAction::None;

        let state = &self.state;
        let page = self.page;
        let settings_page = self.settings_page;
        let advanced = self.analysis_advanced;
        let month = self.month;
        let wave = self.wave;
        let overlay = &mut self.overlay;

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(theme::bg()))
            .show(ctx, |ui| {
                if matches!(overlay, Overlay::None) {
                    match page {
                        Page::Home => {
                            home_event = screens::home(ui, state, wave, now);
                        }
                        Page::Ledger => {
                            action = screens::ledger(ui, state, motion, now);
                        }
                        Page::Analysis => {
                            action = if advanced {
                                screens::advanced(ui, state, motion, chart_progress)
                            } else {
                                screens::analysis(ui, state, month, motion, chart_progress)
                            };
                        }
                        Page::Settings => {
                            action = screens::settings(ui, state, settings_page, motion, now);
                        }
                    }
                } else {
                    ui.set_opacity(reveal.max(0.25));
                    match overlay {
                        Overlay::None => {}
                        Overlay::Composer(form) => {
                            composer_action = sheets::render_composer(ui, form, state);
                        }
                        Overlay::Account(form) => {
                            account_action =
                                sheets::render_account_editor(ui, form, state.currency);
                        }
                        Overlay::Category { form, .. } => {
                            category_action = sheets::render_category_editor(ui, form, state);
                        }
                        Overlay::Budget(form) => {
                            budget_action = sheets::render_budget(ui, form, state);
                        }
                    }
                }
            });

        if let Some(target) = screens::take_nav(ctx) {
            self.apply_action(
                match target {
                    Page::Home => Action::Home,
                    Page::Ledger => Action::OpenLedger,
                    Page::Analysis => Action::OpenAnalysis,
                    Page::Settings => Action::OpenSettings,
                },
                now,
            );
        }

        self.handle_home(home_event, now);
        self.apply_action(action, now);

        if sheets::take_new_category_request(ctx) {
            if let Overlay::Composer(form) = std::mem::replace(&mut self.overlay, Overlay::None) {
                self.suspended = Some(form);
            }
            self.overlay = Overlay::Category {
                form: CategoryForm::new(TxKind::Expense),
                compose_after: true,
            };
            self.enter(now);
        }

        match composer_action {
            ComposerAction::None => {}
            ComposerAction::Close => {
                android::hide_keyboard();
                self.overlay = Overlay::None;
                self.suspended = None;
                self.enter(now);
            }
            ComposerAction::Save => {
                let overlay = std::mem::replace(&mut self.overlay, Overlay::None);
                if let Overlay::Composer(form) = overlay {
                    let editing = form.editing.is_some();
                    match self.commit_composer(&form) {
                        Ok(()) => {
                            android::hide_keyboard();
                            self.enter(now);
                            self.notify(
                                if editing {
                                    "Changes saved"
                                } else {
                                    "Entry added"
                                },
                                now,
                            );
                            haptic(&self.state, android::Haptic::Confirm);
                        }
                        Err(error) => {
                            let mut form = form;
                            form.error = Some(error);
                            self.overlay = Overlay::Composer(form);
                        }
                    }
                }
            }
            ComposerAction::Delete => {
                if let Overlay::Composer(form) = &self.overlay {
                    if let Some(id) = form.editing {
                        self.apply_action(Action::DeleteTransaction(id), now);
                    }
                }
            }
            ComposerAction::NewCategory => {
                if let Overlay::Composer(form) = std::mem::replace(&mut self.overlay, Overlay::None)
                {
                    let kind = form.kind;
                    self.suspended = Some(form);
                    self.overlay = Overlay::Category {
                        form: CategoryForm::new(kind),
                        compose_after: true,
                    };
                    self.enter(now);
                }
            }
        }

        match account_action {
            AccountAction::None => {}
            AccountAction::Close => {
                android::hide_keyboard();
                self.overlay = Overlay::None;
                self.enter(now);
            }
            AccountAction::Save => {
                let overlay = std::mem::replace(&mut self.overlay, Overlay::None);
                if let Overlay::Account(form) = overlay {
                    let editing = form.editing.is_some();
                    match self.commit_account(&form) {
                        Ok(()) => {
                            android::hide_keyboard();
                            self.enter(now);
                            self.notify(
                                if editing {
                                    "Account updated"
                                } else {
                                    "Account added"
                                },
                                now,
                            );
                        }
                        Err(error) => {
                            let mut form = form;
                            form.error = Some(error);
                            self.overlay = Overlay::Account(form);
                        }
                    }
                }
            }
            AccountAction::Delete => {
                if let Overlay::Account(form) = &self.overlay {
                    if let Some(id) = form.editing {
                        self.apply_action(Action::DeleteAccount(id), now);
                    }
                }
            }
        }

        match category_action {
            CategoryAction::None => {}
            CategoryAction::Close => {
                android::hide_keyboard();
                self.overlay = match self.suspended.take() {
                    Some(form) => Overlay::Composer(form),
                    None => Overlay::None,
                };
                self.enter(now);
            }
            CategoryAction::Save => {
                let overlay = std::mem::replace(&mut self.overlay, Overlay::None);
                if let Overlay::Category {
                    form,
                    compose_after,
                } = overlay
                {
                    match self.commit_category(&form) {
                        Ok(id) => {
                            android::hide_keyboard();
                            self.enter(now);
                            self.notify("Category saved", now);
                            if compose_after {
                                let mut composer = self
                                    .suspended
                                    .take()
                                    .unwrap_or_else(|| Composer::new(&self.state, CategoryId(id)));
                                composer.category = CategoryId(id);
                                composer.picking = false;
                                self.overlay = Overlay::Composer(composer);
                            }
                        }
                        Err(error) => {
                            let mut form = form;
                            form.error = Some(error);
                            self.overlay = Overlay::Category {
                                form,
                                compose_after,
                            };
                        }
                    }
                }
            }
            CategoryAction::Delete => {
                if let Overlay::Category { form, .. } = &self.overlay {
                    if let Some(id) = form.editing {
                        self.apply_action(Action::DeleteCategory(id), now);
                    }
                }
            }
        }

        match budget_action {
            BudgetAction::None => {}
            BudgetAction::Close => {
                android::hide_keyboard();
                self.overlay = Overlay::None;
                self.enter(now);
            }
            BudgetAction::Save => {
                let overlay = std::mem::replace(&mut self.overlay, Overlay::None);
                if let Overlay::Budget(form) = overlay {
                    match form.amount(self.state.currency) {
                        Some(amount) if amount > 0 => {
                            self.state.monthly_budget = amount;
                            self.state.budget_reset_day = form.reset_day.clamp(1, 28);
                            self.after_change();
                            android::hide_keyboard();
                            self.enter(now);
                            self.notify("Budget saved", now);
                            haptic(&self.state, android::Haptic::Confirm);
                        }
                        _ => {
                            let mut form = form;
                            form.error = Some("Enter an amount greater than zero.".to_owned());
                            self.overlay = Overlay::Budget(form);
                        }
                    }
                }
            }
        }

        if let Some(confirm) = &self.confirm {
            let result = sheets::confirm_dialog(
                ctx,
                &confirm.title,
                &confirm.body,
                &confirm.label,
                confirm.danger,
            );
            match result {
                ConfirmResult::None => {}
                ConfirmResult::Cancelled => self.confirm = None,
                ConfirmResult::Confirmed => {
                    if let Some(confirm) = self.confirm.take() {
                        self.apply_confirm(confirm.action, now);
                    }
                }
            }
        }

        if let Some((message, started)) = self.toast.clone() {
            widgets::toast(ctx, &message, started, now);
        }
        if self
            .toast
            .as_ref()
            .map(|(_, started)| now - started > 2.6)
            .unwrap_or(false)
        {
            self.toast = None;
        }

        // Hardware back / escape unwinds one layer at a time.
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.confirm.is_some() {
                self.confirm = None;
            } else if !matches!(self.overlay, Overlay::None) {
                android::hide_keyboard();
                self.overlay = Overlay::None;
                self.suspended = None;
                self.enter(now);
            } else if self.settings_page != SettingsPage::Root {
                self.settings_page = SettingsPage::Root;
                self.enter(now);
            } else if self.analysis_advanced {
                self.analysis_advanced = false;
                self.enter(now);
            } else if self.page != Page::Home {
                self.go_home(now);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Android
// ---------------------------------------------------------------------------
#[cfg(target_os = "android")]
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::activity::AndroidApp;

    fn run(app: AndroidApp) {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("AioTrack"),
        );
        android::init(&app);
        let path = app.internal_data_path().map(|dir| dir.join("state.json"));
        let options = eframe::NativeOptions {
            android_app: Some(app),
            viewport: egui::ViewportBuilder::default()
                .with_title("AioTrack")
                .with_inner_size([400.0, 860.0]),
            ..Default::default()
        };
        let _ = eframe::run_native(
            "AioTrack",
            options,
            Box::new(move |cc| Ok(Box::new(AioApp::new(cc, path)))),
        );
    }

    run(app);
}
