//! Data model, money formatting, dates and all pure calculations.
//!
//! Everything in this file is independent of egui so it can be unit-tested
//! without a window (and it keeps the UI code focused on looking good).

use std::fmt;

use chrono::{Datelike, Local, Months, NaiveDate, NaiveTime, TimeZone, Timelike};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub type Minor = i64;

// ---------------------------------------------------------------------------
// Currency
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Currency {
    #[default]
    Usd,
    Eur,
    Gbp,
    Inr,
    Jpy,
    Cad,
    Aud,
    Chf,
    Sgd,
    Aed,
}

impl Currency {
    pub const ALL: [Currency; 10] = [
        Currency::Usd,
        Currency::Eur,
        Currency::Gbp,
        Currency::Inr,
        Currency::Jpy,
        Currency::Cad,
        Currency::Aud,
        Currency::Chf,
        Currency::Sgd,
        Currency::Aed,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Currency::Usd => "USD",
            Currency::Eur => "EUR",
            Currency::Gbp => "GBP",
            Currency::Inr => "INR",
            Currency::Jpy => "JPY",
            Currency::Cad => "CAD",
            Currency::Aud => "AUD",
            Currency::Chf => "CHF",
            Currency::Sgd => "SGD",
            Currency::Aed => "AED",
        }
    }

    /// Display prefix. Currency codes get a trailing space; symbols do not.
    pub fn symbol(self) -> &'static str {
        match self {
            Currency::Usd => "$",
            Currency::Eur => "€",
            Currency::Gbp => "£",
            Currency::Inr => "₹",
            Currency::Jpy => "¥",
            Currency::Cad => "C$",
            Currency::Aud => "A$",
            Currency::Chf => "CHF ",
            Currency::Sgd => "S$",
            Currency::Aed => "AED ",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Currency::Usd => "US Dollar",
            Currency::Eur => "Euro",
            Currency::Gbp => "British Pound",
            Currency::Inr => "Indian Rupee",
            Currency::Jpy => "Japanese Yen",
            Currency::Cad => "Canadian Dollar",
            Currency::Aud => "Australian Dollar",
            Currency::Chf => "Swiss Franc",
            Currency::Sgd => "Singapore Dollar",
            Currency::Aed => "UAE Dirham",
        }
    }

    pub fn decimals(self) -> u32 {
        match self {
            Currency::Jpy => 0,
            _ => 2,
        }
    }

    /// Amount without a sign, e.g. `$1,248.50`.
    pub fn format_abs(self, minor: Minor) -> String {
        format!("{}{}", self.symbol(), self.number(minor.unsigned_abs()))
    }

    /// Amount with a leading minus for negative values.
    pub fn format(self, minor: Minor) -> String {
        if minor < 0 {
            format!("-{}", self.format_abs(minor))
        } else {
            self.format_abs(minor)
        }
    }

    /// Grouped number only (no currency symbol).
    pub fn number(self, minor_abs: u64) -> String {
        let decimals = self.decimals();
        let scale = 10u64.pow(decimals);
        let whole = minor_abs / scale;
        let frac = minor_abs % scale;
        let grouped = group_digits(whole, self == Currency::Inr);
        if decimals == 0 {
            grouped
        } else {
            format!("{}.{:0width$}", grouped, frac, width = decimals as usize)
        }
    }

    /// Ungrouped editable form, e.g. `90000.00`. Text fields must not start
    /// with thousands separators because `,` is also accepted as a decimal.
    pub fn plain(self, minor_abs: u64) -> String {
        let decimals = self.decimals();
        let scale = 10u64.pow(decimals);
        let whole = minor_abs / scale;
        let frac = minor_abs % scale;
        if decimals == 0 {
            whole.to_string()
        } else {
            format!("{}.{:0width$}", whole, frac, width = decimals as usize)
        }
    }

    /// Compact amount for chart axes, e.g. `$1.2k`.
    pub fn compact(self, minor_abs: Minor) -> String {
        let whole = minor_abs.unsigned_abs() as f64 / 10f64.powi(self.decimals() as i32);
        if whole >= 1_000_000.0 {
            format!("{}{:.1}m", self.symbol(), whole / 1_000_000.0)
        } else if whole >= 1000.0 {
            let k = whole / 1000.0;
            if k >= 10.0 {
                format!("{}{:.0}k", self.symbol(), k)
            } else {
                format!("{}{:.1}k", self.symbol(), k)
            }
        } else {
            self.format_abs(minor_abs)
        }
    }
}

