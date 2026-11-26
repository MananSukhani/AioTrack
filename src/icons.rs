//! Iconography.
//!
//! Every glyph is a Lucide icon (ISC licence), rasterised once into an 8-bit
//! coverage mask and tinted at paint time. Using one authoritative set keeps
//! the stroke weight, corner radius and optical size identical everywhere.

use eframe::egui::{
    pos2, Color32, ColorImage, Context, Id, Painter, Rect, TextureFilter, TextureHandle,
    TextureOptions, TextureWrapMode,
};

use crate::icon_assets as assets;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Home,
    Activity,
    Accounts,
    Insights,
    Plus,
    Settings,
    Back,
    Close,
    Check,
    Trash,
    ChevronLeft,
    ChevronRight,
    ArrowUpRight,
    ArrowDownLeft,
    Backspace,
    Calendar,
    Sparkle,
    Info,
    Edit,
    Wallet,
    Food,
    Shopping,
    Transport,
    Bills,
    Health,
    Fun,
    House,
    Education,
    Travel,
    Briefcase,
    Gift,
    Refund,
    More,
    Sun,
    Moon,
    User,
    Shield,
    Database,
    ArrowRight,
    TrendUp,
    TrendDown,
    Banknote,
    Landmark,
    CreditCard,
    PiggyBank,
    Coins,
    BadgeCheck,
    CheckCircle,
    ChartColumn,
    Sliders,
    ListFilter,
    Lock,
    Phone,
    Target,
    Help,
    Tag,
}

impl Icon {
    fn key(self) -> &'static str {
        match self {
            Icon::Home => "home",
            Icon::Activity => "activity",
            Icon::Accounts => "accounts",
            Icon::Insights => "insights",
            Icon::Plus => "plus",
            Icon::Settings => "settings",
            Icon::Back => "back",
            Icon::Close => "close",
            Icon::Check => "check",
            Icon::Trash => "trash",
            Icon::ChevronLeft => "chevron-left",
            Icon::ChevronRight => "chevron-right",
            Icon::ArrowUpRight => "arrow-up-right",
            Icon::ArrowDownLeft => "arrow-down-left",
            Icon::Backspace => "backspace",
            Icon::Calendar => "calendar",
            Icon::Sparkle => "sparkle",
            Icon::Info => "info",
            Icon::Edit => "edit",
            Icon::Wallet => "wallet",
            Icon::Food => "food",
            Icon::Shopping => "shopping",
            Icon::Transport => "transport",
            Icon::Bills => "bills",
            Icon::Health => "health",
            Icon::Fun => "fun",
            Icon::House => "house",
            Icon::Education => "education",
            Icon::Travel => "travel",
            Icon::Briefcase => "briefcase",
            Icon::Gift => "gift",
            Icon::Refund => "refund",
            Icon::More => "more",
            Icon::Sun => "sun",
            Icon::Moon => "moon",
            Icon::User => "user",
            Icon::Shield => "shield",
            Icon::Database => "database",
            Icon::ArrowRight => "arrow-right",
            Icon::TrendUp => "trend-up",
            Icon::TrendDown => "trend-down",
            Icon::Banknote => "banknote",
            Icon::Landmark => "landmark",
            Icon::CreditCard => "credit-card",
            Icon::PiggyBank => "piggy-bank",
            Icon::Coins => "coins",
            Icon::BadgeCheck => "badge-check",
            Icon::CheckCircle => "check-circle",
            Icon::ChartColumn => "chart-column",
            Icon::Sliders => "sliders",
            Icon::ListFilter => "list-filter",
            Icon::Lock => "lock",
            Icon::Phone => "phone",
            Icon::Target => "target",
            Icon::Help => "help",
            Icon::Tag => "tag",
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Icon::Home => assets::HOUSE,
            Icon::Activity => assets::RECEIPT_TEXT,
            Icon::Accounts => assets::WALLET,
            Icon::Insights => assets::CHART_PIE,
            Icon::Plus => assets::PLUS,
            Icon::Settings => assets::SETTINGS,
            Icon::Back => assets::ARROW_LEFT,
            Icon::Close => assets::X,
            Icon::Check => assets::CHECK,
            Icon::Trash => assets::TRASH,
            Icon::ChevronLeft => assets::CHEVRON_LEFT,
            Icon::ChevronRight => assets::CHEVRON_RIGHT,
            Icon::ArrowUpRight => assets::ARROW_UP_RIGHT,
            Icon::ArrowDownLeft => assets::ARROW_DOWN_LEFT,
            Icon::Backspace => assets::DELETE,
            Icon::Calendar => assets::CALENDAR,
            Icon::Sparkle => assets::SPARKLES,
            Icon::Info => assets::INFO,
            Icon::Edit => assets::PEN,
            Icon::Wallet => assets::WALLET_MINIMAL,
            Icon::Food => assets::UTENSILS,
            Icon::Shopping => assets::SHOPPING_BAG,
            Icon::Transport => assets::CAR,
            Icon::Bills => assets::RECEIPT,
            Icon::Health => assets::HEART_PULSE,
            Icon::Fun => assets::GAMEPAD,
            Icon::House => assets::SOFA,
            Icon::Education => assets::GRADUATION_CAP,
            Icon::Travel => assets::PLANE,
            Icon::Briefcase => assets::BRIEFCASE,
            Icon::Gift => assets::GIFT,
            Icon::Refund => assets::ROTATE_CCW,
            Icon::More => assets::MORE,
            Icon::Sun => assets::SUN,
            Icon::Moon => assets::MOON,
            Icon::User => assets::USER,
            Icon::Shield => assets::SHIELD,
            Icon::Database => assets::DATABASE,
            Icon::ArrowRight => assets::ARROW_RIGHT,
            Icon::TrendUp => assets::TREND_UP,
            Icon::TrendDown => assets::TREND_DOWN,
            Icon::Banknote => assets::BANKNOTE,
            Icon::Landmark => assets::LANDMARK,
            Icon::CreditCard => assets::CREDIT_CARD,
            Icon::PiggyBank => assets::PIGGY_BANK,
            Icon::Coins => assets::COINS,
            Icon::BadgeCheck => assets::BADGE_CHECK,
            Icon::CheckCircle => assets::CIRCLE_CHECK,
            Icon::ChartColumn => assets::CHART_COLUMN,
            Icon::Sliders => assets::SLIDERS,
            Icon::ListFilter => assets::LIST_FILTER,
            Icon::Lock => assets::LOCK,
            Icon::Phone => assets::SMARTPHONE,
            Icon::Target => assets::TARGET,
            Icon::Help => assets::HELP,
            Icon::Tag => assets::SHAPES,
        }
    }
}

