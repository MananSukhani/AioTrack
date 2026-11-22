//! The visual language of AioTrack.
//!
//! Pure black, pure white, and a single warm wave of colour that only appears
//! when money moves. Everything else is typography and space.

use eframe::egui::style::{WidgetVisuals, Widgets};
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Visuals,
};
use std::collections::BTreeMap;
use std::sync::Arc;

// Type scale ----------------------------------------------------------------
pub const FS_MICRO: f32 = 10.0;
pub const FS_CAPTION: f32 = 12.0;
pub const FS_BODY: f32 = 14.0;
pub const FS_TITLE: f32 = 17.0;
pub const FS_HEADLINE: f32 = 26.0;
pub const FS_DISPLAY: f32 = 58.0;

// Rhythm --------------------------------------------------------------------
pub const PAGE_PAD: f32 = 22.0;
pub const TOP_PAD: f32 = 54.0;
pub const BOTTOM_PAD: f32 = 42.0;
pub const CONTENT_MAX: f32 = 520.0;

// Palette -------------------------------------------------------------------
pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);

pub fn bg() -> Color32 {
    BLACK
}

pub fn surface() -> Color32 {
    Color32::from_rgb(8, 8, 8)
}

pub fn raised() -> Color32 {
    Color32::from_rgb(17, 17, 18)
}

pub fn line() -> Color32 {
    Color32::from_rgb(38, 38, 40)
}

pub fn line_soft() -> Color32 {
    Color32::from_rgb(24, 24, 26)
}

pub fn ink() -> Color32 {
    WHITE
}

pub fn muted() -> Color32 {
    Color32::from_rgb(148, 148, 153)
}

pub fn faint() -> Color32 {
    Color32::from_rgb(92, 92, 97)
}

pub fn good() -> Color32 {
    Color32::from_rgb(57, 217, 138)
}

pub fn bad() -> Color32 {
    Color32::from_rgb(255, 92, 92)
}

pub fn alpha(color: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

/// Green → yellow → orange → red as today's budget drains away.
pub fn wave_color(fraction: f32) -> Color32 {
    let t = fraction.clamp(0.0, 1.0);
    // Bright, evenly-spaced stops for a confident wash of colour.
    let green = Color32::from_rgb(0x2F, 0xD5, 0x8A);
    let yellow = Color32::from_rgb(0xF2, 0xC9, 0x4C);
    let orange = Color32::from_rgb(0xF2, 0x99, 0x4A);
    let red = Color32::from_rgb(0xEB, 0x57, 0x57);
    if t < 0.42 {
        mix(green, yellow, t / 0.42)
    } else if t < 0.72 {
        mix(yellow, orange, (t - 0.42) / 0.30)
    } else {
        mix(orange, red, (t - 0.72) / 0.28)
    }
}

pub fn category_color(index: u8) -> Color32 {
    const COLORS: [Color32; 10] = [
        Color32::from_rgb(0xFF, 0xFF, 0xFF),
        Color32::from_rgb(0xC7, 0xC7, 0xCC),
        Color32::from_rgb(0x8E, 0x8E, 0x93),
        Color32::from_rgb(0xF2, 0xC9, 0x4C),
        Color32::from_rgb(0xF2, 0x99, 0x4A),
        Color32::from_rgb(0xEB, 0x57, 0x57),
        Color32::from_rgb(0x6F, 0xCF, 0x97),
        Color32::from_rgb(0x56, 0xCC, 0xF2),
        Color32::from_rgb(0xBB, 0x6B, 0xD9),
        Color32::from_rgb(0xBD, 0xB2, 0xFF),
    ];
    COLORS[index as usize % COLORS.len()]
}

/// A calmer, independent palette for the analysis donut. Category chips stay
/// pure white; only the chart uses colour, so adjacent slices never collapse
/// into the same grey as the background.
pub fn pie_color(index: usize) -> Color32 {
    const COLORS: [Color32; 10] = [
        Color32::from_rgb(0x6E, 0xE7, 0xB7),
        Color32::from_rgb(0x60, 0xA5, 0xFA),
        Color32::from_rgb(0xFB, 0xBF, 0x24),
        Color32::from_rgb(0xF4, 0x72, 0xB6),
        Color32::from_rgb(0xA7, 0x8B, 0xFA),
        Color32::from_rgb(0x34, 0xD3, 0x99),
        Color32::from_rgb(0xF8, 0x71, 0x71),
        Color32::from_rgb(0x22, 0xD3, 0xEE),
        Color32::from_rgb(0xFC, 0xD3, 0x4D),
        Color32::from_rgb(0xC4, 0xB5, 0xFD),
    ];
    COLORS[index % COLORS.len()]
}

pub fn account_color(index: u8) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(0xFF, 0xFF, 0xFF),
        Color32::from_rgb(0x6F, 0xCF, 0x97),
        Color32::from_rgb(0xF2, 0xC9, 0x4C),
        Color32::from_rgb(0x56, 0xCC, 0xF2),
        Color32::from_rgb(0xBB, 0x6B, 0xD9),
        Color32::from_rgb(0xF2, 0x99, 0x4A),
    ];
    COLORS[index as usize % COLORS.len()]
}

pub fn account_palette() -> [Color32; 6] {
    [
        account_color(0),
        account_color(1),
        account_color(2),
        account_color(3),
        account_color(4),
        account_color(5),
    ]
}