fn group_digits(value: u64, indian: bool) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    if indian && digits.len() > 3 {
        let (head, tail) = digits.split_at(digits.len() - 3);
        let mut first = true;
        for (i, ch) in head.chars().enumerate() {
            if !first && (head.len() - i) % 2 == 0 {
                out.push(',');
            }
            first = false;
            out.push(ch);
        }
        out.push(',');
        out.push_str(tail);
        return out;
    }
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Parse a user-typed amount into minor units.
pub fn parse_amount(raw: &str, decimals: u32) -> Option<Minor> {
    let raw = raw.trim().replace(',', "").replace(' ', "");
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with('-') {
        return None;
    }
    let mut parts = raw.split('.');
    let whole_text = parts.next().unwrap_or("");
    let frac_text = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return None;
    }
    if whole_text.is_empty() && frac_text.is_empty() {
        return None;
    }
    let whole: i64 = if whole_text.is_empty() {
        0
    } else {
        whole_text.parse().ok()?
    };
    if whole > 99_999_999 {
        return None;
    }
    let mut frac = frac_text
        .chars()
        .take(decimals as usize)
        .collect::<String>();
    while frac.len() < decimals as usize {
        frac.push('0');
    }
    let frac_value: i64 = if decimals == 0 { 0 } else { frac.parse().ok()? };
    whole
        .checked_mul(10i64.pow(decimals))?
        .checked_add(frac_value)
}

// ---------------------------------------------------------------------------
// Dates and time
// ---------------------------------------------------------------------------

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub const WEEKDAYS_SHORT: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn now_timestamp() -> i64 {
    Local::now().timestamp()
}

pub fn now_minutes() -> u32 {
    let now = Local::now();
    now.hour() * 60 + now.minute()
}

pub fn format_minutes(minutes: u32) -> String {
    format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60)
}

pub fn parse_minutes(raw: &str) -> Option<u32> {
    let (hour, minute) = raw.split_once(':')?;
    let hour: u32 = hour.trim().parse().ok()?;
    let minute: u32 = minute.trim().parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct YearMonth {
    pub year: i32,
    pub month: u32,
}

impl YearMonth {
    pub fn current() -> Self {
        Self::from_date(today())
    }

    pub fn from_date(date: NaiveDate) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
        }
    }

    pub fn first_day(self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, self.month, 1).unwrap_or_else(today)
    }

    pub fn shifted(self, months: i32) -> Self {
        let start = self.first_day();
        let moved = if months >= 0 {
            start.checked_add_months(Months::new(months as u32))
        } else {
            start.checked_sub_months(Months::new((-months) as u32))
        }
        .unwrap_or(start);
        Self::from_date(moved)
    }

    pub fn contains(self, date: NaiveDate) -> bool {
        date.year() == self.year && date.month() == self.month
    }

    pub fn label(self) -> String {
        format!("{} {}", MONTHS[(self.month - 1) as usize], self.year)
    }

    pub fn short_label(self) -> &'static str {
        MONTHS_SHORT[(self.month - 1) as usize]
    }

    pub fn index(self) -> i32 {
        self.year * 12 + (self.month as i32 - 1)
    }

    /// Number of days in this month.
    pub fn days(self) -> u32 {
        let next = if self.month == 12 {
            NaiveDate::from_ymd_opt(self.year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(self.year, self.month + 1, 1)
        };
        next.and_then(|date| date.pred_opt())
            .map(|date| date.day())
            .unwrap_or(30)
    }
}

pub fn friendly_date(date: NaiveDate) -> String {
    let now = today();
    if date == now {
        "Today".to_owned()
    } else if Some(date) == now.pred_opt() {
        "Yesterday".to_owned()
    } else {
        format!(
            "{}, {} {}",
            WEEKDAYS_SHORT[date.weekday().num_days_from_monday() as usize],
            MONTHS_SHORT[(date.month() - 1) as usize],
            date.day()
        )
    }
}

pub fn long_date(date: NaiveDate) -> String {
    format!(
        "{} {} {}",
        MONTHS[(date.month() - 1) as usize],
        date.day(),
        date.year()
    )
}

/// The local clock time a transaction happened, falling back to the moment it
/// was recorded for entries created before the time field existed.
pub fn transaction_minutes(tx: &Transaction) -> u32 {
    tx.time.unwrap_or_else(|| {
        Local
            .timestamp_opt(tx.created_at, 0)
            .single()
            .map(|date| date.hour() * 60 + date.minute())
            .unwrap_or(0)
    })
}