fn texture_for(ctx: &Context, icon: Icon) -> TextureHandle {
    let id = Id::new(("aiotrack-icon", icon.key()));
    if let Some(handle) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return handle;
    }
    let alpha = icon.bytes();
    let side = assets::ICON_SIZE;
    let mut pixels = Vec::with_capacity(side * side);
    for &coverage in alpha {
        pixels.push(Color32::from_white_alpha(coverage));
    }
    let image = ColorImage {
        size: [side, side],
        pixels,
    };
    let handle = ctx.load_texture(
        format!("lucide-{}", icon.key()),
        image,
        TextureOptions {
            magnification: TextureFilter::Linear,
            minification: TextureFilter::Linear,
            wrap_mode: TextureWrapMode::ClampToEdge,
            mipmap_mode: Some(TextureFilter::Linear),
        },
    );
    ctx.data_mut(|data| data.insert_temp(id, handle.clone()));
    handle
}

/// Paint `icon` inside `rect`, tinted with `color`. The `weight` argument is
/// accepted for source compatibility with older call sites and ignored: Lucide
/// already ships a single, consistent stroke weight.
pub fn paint(painter: &Painter, icon: Icon, rect: Rect, color: Color32, _weight: f32) {
    let texture = texture_for(painter.ctx(), icon);
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        color,
    );
}

pub fn paint_center(
    painter: &Painter,
    icon: Icon,
    center: eframe::egui::Pos2,
    size: f32,
    color: Color32,
) {
    paint(
        painter,
        icon,
        Rect::from_center_size(center, eframe::egui::Vec2::splat(size)),
        color,
        2.0,
    );
}

pub fn category(icon: crate::model::CategoryIcon) -> Icon {
    use crate::model::CategoryIcon as C;
    match icon {
        C::Food => Icon::Food,
        C::Shopping => Icon::Shopping,
        C::Transport => Icon::Transport,
        C::Bills => Icon::Bills,
        C::Health => Icon::Health,
        C::Fun => Icon::Fun,
        C::Home => Icon::House,
        C::Education => Icon::Education,
        C::Travel => Icon::Travel,
        C::Salary => Icon::Briefcase,
        C::Gift => Icon::Gift,
        C::Refund => Icon::Refund,
        C::Other => Icon::More,
        C::Tag => Icon::Tag,
    }
}

pub fn account_kind(kind: crate::model::AccountKind) -> Icon {
    use crate::model::AccountKind as K;
    match kind {
        K::Cash => Icon::Banknote,
        K::Bank => Icon::Landmark,
        K::Card => Icon::CreditCard,
        K::Savings => Icon::PiggyBank,
    }
}