// Typography -----------------------------------------------------------------
pub fn sans() -> FontFamily {
    FontFamily::Name("sans".into())
}

pub fn sans_medium() -> FontFamily {
    FontFamily::Name("sans-md".into())
}

pub fn sans_semibold() -> FontFamily {
    FontFamily::Name("sans-semi".into())
}

pub fn sans_bold() -> FontFamily {
    FontFamily::Name("sans-bold".into())
}

/// Big numbers use the bold cut.
pub fn display() -> FontFamily {
    FontFamily::Name("sans-bold".into())
}

pub fn font(size: f32, family: FontFamily) -> FontId {
    FontId::new(size, family)
}

pub fn sans_font(size: f32) -> FontId {
    font(size, sans())
}

pub fn medium(size: f32) -> FontId {
    font(size, sans_medium())
}

pub fn semibold(size: f32) -> FontId {
    font(size, sans_semibold())
}

pub fn bold(size: f32) -> FontId {
    font(size, sans_bold())
}

pub fn display_font(size: f32) -> FontId {
    font(size, display())
}

// Install --------------------------------------------------------------------

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let font_data: [(&str, &'static [u8]); 4] = [
        (
            "inter-regular",
            include_bytes!("../assets/fonts/Inter-Regular.ttf"),
        ),
        (
            "inter-medium",
            include_bytes!("../assets/fonts/Inter-Medium.ttf"),
        ),
        (
            "inter-semibold",
            include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
        ),
        (
            "inter-bold",
            include_bytes!("../assets/fonts/Inter-Bold.ttf"),
        ),
    ];
    for (name, bytes) in font_data {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    let family = |primary: &str| {
        let mut list = vec![primary.to_owned()];
        list.extend(fallback.iter().cloned());
        list
    };

    fonts
        .families
        .insert(FontFamily::Proportional, family("inter-regular"));
    fonts
        .families
        .insert(FontFamily::Name("sans".into()), family("inter-regular"));
    fonts
        .families
        .insert(FontFamily::Name("sans-md".into()), family("inter-medium"));
    fonts.families.insert(
        FontFamily::Name("sans-semi".into()),
        family("inter-semibold"),
    );
    fonts
        .families
        .insert(FontFamily::Name("sans-bold".into()), family("inter-bold"));
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals();
    style.animation_time = 0.20;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 10.0);
    style.spacing.interact_size = egui::vec2(48.0, 48.0);
    style.spacing.icon_width = 18.0;
    style.spacing.icon_width_inner = 12.0;
    style.spacing.icon_spacing = 8.0;
    style.spacing.scroll.floating = true;
    style.spacing.scroll.bar_width = 0.0;
    style.spacing.scroll.floating_width = 0.0;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.spacing.scroll.dormant_handle_opacity = 0.0;
    style.text_styles = BTreeMap::from([
        (TextStyle::Small, sans_font(FS_CAPTION)),
        (TextStyle::Body, sans_font(FS_BODY)),
        (TextStyle::Button, semibold(FS_BODY)),
        (TextStyle::Heading, semibold(FS_HEADLINE)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]);
    ctx.set_style(style);
}

fn visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.dark_mode = true;
    visuals.override_text_color = Some(WHITE);
    visuals.panel_fill = BLACK;
    visuals.window_fill = BLACK;
    visuals.extreme_bg_color = BLACK;
    visuals.faint_bg_color = surface();
    visuals.code_bg_color = raised();
    visuals.hyperlink_color = WHITE;
    visuals.error_fg_color = bad();
    visuals.warn_fg_color = Color32::from_rgb(0xF2, 0xC9, 0x4C);
    visuals.window_corner_radius = CornerRadius::same(24);
    visuals.menu_corner_radius = CornerRadius::same(20);
    visuals.window_stroke = Stroke::NONE;
    visuals.selection.bg_fill = alpha(WHITE, 46);
    visuals.selection.stroke = Stroke::new(1.0_f32, WHITE);
    visuals.text_cursor.stroke = Stroke::new(2.0_f32, WHITE);

    let base = WidgetVisuals {
        bg_fill: BLACK,
        weak_bg_fill: BLACK,
        bg_stroke: Stroke::NONE,
        fg_stroke: Stroke::new(1.0_f32, WHITE),
        corner_radius: CornerRadius::same(16),
        expansion: 0.0,
    };
    visuals.widgets = Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: BLACK,
            weak_bg_fill: BLACK,
            bg_stroke: Stroke::NONE,
            fg_stroke: Stroke::new(1.0_f32, WHITE),
            corner_radius: CornerRadius::same(16),
            expansion: 0.0,
        },
        inactive: base,
        hovered: WidgetVisuals {
            bg_fill: raised(),
            weak_bg_fill: raised(),
            bg_stroke: Stroke::NONE,
            fg_stroke: Stroke::new(1.0_f32, WHITE),
            corner_radius: CornerRadius::same(16),
            expansion: 0.0,
        },
        active: WidgetVisuals {
            bg_fill: alpha(WHITE, 24),
            weak_bg_fill: alpha(WHITE, 24),
            bg_stroke: Stroke::NONE,
            fg_stroke: Stroke::new(1.0_f32, WHITE),
            corner_radius: CornerRadius::same(16),
            expansion: 0.0,
        },
        open: base,
    };
    visuals
}