pub fn transaction_time_label(tx: &Transaction) -> String {
    format_minutes(transaction_minutes(tx))
}

// ---------------------------------------------------------------------------
// Accounts and transactions
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AccountKind {
    #[default]
    Cash,
    Bank,
    Card,
    Savings,
}

impl AccountKind {
    pub const ALL: [AccountKind; 4] = [
        AccountKind::Cash,
        AccountKind::Bank,
        AccountKind::Card,
        AccountKind::Savings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AccountKind::Cash => "Cash",
            AccountKind::Bank => "Bank",
            AccountKind::Card => "Card",
            AccountKind::Savings => "Savings",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            AccountKind::Cash => "Everyday cash",
            AccountKind::Bank => "Checking account",
            AccountKind::Card => "Credit or debit",
            AccountKind::Savings => "Savings & goals",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    pub id: u64,
    pub name: String,
    pub kind: AccountKind,
    pub color: u8,
    pub opening: Minor,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TxKind {
    Expense,
    Income,
}

impl TxKind {
    pub fn label(self) -> &'static str {
        match self {
            TxKind::Expense => "Expense",
            TxKind::Income => "Income",
        }
    }
}

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

/// Stable identifier for a category. Built-ins occupy `1..=14`; custom
/// categories continue from there. Deserialisation accepts the old string
/// names so existing state files keep working.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct CategoryId(pub u64);

impl Serialize for CategoryId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for CategoryId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdVisitor;

        impl Visitor<'_> for IdVisitor {
            type Value = CategoryId;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a category id or a legacy category name")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(CategoryId(value))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(CategoryId(value.max(0) as u64))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(CategoryId(legacy_category_id(value)))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(IdVisitor)
    }
}

fn legacy_category_id(name: &str) -> u64 {
    match name {
        "Food" => 1,
        "Shopping" => 2,
        "Transport" => 3,
        "Bills" => 4,
        "Health" => 5,
        "Fun" => 6,
        "Home" => 7,
        "Education" => 8,
        "Travel" => 9,
        "Other" => 10,
        "Salary" => 11,
        "Gift" => 12,
        "Refund" => 13,
        _ => 10,
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum CategoryIcon {
    #[default]
    Tag,
    Food,
    Shopping,
    Transport,
    Bills,
    Health,
    Fun,
    Home,
    Education,
    Travel,
    Salary,
    Gift,
    Refund,
    Other,
}

impl CategoryIcon {
    pub const ALL: [CategoryIcon; 14] = [
        CategoryIcon::Tag,
        CategoryIcon::Food,
        CategoryIcon::Shopping,
        CategoryIcon::Transport,
        CategoryIcon::Bills,
        CategoryIcon::Health,
        CategoryIcon::Fun,
        CategoryIcon::Home,
        CategoryIcon::Education,
        CategoryIcon::Travel,
        CategoryIcon::Salary,
        CategoryIcon::Gift,
        CategoryIcon::Refund,
        CategoryIcon::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CategoryIcon::Tag => "Tag",
            CategoryIcon::Food => "Food",
            CategoryIcon::Shopping => "Shopping",
            CategoryIcon::Transport => "Transport",
            CategoryIcon::Bills => "Bills",
            CategoryIcon::Health => "Health",
            CategoryIcon::Fun => "Fun",
            CategoryIcon::Home => "Home",
            CategoryIcon::Education => "Education",
            CategoryIcon::Travel => "Travel",
            CategoryIcon::Salary => "Salary",
            CategoryIcon::Gift => "Gift",
            CategoryIcon::Refund => "Refund",
            CategoryIcon::Other => "Other",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Category {
    pub id: u64,
    pub name: String,
    pub icon: CategoryIcon,
    pub color: u8,
    pub kind: TxKind,
    #[serde(default)]
    pub builtin: bool,
}

pub const OTHER_EXPENSE: u64 = 10;
pub const OTHER_INCOME: u64 = 14;
pub const CUSTOM_START: u64 = 15;

fn builtin_categories() -> Vec<Category> {
    let spec: [(u64, &str, CategoryIcon, u8, TxKind); 14] = [
        (1, "Food", CategoryIcon::Food, 0, TxKind::Expense),
        (2, "Shopping", CategoryIcon::Shopping, 1, TxKind::Expense),
        (3, "Transport", CategoryIcon::Transport, 2, TxKind::Expense),
        (4, "Bills", CategoryIcon::Bills, 3, TxKind::Expense),
        (5, "Health", CategoryIcon::Health, 4, TxKind::Expense),
        (6, "Fun", CategoryIcon::Fun, 5, TxKind::Expense),
        (7, "Home", CategoryIcon::Home, 6, TxKind::Expense),
        (8, "Education", CategoryIcon::Education, 7, TxKind::Expense),
        (9, "Travel", CategoryIcon::Travel, 8, TxKind::Expense),
        (10, "Other", CategoryIcon::Other, 9, TxKind::Expense),
        (11, "Salary", CategoryIcon::Salary, 10, TxKind::Income),
        (12, "Gift", CategoryIcon::Gift, 11, TxKind::Income),
        (13, "Refund", CategoryIcon::Refund, 12, TxKind::Income),
        (14, "Other income", CategoryIcon::Other, 9, TxKind::Income),
    ];
    spec.into_iter()
        .map(|(id, name, icon, color, kind)| Category {
            id,
            name: name.to_owned(),
            icon,
            color,
            kind,
            builtin: true,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Navigation preference
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NavMode {
    #[default]
    Gestures,
    Buttons,
}

impl NavMode {
    pub fn label(self) -> &'static str {
        match self {
            NavMode::Gestures => "Gestures",
            NavMode::Buttons => "Buttons",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            NavMode::Gestures => NavMode::Buttons,
            NavMode::Buttons => NavMode::Gestures,
        }
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Transaction {
    pub id: u64,
    pub account_id: u64,
    pub kind: TxKind,
    pub amount: Minor,
    #[serde(default)]
    pub category: CategoryId,
    #[serde(default)]
    pub note: String,
    pub date: NaiveDate,
    pub created_at: i64,
    /// Minutes since local midnight. Older entries fall back to `created_at`.
    #[serde(default)]
    pub time: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub currency: Currency,
    #[serde(default = "default_next_id")]
    pub next_account_id: u64,
    #[serde(default = "default_next_id")]
    pub next_tx_id: u64,
    #[serde(default = "default_custom_start")]
    pub next_category_id: u64,
    /// Monthly spending budget in minor units. Zero means "not set yet".
    #[serde(default)]
    pub monthly_budget: Minor,
    /// Day of the month the budget cycle starts again (1–28). `1` means a
    /// normal calendar month; older state files migrate to that behaviour.
    #[serde(default = "default_reset_day")]
    pub budget_reset_day: u8,
    #[serde(default)]
    pub last_category: CategoryId,
    #[serde(default)]
    pub last_account: u64,
    #[serde(default)]
    pub nav: NavMode,
    /// New installs start with onboarding; existing state files default to
    /// `true` so upgrades never show it again.
    #[serde(default = "default_onboarded")]
    pub onboarded: bool,
    #[serde(default = "default_true")]
    pub haptics: bool,
}

fn default_onboarded() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_version() -> u32 {
    1
}

fn default_next_id() -> u64 {
    1
}

fn default_custom_start() -> u64 {
    CUSTOM_START
}

fn default_reset_day() -> u8 {
    1
}

/// `1st`, `2nd`, `3rd`, `4th`, … for labels.
pub fn ordinal(day: u8) -> String {
    let value = day as u32;
    let suffix = match value % 100 {
        11..=13 => "th",
        _ => match value % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{value}{suffix}")
}

impl Default for AppState {
    fn default() -> Self {
        let mut state = Self {
            version: 1,
            accounts: Vec::new(),
            transactions: Vec::new(),
            categories: builtin_categories(),
            currency: Currency::default(),
            next_account_id: 1,
            next_tx_id: 1,
            next_category_id: CUSTOM_START,
            monthly_budget: 0,
            budget_reset_day: 1,
            last_category: CategoryId(1),
            last_account: 0,
            nav: NavMode::Gestures,
            onboarded: false,
            haptics: true,
        };
        state.ensure_seed_account();
        state
    }
}

impl AppState {
    pub fn normalize(&mut self) {
        if self.accounts.is_empty() {
            self.ensure_seed_account();
        }
        // Built-ins are always present so old files and new installs agree.
        for category in builtin_categories() {
            if !self.categories.iter().any(|c| c.id == category.id) {
                self.categories.push(category);
            }
        }
        self.categories.sort_by_key(|c| c.id);
        let max_account = self.accounts.iter().map(|a| a.id).max().unwrap_or(0);
        self.next_account_id = self.next_account_id.max(max_account + 1);
        let max_tx = self.transactions.iter().map(|t| t.id).max().unwrap_or(0);
        self.next_tx_id = self.next_tx_id.max(max_tx + 1);
        let max_category = self.categories.iter().map(|c| c.id).max().unwrap_or(0);
        self.next_category_id = self
            .next_category_id
            .max(max_category + 1)
            .max(CUSTOM_START);

        self.monthly_budget = self.monthly_budget.max(0);
        // 28 is safe in every month, so cycles never lose a day to clamping.
        self.budget_reset_day = self.budget_reset_day.clamp(1, 28);
        let known: Vec<u64> = self.categories.iter().map(|category| category.id).collect();
        for tx in &mut self.transactions {
            if tx.amount < 0 {
                tx.amount = -tx.amount;
            }
            if tx.category.0 == 0 || !known.contains(&tx.category.0) {
                tx.category = CategoryId(match tx.kind {
                    TxKind::Expense => OTHER_EXPENSE,
                    TxKind::Income => OTHER_INCOME,
                });
            }
        }

        if self.last_category.0 == 0
            || self
                .category(self.last_category.0)
                .map(|category| category.kind != TxKind::Expense)
                .unwrap_or(true)
        {
            self.last_category = CategoryId(
                self.sorted_categories(TxKind::Expense)
                    .first()
                    .map(|c| c.id)
                    .unwrap_or(1),
            );
        }
        if self.last_account == 0 || self.account(self.last_account).is_none() {
            self.last_account = self.accounts.first().map(|a| a.id).unwrap_or(1);
        }
    }

    fn ensure_seed_account(&mut self) {
        if self.accounts.is_empty() {
            let id = self.next_account_id.max(1);
            self.accounts.push(Account {
                id,
                name: "Cash".to_owned(),
                kind: AccountKind::Cash,
                color: 0,
                opening: 0,
                created_at: now_timestamp(),
            });
            self.next_account_id = id + 1;
        }
    }

    pub fn account(&self, id: u64) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn account_name(&self, id: u64) -> &str {
        self.account(id)
            .map(|a| a.name.as_str())
            .unwrap_or("Account")
    }

    pub fn balance(&self, account_id: u64) -> Minor {
        let opening = self.account(account_id).map(|a| a.opening).unwrap_or(0);
        self.transactions
            .iter()
            .filter(|tx| tx.account_id == account_id)
            .fold(opening, |balance, tx| match tx.kind {
                TxKind::Expense => balance - tx.amount,
                TxKind::Income => balance + tx.amount,
            })
    }

    pub fn total_balance(&self) -> Minor {
        self.accounts
            .iter()
            .map(|account| self.balance(account.id))
            .sum()
    }

    // Categories ------------------------------------------------------------

    pub fn category(&self, id: u64) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == id)
    }

    pub fn category_name(&self, id: CategoryId) -> &str {
        self.category(id.0)
            .map(|category| category.name.as_str())
            .unwrap_or("Other")
    }

    pub fn categories_for(&self, kind: TxKind) -> Vec<&Category> {
        self.categories
            .iter()
            .filter(|category| category.kind == kind)
            .collect()
    }

    /// Categories ordered by how often they are used, then by name.
    pub fn sorted_categories(&self, kind: TxKind) -> Vec<&Category> {
        let mut list = self.categories_for(kind);
        list.sort_by(|a, b| {
            self.category_usage(b.id)
                .cmp(&self.category_usage(a.id))
                .then_with(|| a.name.cmp(&b.name))
        });
        list
    }

    pub fn category_usage(&self, id: u64) -> usize {
        self.transactions
            .iter()
            .filter(|tx| tx.category.0 == id)
            .count()
    }

    pub fn add_category(&mut self, name: &str, icon: CategoryIcon, kind: TxKind) -> u64 {
        let id = self.next_category_id.max(CUSTOM_START);
        self.next_category_id = id + 1;
        let color = (id % 8) as u8;
        self.categories.push(Category {
            id,
            name: name.to_owned(),
            icon,
            color,
            kind,
            builtin: false,
        });
        id
    }

    pub fn delete_category(&mut self, id: u64) {
        let Some(category) = self.category(id) else {
            return;
        };
        if category.builtin {
            return;
        }
        let fallback = match category.kind {
            TxKind::Expense => OTHER_EXPENSE,
            TxKind::Income => OTHER_INCOME,
        };
        for tx in &mut self.transactions {
            if tx.category.0 == id {
                tx.category = CategoryId(fallback);
            }
        }
        self.categories.retain(|category| category.id != id);
        if self.last_category.0 == id {
            self.last_category = CategoryId(fallback);
        }
    }

    // Totals ----------------------------------------------------------------

    pub fn totals_for(&self, month: YearMonth) -> Totals {
        let mut totals = Totals::default();
        for tx in self
            .transactions
            .iter()
            .filter(|tx| month.contains(tx.date))
        {
            match tx.kind {
                TxKind::Expense => totals.spent += tx.amount,
                TxKind::Income => totals.income += tx.amount,
            }
        }
        totals
    }

    pub fn category_totals(&self, month: YearMonth, kind: TxKind) -> Vec<(CategoryId, Minor)> {
        let mut totals: Vec<(CategoryId, Minor)> = Vec::new();
        for tx in self
            .transactions
            .iter()
            .filter(|tx| tx.kind == kind && month.contains(tx.date))
        {
            if let Some(entry) = totals
                .iter_mut()
                .find(|(category, _)| *category == tx.category)
            {
                entry.1 += tx.amount;
            } else {
                totals.push((tx.category, tx.amount));
            }
        }
        totals.sort_by(|a, b| b.1.cmp(&a.1));
        totals
    }

    pub fn transactions_for_month(&self, month: YearMonth) -> Vec<&Transaction> {
        let mut list: Vec<&Transaction> = self
            .transactions
            .iter()
            .filter(|tx| month.contains(tx.date))
            .collect();
        list.sort_by(|a, b| {
            b.date
                .cmp(&a.date)
                .then(b.created_at.cmp(&a.created_at))
                .then(b.id.cmp(&a.id))
        });
        list
    }

    pub fn recent(&self, limit: usize) -> Vec<&Transaction> {
        let mut list: Vec<&Transaction> = self.transactions.iter().collect();
        list.sort_by(|a, b| {
            b.date
                .cmp(&a.date)
                .then(b.created_at.cmp(&a.created_at))
                .then(b.id.cmp(&a.id))
        });
        list.truncate(limit);
        list
    }

    pub fn account_transaction_count(&self, account_id: u64) -> usize {
        self.transactions
            .iter()
            .filter(|tx| tx.account_id == account_id)
            .count()
    }

    pub fn add_transaction(&mut self, tx: Transaction) {
        self.transactions.push(tx);
    }

    pub fn add_account(&mut self, account: Account) {
        self.accounts.push(account);
    }

    pub fn delete_account(&mut self, account_id: u64) {
        self.accounts.retain(|account| account.id != account_id);
        self.transactions.retain(|tx| tx.account_id != account_id);
    }

    pub fn monthly_totals(&self, end: YearMonth, count: usize) -> Vec<(YearMonth, Totals)> {
        let mut out = Vec::with_capacity(count);
        for i in (0..count).rev() {
            let month = end.shifted(-(i as i32));
            out.push((month, self.totals_for(month)));
        }
        out
    }

    pub fn previous_totals(&self, month: YearMonth) -> Totals {
        self.totals_for(month.shifted(-1))
    }

    pub fn account_flow(&self, month: YearMonth, account_id: u64) -> Totals {
        let mut totals = Totals::default();
        for tx in self
            .transactions
            .iter()
            .filter(|tx| tx.account_id == account_id && month.contains(tx.date))
        {
            match tx.kind {
                TxKind::Expense => totals.spent += tx.amount,
                TxKind::Income => totals.income += tx.amount,
            }
        }
        totals
    }

    pub fn top_expenses(&self, month: YearMonth, limit: usize) -> Vec<&Transaction> {
        let mut list: Vec<&Transaction> = self
            .transactions
            .iter()
            .filter(|tx| tx.kind == TxKind::Expense && month.contains(tx.date))
            .collect();
        list.sort_by(|a, b| b.amount.cmp(&a.amount).then(b.date.cmp(&a.date)));
        list.truncate(limit);
        list
    }

    pub fn active_days(&self, month: YearMonth) -> usize {
        let mut days: Vec<NaiveDate> = self
            .transactions
            .iter()
            .filter(|tx| month.contains(tx.date))
            .map(|tx| tx.date)
            .collect();
        days.sort();
        days.dedup();
        days.len()
    }

    // Daily budget ----------------------------------------------------------

    pub fn spent_on(&self, date: NaiveDate) -> Minor {
        self.transactions
            .iter()
            .filter(|tx| tx.kind == TxKind::Expense && tx.date == date)
            .map(|tx| tx.amount)
            .sum()
    }

    pub fn spent_today(&self) -> Minor {
        self.spent_on(today())
    }

    /// The current budget cycle as `[start, end)` for any date. A reset day of
    /// `1` gives the calendar month; `15` gives the 15th through the next 14th.
    pub fn budget_period_for(&self, date: NaiveDate) -> (NaiveDate, NaiveDate) {
        let day = self.budget_reset_day.clamp(1, 28) as u32;
        let month_start = date.with_day(1).unwrap_or(date);
        let this_start =
            NaiveDate::from_ymd_opt(date.year(), date.month(), day).unwrap_or(month_start);
        let start = if date >= this_start {
            this_start
        } else {
            let previous_month = month_start
                .checked_sub_months(Months::new(1))
                .unwrap_or(month_start);
            NaiveDate::from_ymd_opt(previous_month.year(), previous_month.month(), day)
                .unwrap_or(previous_month)
        };
        let end = start
            .checked_add_months(Months::new(1))
            .unwrap_or_else(|| start + chrono::Duration::days(30));
        (start, end)
    }

    pub fn budget_period(&self) -> (NaiveDate, NaiveDate) {
        self.budget_period_for(today())
    }

    pub fn budget_days_for(&self, date: NaiveDate) -> u32 {
        let (start, end) = self.budget_period_for(date);
        (end - start).num_days().max(1) as u32
    }

    /// Days left in the cycle, including `date` itself.
    pub fn budget_days_left_for(&self, date: NaiveDate) -> u32 {
        let (_, end) = self.budget_period_for(date);
        (end - date).num_days().max(1) as u32
    }

    pub fn budget_days_left(&self) -> u32 {
        self.budget_days_left_for(today())
    }

    /// Everything spent in the active cycle, including today.
    pub fn spent_in_period_for(&self, date: NaiveDate) -> Minor {
        let (start, end) = self.budget_period_for(date);
        self.transactions
            .iter()
            .filter(|tx| tx.kind == TxKind::Expense && tx.date >= start && tx.date < end)
            .map(|tx| tx.amount)
            .sum()
    }

    pub fn spent_in_period(&self) -> Minor {
        self.spent_in_period_for(today())
    }

    /// Spending in the cycle before today — today's allowance is fixed at the
    /// start of the day so the wave can still rise as today's entries land.
    pub fn spent_before_on(&self, date: NaiveDate) -> Minor {
        let (start, _) = self.budget_period_for(date);
        self.transactions
            .iter()
            .filter(|tx| tx.kind == TxKind::Expense && tx.date >= start && tx.date < date)
            .map(|tx| tx.amount)
            .sum()
    }

    pub fn remaining_budget_for(&self, date: NaiveDate) -> Minor {
        self.monthly_budget - self.spent_in_period_for(date)
    }

    pub fn remaining_budget(&self) -> Minor {
        self.remaining_budget_for(today())
    }

    /// Today's allowance: unspent budget divided by the days still left in the
    /// current cycle. Spent earlier in the month is not charged to today again.
    pub fn daily_budget_for(&self, date: NaiveDate) -> Minor {
        let left = self.budget_days_left_for(date).max(1) as i64;
        let unspent = (self.monthly_budget - self.spent_before_on(date)).max(0);
        unspent / left
    }

    pub fn daily_budget(&self) -> Minor {
        self.daily_budget_for(today())
    }

    /// The number shown on Home: unspent budget spread over the days still
    /// left in the cycle. Today's spending is already inside `spent_in_period`,
    /// so it must not be subtracted a second time here.
    pub fn available_today_for(&self, date: NaiveDate) -> Minor {
        let left = self.budget_days_left_for(date).max(1) as i64;
        self.remaining_budget_for(date) / left
    }

    pub fn available_today(&self) -> Minor {
        self.available_today_for(today())
    }

    /// How much of today's baseline allowance has been spent, clamped to `0..=1`.
    /// This drives the wave; it is deliberately separate from the Home number,
    /// which can be higher after underspending earlier in the cycle.
    pub fn spent_fraction_today(&self) -> f32 {
        let budget = self.daily_budget();
        if budget <= 0 {
            0.0
        } else {
            (self.spent_today() as f32 / budget as f32).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Totals {
    pub spent: Minor,
    pub income: Minor,
}

impl Totals {
    pub fn saved(self) -> Minor {
        self.income - self.spent
    }
}

pub fn percent_change(current: Minor, previous: Minor) -> Option<f32> {
    if previous <= 0 {
        None
    } else {
        Some((current - previous) as f32 * 100.0 / previous as f32)
    }
}

/// Current clock time as a `NaiveTime`, used by tests.
pub fn now_time() -> NaiveTime {
    Local::now().time()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_money() {
        assert_eq!(Currency::Usd.format_abs(124_850), "$1,248.50");
        assert_eq!(Currency::Usd.format(-50), "-$0.50");
        assert_eq!(Currency::Jpy.format_abs(1_250), "¥1,250");
        assert_eq!(Currency::Inr.format_abs(1_00_000_00), "₹1,00,000.00");
    }

    #[test]
    fn parses_amounts() {
        assert_eq!(parse_amount("12.34", 2), Some(1234));
        assert_eq!(parse_amount("12.", 2), Some(1200));
        assert_eq!(parse_amount(".5", 2), Some(50));
        assert_eq!(parse_amount("1,234.5", 2), Some(123450));
        assert_eq!(parse_amount("12.345", 2), Some(1234));
        assert_eq!(parse_amount("", 2), None);
        assert_eq!(parse_amount("-2", 2), None);
    }

    #[test]
    fn month_shifts() {
        let jan = YearMonth {
            year: 2026,
            month: 1,
        };
        assert_eq!(jan.shifted(-1).year, 2025);
        assert_eq!(jan.shifted(-1).month, 12);
        assert_eq!(jan.shifted(2).month, 3);
        assert_eq!(jan.days(), 31);
        assert_eq!(
            YearMonth {
                year: 2024,
                month: 2
            }
            .days(),
            29
        );
    }

    #[test]
    fn migrates_legacy_categories() {
        let json = r#"{
            "version": 1,
            "accounts": [],
            "transactions": [{
                "id": 1,
                "account_id": 1,
                "kind": "Expense",
                "amount": 500,
                "category": "Food",
                "note": "",
                "date": "2026-01-01",
                "created_at": 0
            }],
            "currency": "Inr",
            "next_account_id": 1,
            "next_tx_id": 2
        }"#;
        let mut state: AppState = serde_json::from_str(json).unwrap();
        state.normalize();
        assert_eq!(state.transactions[0].category, CategoryId(1));
        assert!(state.categories.len() >= 14);
        assert!(state.accounts.len() == 1);
    }

    #[test]
    fn budget_cycles_and_remaining_days() {
        let mut state = AppState::default();
        state.monthly_budget = 30_000; // ₹300.00
        state.budget_reset_day = 15;
        let date = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();

        let (start, end) = state.budget_period_for(date);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 11, 15).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 12, 15).unwrap());
        assert_eq!(state.budget_days_for(date), 30);
        assert_eq!(state.budget_days_left_for(date), 23);

        state.transactions.push(Transaction {
            id: 1,
            account_id: 1,
            kind: TxKind::Expense,
            amount: 5_000,
            category: CategoryId(1),
            note: String::new(),
            date: NaiveDate::from_ymd_opt(2026, 11, 18).unwrap(),
            created_at: 0,
            time: Some(0),
        });
        assert_eq!(state.spent_before_on(date), 5_000);
        assert_eq!(state.daily_budget_for(date), (30_000 - 5_000) / 23);

        state.transactions.push(Transaction {
            id: 2,
            account_id: 1,
            kind: TxKind::Expense,
            amount: 3_000,
            category: CategoryId(1),
            note: String::new(),
            date,
            created_at: 0,
            time: Some(0),
        });
        // Today's spend is already in the remaining budget; it is not charged twice.
        assert_eq!(
            state.available_today_for(date),
            (30_000 - 5_000 - 3_000) / 23
        );

        // A reset on the 1st is the ordinary calendar month.
        state.budget_reset_day = 1;
        let (start, end) = state.budget_period_for(date);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 12, 1).unwrap());
        assert_eq!(state.budget_days_left_for(date), 9);
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(22), "22nd");
        assert_eq!(ordinal(13), "13th");
    }

    #[test]
    fn parses_time() {
        assert_eq!(parse_minutes("09:05"), Some(545));
        assert_eq!(parse_minutes("23:59"), Some(1439));
        assert_eq!(parse_minutes("24:00"), None);
        assert_eq!(format_minutes(545), "09:05");
    }
}
