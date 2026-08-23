use std::sync::OnceLock;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use serde::Serialize;
use tauri::image::Image;

use crate::models::WindowsTrayIconStyle;

const CODEX_TOOLS_ICON: Image<'_> = tauri::include_image!("./icons/icon.png");
const TRAY_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/Outfit-SemiBold.ttf");
const SUPERSAMPLE: u32 = 4;
const GLYPH_HORIZONTAL_SCALE: f32 = 0.95;
const WINDOWS_CARD_GLYPH_HORIZONTAL_SCALE: f32 = 0.90;
const WIDE_CARD_GLYPH_HORIZONTAL_SCALE: f32 = 0.84;
const DARK_TEXT: [u8; 4] = [11, 25, 46, 255];
const LIGHT_TEXT: [u8; 4] = [255, 255, 255, 255];
const CARD_EMPTY: [u8; 4] = [48, 56, 77, 255];
const CARD_BORDER_LIGHT: [u8; 4] = [105, 120, 146, 255];
const CARD_BORDER_DARK: [u8; 4] = [135, 152, 183, 255];
const RING_BLUE: [u8; 4] = [24, 102, 218, 255];
const MACOS_CARD_DIGIT_SCALE: f32 = 1.0;
const MACOS_CARD_HUNDRED_DIGIT_SCALE: f32 = 1.3;
const MACOS_CARD_BORDER_WIDTH: f32 = 3.75;

pub(crate) const TRAY_VISUAL_STYLES: [WindowsTrayIconStyle; 5] = [
    WindowsTrayIconStyle::GradientNumberPlate,
    WindowsTrayIconStyle::GradientNumberCard,
    WindowsTrayIconStyle::GradientNumber,
    WindowsTrayIconStyle::NumberProgressBar,
    WindowsTrayIconStyle::LogoProgressRing,
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrayVisualPreview {
    style: WindowsTrayIconStyle,
    data_url: String,
    pixel_width: u32,
    pixel_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayVisualStatus {
    Fresh,
    Stale,
    Error,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayVisualPlatform {
    Windows,
    Macos,
}

pub(crate) fn tray_visual_dimensions(
    style: WindowsTrayIconStyle,
    platform: TrayVisualPlatform,
    base_size: u32,
) -> (u32, u32) {
    let base_size = base_size.max(16);
    if platform == TrayVisualPlatform::Macos {
        match style {
            WindowsTrayIconStyle::GradientNumberCard => (base_size * 4 / 3, base_size),
            WindowsTrayIconStyle::NumberProgressBar => (base_size * 5 / 4, base_size),
            _ => (base_size, base_size),
        }
    } else {
        (base_size, base_size)
    }
}

pub(crate) fn render_tray_visual(
    style: WindowsTrayIconStyle,
    percent: Option<f64>,
    status: TrayVisualStatus,
    light_theme: bool,
    width: u32,
    height: u32,
) -> Image<'static> {
    render_tray_visual_internal(style, percent, status, light_theme, width, height, false)
}

#[cfg(any(target_os = "macos", test))]
pub(crate) fn render_native_macos_tray_visual(
    style: WindowsTrayIconStyle,
    percent: Option<f64>,
    status: TrayVisualStatus,
    light_theme: bool,
    width: u32,
    height: u32,
) -> Image<'static> {
    render_tray_visual_internal(style, percent, status, light_theme, width, height, true)
}

fn render_tray_visual_internal(
    style: WindowsTrayIconStyle,
    percent: Option<f64>,
    status: TrayVisualStatus,
    light_theme: bool,
    width: u32,
    height: u32,
    native_macos: bool,
) -> Image<'static> {
    let width = width.max(16);
    let height = height.max(16);
    let master_width = width * SUPERSAMPLE;
    let master_height = height * SUPERSAMPLE;
    let normalized_percent = percent.map(normalize_percent);
    let label = icon_label(normalized_percent, status);
    let digit_scale = if label == "100" {
        MACOS_CARD_HUNDRED_DIGIT_SCALE
    } else {
        MACOS_CARD_DIGIT_SCALE
    };
    let digit_rect = |rect: RectF| {
        if native_macos {
            constrain_rect_to_canvas(
                scale_rect_from_center(rect, digit_scale),
                master_width as f32,
                master_height as f32,
                SUPERSAMPLE as f32,
            )
        } else {
            rect
        }
    };
    let mut canvas = Canvas::new(master_width, master_height);

    match style {
        WindowsTrayIconStyle::GradientNumberPlate => {
            let inset = master_width.min(master_height) as f32 * 0.055;
            draw_gradient_rounded_rect(
                &mut canvas,
                RectF::new(
                    inset,
                    inset,
                    master_width as f32 - inset,
                    master_height as f32 - inset,
                ),
                master_width.min(master_height) as f32 * 0.19,
            );
            draw_centered_label(
                &mut canvas,
                &label,
                digit_rect(RectF::new(
                    master_width as f32 * 0.19,
                    master_height as f32 * 0.18,
                    master_width as f32 * 0.81,
                    master_height as f32 * 0.82,
                )),
                LabelPaint::Solid(LIGHT_TEXT),
                None,
            );
        }
        WindowsTrayIconStyle::GradientNumberCard => {
            let windows_notification_cell = master_width == master_height;
            let rect = if windows_notification_cell {
                RectF::new(
                    master_width as f32 * 0.02,
                    master_height as f32 * 0.14,
                    master_width as f32 * 0.98,
                    master_height as f32 * 0.86,
                )
            } else {
                RectF::new(
                    master_width as f32 * 0.02,
                    master_height as f32 * 0.03,
                    master_width as f32 * 0.98,
                    master_height as f32 * 0.97,
                )
            };
            let radius = master_height as f32 * 0.13;
            let border_width = if native_macos && !windows_notification_cell {
                SUPERSAMPLE as f32 * MACOS_CARD_BORDER_WIDTH
            } else {
                SUPERSAMPLE as f32
            };
            let inner_rect = RectF::new(
                rect.left + border_width,
                rect.top + border_width,
                rect.right - border_width,
                rect.bottom - border_width,
            );
            let inner_radius = (radius - border_width).max(0.0);
            let border_color = if light_theme {
                CARD_BORDER_LIGHT
            } else {
                CARD_BORDER_DARK
            };
            draw_solid_rounded_rect(&mut canvas, rect, radius, border_color);
            draw_solid_rounded_rect(&mut canvas, inner_rect, inner_radius, CARD_EMPTY);
            draw_gradient_progress_fill(
                &mut canvas,
                inner_rect,
                inner_radius,
                normalized_percent.unwrap_or(0.0) / 100.0,
            );
            draw_centered_label_scaled(
                &mut canvas,
                &label,
                digit_rect(if windows_notification_cell {
                    RectF::new(
                        master_width as f32 * 0.11,
                        master_height as f32 * 0.21,
                        master_width as f32 * 0.89,
                        master_height as f32 * 0.79,
                    )
                } else {
                    RectF::new(
                        master_width as f32 * 0.20,
                        master_height as f32 * 0.20,
                        master_width as f32 * 0.80,
                        master_height as f32 * 0.80,
                    )
                }),
                if windows_notification_cell {
                    WINDOWS_CARD_GLYPH_HORIZONTAL_SCALE
                } else {
                    WIDE_CARD_GLYPH_HORIZONTAL_SCALE
                },
                LabelPaint::Solid(LIGHT_TEXT),
                None,
            );
        }
        WindowsTrayIconStyle::GradientNumber => {
            draw_centered_label(
                &mut canvas,
                &label,
                digit_rect(RectF::new(
                    0.0,
                    master_height as f32 * 0.02,
                    master_width as f32,
                    master_height as f32 * 0.98,
                )),
                LabelPaint::Gradient,
                Some(if light_theme {
                    [166, 178, 198, 220]
                } else {
                    [255, 255, 255, 225]
                }),
            );
        }
        WindowsTrayIconStyle::NumberProgressBar => {
            draw_centered_label(
                &mut canvas,
                &label,
                digit_rect(RectF::new(
                    master_width as f32 * 0.08,
                    master_height as f32 * 0.02,
                    master_width as f32 * 0.92,
                    master_height as f32 * 0.64,
                )),
                LabelPaint::Solid(if light_theme { DARK_TEXT } else { LIGHT_TEXT }),
                None,
            );
            draw_progress_bar(
                &mut canvas,
                normalized_percent.unwrap_or(0.0) / 100.0,
                RectF::new(
                    master_width as f32 * 0.03,
                    master_height as f32 * 0.76,
                    master_width as f32 * 0.97,
                    master_height as f32 * 0.88,
                ),
                light_theme,
            );
        }
        WindowsTrayIconStyle::LogoProgressRing => {
            draw_progress_ring(
                &mut canvas,
                normalized_percent.unwrap_or(0.0) / 100.0,
                light_theme,
            );
            if status == TrayVisualStatus::Unavailable {
                draw_centered_label(
                    &mut canvas,
                    &label,
                    RectF::new(
                        master_width as f32 * 0.27,
                        master_height as f32 * 0.27,
                        master_width as f32 * 0.73,
                        master_height as f32 * 0.73,
                    ),
                    LabelPaint::Solid(LIGHT_TEXT),
                    Some([0, 0, 0, 120]),
                );
            } else {
                draw_codex_mark(&mut canvas);
            }
        }
    }

    let rgba = downsample_rgba(&canvas.pixels, master_width, master_height, width, height);
    Image::new_owned(rgba, width, height)
}

pub(crate) fn render_tray_visual_previews(
    platform: TrayVisualPlatform,
    base_size: u32,
    light_theme: bool,
) -> Result<Vec<TrayVisualPreview>, String> {
    TRAY_VISUAL_STYLES
        .into_iter()
        .map(|style| {
            let (width, height) = tray_visual_dimensions(style, platform, base_size);
            let image = render_tray_visual(
                style,
                Some(97.0),
                TrayVisualStatus::Fresh,
                light_theme,
                width,
                height,
            );
            let png = encode_png(&image)?;
            Ok(TrayVisualPreview {
                style,
                data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
                pixel_width: width,
                pixel_height: height,
            })
        })
        .collect()
}

fn encode_png(image: &Image<'_>) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, image.width(), image.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("encode tray preview header: {error}"))?;
        writer
            .write_image_data(image.rgba())
            .map_err(|error| format!("encode tray preview pixels: {error}"))?;
    }
    Ok(output)
}

pub(crate) fn fit_icon_to_square(source: &Image<'_>, destination_size: u32) -> Vec<u8> {
    fit_icon_to_canvas(source, destination_size, destination_size)
}

pub(crate) fn codex_tools_icon_rgba(destination_size: u32) -> Vec<u8> {
    fit_icon_to_square(&CODEX_TOOLS_ICON, destination_size.max(1))
}

fn normalize_percent(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 100.0).round()
    } else {
        0.0
    }
}

fn icon_label(percent: Option<f64>, status: TrayVisualStatus) -> String {
    match status {
        TrayVisualStatus::Unavailable => "--".to_string(),
        TrayVisualStatus::Fresh | TrayVisualStatus::Stale | TrayVisualStatus::Error => percent
            .map(|value| format!("{:.0}", normalize_percent(value)))
            .unwrap_or_else(|| "--".to_string()),
    }
}

#[derive(Clone, Copy)]
struct RectF {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl RectF {
    fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    fn width(self) -> f32 {
        self.right - self.left
    }

    fn height(self) -> f32 {
        self.bottom - self.top
    }

    fn center_x(self) -> f32 {
        (self.left + self.right) / 2.0
    }

    fn center_y(self) -> f32 {
        (self.top + self.bottom) / 2.0
    }
}

fn scale_rect_from_center(rect: RectF, scale: f32) -> RectF {
    let half_width = rect.width() * scale / 2.0;
    let half_height = rect.height() * scale / 2.0;
    RectF::new(
        rect.center_x() - half_width,
        rect.center_y() - half_height,
        rect.center_x() + half_width,
        rect.center_y() + half_height,
    )
}

fn constrain_rect_to_canvas(
    rect: RectF,
    canvas_width: f32,
    canvas_height: f32,
    margin: f32,
) -> RectF {
    let margin_x = margin.min(canvas_width / 2.0);
    let margin_y = margin.min(canvas_height / 2.0);
    RectF::new(
        rect.left.max(margin_x),
        rect.top.max(margin_y),
        rect.right.min(canvas_width - margin_x),
        rect.bottom.min(canvas_height - margin_y),
    )
}

struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    fn blend(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if x >= self.width || y >= self.height || color[3] == 0 {
            return;
        }
        let index = ((y * self.width + x) * 4) as usize;
        let destination_alpha = self.pixels[index + 3] as u32;
        let source_alpha = color[3] as u32;
        let inverse = 255 - source_alpha;
        let output_alpha = source_alpha + (destination_alpha * inverse + 127) / 255;
        if output_alpha == 0 {
            return;
        }
        for (channel, source) in color.iter().copied().enumerate() {
            let source_premultiplied = source as u32 * source_alpha;
            let destination_premultiplied =
                self.pixels[index + channel] as u32 * destination_alpha * inverse / 255;
            self.pixels[index + channel] =
                ((source_premultiplied + destination_premultiplied + output_alpha / 2)
                    / output_alpha) as u8;
        }
        self.pixels[index + 3] = output_alpha.min(255) as u8;
    }
}

#[derive(Clone, Copy)]
enum LabelPaint {
    Solid([u8; 4]),
    Gradient,
}

fn draw_centered_label(
    canvas: &mut Canvas,
    label: &str,
    rect: RectF,
    paint: LabelPaint,
    outline_color: Option<[u8; 4]>,
) {
    draw_centered_label_scaled(
        canvas,
        label,
        rect,
        GLYPH_HORIZONTAL_SCALE,
        paint,
        outline_color,
    );
}

fn draw_centered_label_scaled(
    canvas: &mut Canvas,
    label: &str,
    rect: RectF,
    horizontal_scale: f32,
    paint: LabelPaint,
    outline_color: Option<[u8; 4]>,
) {
    let mask = rasterize_text_mask(canvas.width, canvas.height, label, rect, horizontal_scale);
    if let Some(color) = outline_color {
        let radius = (canvas.width.min(canvas.height) / 48).max(2);
        let expanded = dilate_mask(&mask, canvas.width, canvas.height, radius);
        for (index, alpha) in expanded.into_iter().enumerate() {
            let original = mask[index];
            if alpha <= original {
                continue;
            }
            let x = index as u32 % canvas.width;
            let y = index as u32 / canvas.width;
            let mut color = color;
            color[3] = ((alpha - original) as u32 * color[3] as u32 / 255) as u8;
            canvas.blend(x, y, color);
        }
    }
    for (index, alpha) in mask.into_iter().enumerate() {
        if alpha == 0 {
            continue;
        }
        let x = index as u32 % canvas.width;
        let y = index as u32 / canvas.width;
        let mut color = match paint {
            LabelPaint::Solid(color) => color,
            LabelPaint::Gradient => codex_gradient(x, y, canvas.width, canvas.height),
        };
        color[3] = (color[3] as u32 * alpha as u32 / 255) as u8;
        canvas.blend(x, y, color);
    }
}

fn rasterize_text_mask(
    width: u32,
    height: u32,
    label: &str,
    rect: RectF,
    horizontal_scale: f32,
) -> Vec<u8> {
    let mut mask = vec![0; (width * height) as usize];
    if let Some(font) = ui_font() {
        if let Some(px) = fitting_font_size(font, text_sizing_reference(label), rect) {
            let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
            layout.reset(&LayoutSettings::default());
            layout.append(&[font], &TextStyle::new(label, px.max(6.0), 0));
            let bounds = glyph_bounds(layout.glyphs());
            if let Some((min_x, min_y, max_x, max_y)) = bounds {
                let offset_x = rect.left + (rect.width() - (max_x - min_x)) / 2.0 - min_x;
                let offset_y = rect.top + (rect.height() - (max_y - min_y)) / 2.0 - min_y;
                let layout_width = max_x - min_x;
                let fitted_horizontal_scale = if layout_width > 0.0 {
                    horizontal_scale.min(((rect.width() - 2.0) / layout_width).min(1.0))
                } else {
                    horizontal_scale
                };
                for glyph in layout.glyphs() {
                    let (_, bitmap) = font.rasterize_config(glyph.key);
                    let origin_x = (glyph.x + offset_x).round() as i32;
                    let origin_y = (glyph.y + offset_y).round() as i32;
                    for glyph_y in 0..glyph.height {
                        for glyph_x in 0..glyph.width {
                            let x = origin_x + glyph_x as i32;
                            let y = origin_y + glyph_y as i32;
                            if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                                continue;
                            }
                            let source = bitmap[glyph_y * glyph.width + glyph_x];
                            let destination = y as usize * width as usize + x as usize;
                            mask[destination] = mask[destination].max(source);
                        }
                    }
                }
                return scale_mask_horizontally(
                    &mask,
                    width,
                    height,
                    rect.center_x(),
                    fitted_horizontal_scale,
                );
            }
        }
    }
    draw_bitmap_text_mask(&mut mask, width, height, label, rect);
    scale_mask_horizontally(&mask, width, height, rect.center_x(), horizontal_scale)
}

fn text_sizing_reference(label: &str) -> &str {
    if label.len() <= 2 && label.bytes().all(|byte| byte.is_ascii_digit()) {
        "70"
    } else {
        label
    }
}

fn fitting_font_size(font: &Font, label: &str, rect: RectF) -> Option<f32> {
    let max_px = rect.height() * 1.16;
    for step in 0..28 {
        let px = (max_px * (1.0 - step as f32 * 0.025)).max(6.0);
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings::default());
        layout.append(&[font], &TextStyle::new(label, px, 0));
        if let Some((min_x, min_y, max_x, max_y)) = glyph_bounds(layout.glyphs()) {
            if max_x - min_x <= rect.width() && max_y - min_y <= rect.height() {
                return Some(px);
            }
        }
    }
    None
}

fn scale_mask_horizontally(
    source: &[u8],
    width: u32,
    height: u32,
    center_x: f32,
    scale: f32,
) -> Vec<u8> {
    if (scale - 1.0).abs() <= f32::EPSILON {
        return source.to_vec();
    }
    let mut destination = vec![0; source.len()];
    for y in 0..height {
        for x in 0..width {
            let source_x = center_x + (x as f32 + 0.5 - center_x) / scale - 0.5;
            if source_x < 0.0 || source_x > width.saturating_sub(1) as f32 {
                continue;
            }
            let left = source_x.floor() as u32;
            let right = (left + 1).min(width - 1);
            let fraction = source_x - left as f32;
            let left_alpha = source[(y * width + left) as usize] as f32;
            let right_alpha = source[(y * width + right) as usize] as f32;
            destination[(y * width + x) as usize] =
                (left_alpha + (right_alpha - left_alpha) * fraction).round() as u8;
        }
    }
    destination
}

fn glyph_bounds<U: Copy + Clone>(
    glyphs: &[fontdue::layout::GlyphPosition<U>],
) -> Option<(f32, f32, f32, f32)> {
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for glyph in glyphs {
        let glyph_bounds = (
            glyph.x,
            glyph.y,
            glyph.x + glyph.width as f32,
            glyph.y + glyph.height as f32,
        );
        bounds = Some(match bounds {
            Some((min_x, min_y, max_x, max_y)) => (
                min_x.min(glyph_bounds.0),
                min_y.min(glyph_bounds.1),
                max_x.max(glyph_bounds.2),
                max_y.max(glyph_bounds.3),
            ),
            None => glyph_bounds,
        });
    }
    bounds
}

fn ui_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| Font::from_bytes(TRAY_FONT_BYTES, FontSettings::default()).ok())
        .as_ref()
}

fn draw_bitmap_text_mask(mask: &mut [u8], width: u32, height: u32, label: &str, rect: RectF) {
    let chars = label.chars().collect::<Vec<_>>();
    let glyph_width = 5;
    let glyph_height = 7;
    let spacing = 1;
    let logical_width =
        chars.len() as i32 * glyph_width + chars.len().saturating_sub(1) as i32 * spacing;
    let scale = ((rect.width() as i32 / logical_width.max(1))
        .min(rect.height() as i32 / glyph_height)
        .max(1)) as u32;
    let rendered_width = logical_width as u32 * scale;
    let rendered_height = glyph_height as u32 * scale;
    let origin_x = (rect.left + (rect.width() - rendered_width as f32) / 2.0).round() as i32;
    let origin_y = (rect.top + (rect.height() - rendered_height as f32) / 2.0).round() as i32;
    for (char_index, ch) in chars.into_iter().enumerate() {
        let rows = glyph_rows(ch);
        for (row, bits) in rows.into_iter().enumerate() {
            for column in 0..glyph_width {
                if bits & (1 << (glyph_width - 1 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = origin_x
                            + ((char_index as i32 * (glyph_width + spacing) + column) as u32
                                * scale
                                + dx) as i32;
                        let y = origin_y + (row as u32 * scale + dy) as i32;
                        if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                            mask[y as usize * width as usize + x as usize] = 255;
                        }
                    }
                }
            }
        }
    }
}

fn glyph_rows(ch: char) -> [u8; 7] {
    match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b10100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        _ => [0; 7],
    }
}

fn dilate_mask(mask: &[u8], width: u32, height: u32, radius: u32) -> Vec<u8> {
    let mut expanded = vec![0; mask.len()];
    let radius = radius as i32;
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let mut value = 0;
            for offset_y in -radius..=radius {
                for offset_x in -radius..=radius {
                    if offset_x * offset_x + offset_y * offset_y > radius * radius {
                        continue;
                    }
                    let source_x = x + offset_x;
                    let source_y = y + offset_y;
                    if source_x >= 0
                        && source_y >= 0
                        && source_x < width as i32
                        && source_y < height as i32
                    {
                        value =
                            value.max(mask[source_y as usize * width as usize + source_x as usize]);
                    }
                }
            }
            expanded[y as usize * width as usize + x as usize] = value;
        }
    }
    expanded
}

fn draw_gradient_rounded_rect(canvas: &mut Canvas, rect: RectF, radius: f32) {
    for y in rect.top.floor().max(0.0) as u32..rect.bottom.ceil().min(canvas.height as f32) as u32 {
        for x in
            rect.left.floor().max(0.0) as u32..rect.right.ceil().min(canvas.width as f32) as u32
        {
            if inside_rounded_rect(x as f32 + 0.5, y as f32 + 0.5, rect, radius) {
                canvas.blend(x, y, codex_gradient(x, y, canvas.width, canvas.height));
            }
        }
    }
}

fn draw_gradient_progress_fill(canvas: &mut Canvas, rect: RectF, radius: f32, progress: f64) {
    let progress = progress.clamp(0.0, 1.0) as f32;
    if progress <= 0.0 {
        return;
    }

    let fill_right = rect.left + rect.width() * progress;
    for y in rect.top.floor().max(0.0) as u32..rect.bottom.ceil().min(canvas.height as f32) as u32 {
        for x in
            rect.left.floor().max(0.0) as u32..rect.right.ceil().min(canvas.width as f32) as u32
        {
            let point_x = x as f32 + 0.5;
            let point_y = y as f32 + 0.5;
            if point_x <= fill_right && inside_rounded_rect(point_x, point_y, rect, radius) {
                canvas.blend(x, y, codex_gradient(x, y, canvas.width, canvas.height));
            }
        }
    }
}

fn draw_solid_rounded_rect(canvas: &mut Canvas, rect: RectF, radius: f32, color: [u8; 4]) {
    for y in rect.top.floor().max(0.0) as u32..rect.bottom.ceil().min(canvas.height as f32) as u32 {
        for x in
            rect.left.floor().max(0.0) as u32..rect.right.ceil().min(canvas.width as f32) as u32
        {
            if inside_rounded_rect(x as f32 + 0.5, y as f32 + 0.5, rect, radius) {
                canvas.blend(x, y, color);
            }
        }
    }
}

fn inside_rounded_rect(x: f32, y: f32, rect: RectF, radius: f32) -> bool {
    let center_x = x.clamp(rect.left + radius, rect.right - radius);
    let center_y = y.clamp(rect.top + radius, rect.bottom - radius);
    let dx = x - center_x;
    let dy = y - center_y;
    dx * dx + dy * dy <= radius * radius
}

fn draw_progress_bar(canvas: &mut Canvas, progress: f64, rect: RectF, light_theme: bool) {
    let radius = rect.height() / 2.0;
    let track = if light_theme {
        [11, 25, 46, 42]
    } else {
        [255, 255, 255, 72]
    };
    draw_solid_rounded_rect(canvas, rect, radius, track);
    let progress = progress.clamp(0.0, 1.0) as f32;
    if progress <= 0.0 {
        return;
    }
    let fill_right = (rect.left + rect.width() * progress).max(rect.left + rect.height());
    draw_gradient_rounded_rect(
        canvas,
        RectF::new(rect.left, rect.top, fill_right.min(rect.right), rect.bottom),
        radius,
    );
}

fn draw_progress_ring(canvas: &mut Canvas, progress: f64, light_theme: bool) {
    let center_x = canvas.width as f32 / 2.0;
    let center_y = canvas.height as f32 / 2.0;
    let radius = canvas.width.min(canvas.height) as f32 * 0.405;
    let thickness = canvas.width.min(canvas.height) as f32 * 0.080;
    let progress = progress.clamp(0.0, 1.0) as f32;
    let track = if light_theme {
        [223, 230, 239, 255]
    } else {
        [109, 119, 133, 255]
    };
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let dx = x as f32 + 0.5 - center_x;
            let dy = y as f32 + 0.5 - center_y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance < radius - thickness / 2.0 || distance > radius + thickness / 2.0 {
                continue;
            }
            let mut turn = dy.atan2(dx) / std::f32::consts::TAU + 0.25;
            if turn < 0.0 {
                turn += 1.0;
            }
            if turn <= progress {
                canvas.blend(x, y, RING_BLUE);
            } else {
                canvas.blend(x, y, track);
            }
        }
    }
}

fn draw_codex_mark(canvas: &mut Canvas) {
    let size = canvas.width.min(canvas.height);
    let center_x = canvas.width as f32 / 2.0;
    let center_y = canvas.height as f32 / 2.0;
    let lobe_radius = size as f32 * 0.108;
    let lobes = [
        (-0.080, -0.082),
        (0.020, -0.108),
        (0.108, -0.052),
        (0.120, 0.052),
        (0.048, 0.116),
        (-0.060, 0.116),
        (-0.124, 0.044),
        (-0.120, -0.060),
    ];
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let point_x = x as f32 + 0.5;
            let point_y = y as f32 + 0.5;
            let inside_center = (point_x - center_x).powi(2) + (point_y - center_y).powi(2)
                <= (size as f32 * 0.155).powi(2);
            let inside_lobe = lobes.iter().any(|(offset_x, offset_y)| {
                let lobe_x = center_x + size as f32 * offset_x;
                let lobe_y = center_y + size as f32 * offset_y;
                (point_x - lobe_x).powi(2) + (point_y - lobe_y).powi(2) <= lobe_radius.powi(2)
            });
            if inside_center || inside_lobe {
                canvas.blend(x, y, codex_gradient(x, y, canvas.width, canvas.height));
            }
        }
    }

    let stroke = size as f32 * 0.043;
    draw_rounded_line(
        canvas,
        (
            center_x - size as f32 * 0.112,
            center_y - size as f32 * 0.088,
        ),
        (center_x - size as f32 * 0.044, center_y),
        stroke,
        LIGHT_TEXT,
    );
    draw_rounded_line(
        canvas,
        (center_x - size as f32 * 0.044, center_y),
        (
            center_x - size as f32 * 0.112,
            center_y + size as f32 * 0.088,
        ),
        stroke,
        LIGHT_TEXT,
    );
    draw_rounded_line(
        canvas,
        (
            center_x + size as f32 * 0.012,
            center_y + size as f32 * 0.080,
        ),
        (
            center_x + size as f32 * 0.128,
            center_y + size as f32 * 0.080,
        ),
        stroke,
        LIGHT_TEXT,
    );
}

fn draw_rounded_line(
    canvas: &mut Canvas,
    start: (f32, f32),
    end: (f32, f32),
    thickness: f32,
    color: [u8; 4],
) {
    let min_x = start.0.min(end.0) - thickness;
    let max_x = start.0.max(end.0) + thickness;
    let min_y = start.1.min(end.1) - thickness;
    let max_y = start.1.max(end.1) + thickness;
    for y in min_y.floor().max(0.0) as u32..max_y.ceil().min(canvas.height as f32) as u32 {
        for x in min_x.floor().max(0.0) as u32..max_x.ceil().min(canvas.width as f32) as u32 {
            if distance_to_segment((x as f32 + 0.5, y as f32 + 0.5), start, end) <= thickness / 2.0
            {
                canvas.blend(x, y, color);
            }
        }
    }
}

fn distance_to_segment(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return ((point.0 - start.0).powi(2) + (point.1 - start.1).powi(2)).sqrt();
    }
    let t =
        (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length_squared).clamp(0.0, 1.0);
    let projected = (start.0 + t * dx, start.1 + t * dy);
    ((point.0 - projected.0).powi(2) + (point.1 - projected.1).powi(2)).sqrt()
}

fn codex_gradient(x: u32, y: u32, width: u32, height: u32) -> [u8; 4] {
    let horizontal = x as f32 / width.saturating_sub(1).max(1) as f32;
    let vertical = y as f32 / height.saturating_sub(1).max(1) as f32;
    let t = (vertical * 0.72 + horizontal * 0.28).clamp(0.0, 1.0);
    let stops = [
        (0.0, [192, 171, 255]),
        (0.34, [137, 135, 255]),
        (0.68, [88, 114, 255]),
        (1.0, [53, 43, 255]),
    ];
    for pair in stops.windows(2) {
        if t <= pair[1].0 {
            let local = (t - pair[0].0) / (pair[1].0 - pair[0].0);
            return [
                lerp(pair[0].1[0], pair[1].1[0], local),
                lerp(pair[0].1[1], pair[1].1[1], local),
                lerp(pair[0].1[2], pair[1].1[2], local),
                255,
            ];
        }
    }
    [53, 43, 255, 255]
}

fn lerp(start: u8, end: u8, t: f32) -> u8 {
    (start as f32 + (end as f32 - start as f32) * t).round() as u8
}

fn fit_icon_to_canvas(
    source: &Image<'_>,
    destination_width: u32,
    destination_height: u32,
) -> Vec<u8> {
    let source_width = source.width() as usize;
    let source_height = source.height() as usize;
    let destination_width = destination_width.max(1) as usize;
    let destination_height = destination_height.max(1) as usize;
    let mut destination = vec![0_u8; destination_width * destination_height * 4];
    let (crop_left, crop_top, crop_right, crop_bottom) = visible_square(source);
    let crop_width = crop_right - crop_left;
    let crop_height = crop_bottom - crop_top;
    let scale = (destination_width as f32 / crop_width as f32)
        .min(destination_height as f32 / crop_height as f32);
    let rendered_width = (crop_width as f32 * scale).round().max(1.0) as usize;
    let rendered_height = (crop_height as f32 * scale).round().max(1.0) as usize;
    let offset_x = (destination_width - rendered_width.min(destination_width)) / 2;
    let offset_y = (destination_height - rendered_height.min(destination_height)) / 2;

    for destination_y in 0..rendered_height.min(destination_height) {
        let source_y = crop_top
            + ((destination_y as f32 + 0.5) * crop_height as f32 / rendered_height as f32)
                .floor()
                .clamp(0.0, (crop_height - 1) as f32) as usize;
        for destination_x in 0..rendered_width.min(destination_width) {
            let source_x = crop_left
                + ((destination_x as f32 + 0.5) * crop_width as f32 / rendered_width as f32)
                    .floor()
                    .clamp(0.0, (crop_width - 1) as f32) as usize;
            let source_index = (source_y.min(source_height - 1) * source_width
                + source_x.min(source_width - 1))
                * 4;
            let destination_index =
                ((destination_y + offset_y) * destination_width + destination_x + offset_x) * 4;
            destination[destination_index..destination_index + 4]
                .copy_from_slice(&source.rgba()[source_index..source_index + 4]);
        }
    }
    destination
}

fn visible_square(source: &Image<'_>) -> (usize, usize, usize, usize) {
    let width = source.width() as usize;
    let height = source.height() as usize;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;
    for (index, pixel) in source.rgba().chunks_exact(4).enumerate() {
        if pixel[3] <= 16 {
            continue;
        }
        let x = index % width;
        let y = index / width;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        found = true;
    }
    if !found {
        return (0, 0, width, height);
    }
    let content_width = max_x - min_x + 1;
    let content_height = max_y - min_y + 1;
    let content_size = content_width.max(content_height).min(width.min(height));
    let center_x = (min_x + max_x).div_ceil(2);
    let center_y = (min_y + max_y).div_ceil(2);
    let crop_left = center_x
        .saturating_sub(content_size / 2)
        .min(width - content_size);
    let crop_top = center_y
        .saturating_sub(content_size / 2)
        .min(height - content_size);
    (
        crop_left,
        crop_top,
        crop_left + content_size,
        crop_top + content_size,
    )
}

fn downsample_rgba(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
) -> Vec<u8> {
    let mut destination = vec![0; (destination_width * destination_height * 4) as usize];
    for destination_y in 0..destination_height {
        let top = destination_y * source_height / destination_height;
        let bottom = ((destination_y + 1) * source_height / destination_height).max(top + 1);
        for destination_x in 0..destination_width {
            let left = destination_x * source_width / destination_width;
            let right = ((destination_x + 1) * source_width / destination_width).max(left + 1);
            let mut alpha_sum = 0_u64;
            let mut premultiplied = [0_u64; 3];
            let mut count = 0_u64;
            for source_y in top..bottom.min(source_height) {
                for source_x in left..right.min(source_width) {
                    let index = ((source_y * source_width + source_x) * 4) as usize;
                    let alpha = source[index + 3] as u64;
                    alpha_sum += alpha;
                    for channel in 0..3 {
                        premultiplied[channel] += source[index + channel] as u64 * alpha;
                    }
                    count += 1;
                }
            }
            let destination_index =
                ((destination_y * destination_width + destination_x) * 4) as usize;
            if let Some(divisor) = std::num::NonZeroU64::new(alpha_sum) {
                for (channel, value) in premultiplied.iter().copied().enumerate() {
                    destination[destination_index + channel] =
                        ((value + alpha_sum / 2) / divisor) as u8;
                }
            }
            destination[destination_index + 3] =
                ((alpha_sum + count / 2) / count.max(1)).min(255) as u8;
        }
    }
    destination
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_bounds(image: &Image<'_>) -> (u32, u32, u32, u32) {
        bounds_where(image, |pixel| pixel[3] > 16)
    }

    fn bounds_where(image: &Image<'_>, predicate: impl Fn(&[u8]) -> bool) -> (u32, u32, u32, u32) {
        let mut min_x = image.width();
        let mut min_y = image.height();
        let mut max_x = 0;
        let mut max_y = 0;
        for (index, pixel) in image.rgba().chunks_exact(4).enumerate() {
            if !predicate(pixel) {
                continue;
            }
            let x = index as u32 % image.width();
            let y = index as u32 / image.width();
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        (min_x, min_y, max_x, max_y)
    }

    fn bounds_size(bounds: (u32, u32, u32, u32)) -> (u32, u32) {
        (bounds.2 - bounds.0 + 1, bounds.3 - bounds.1 + 1)
    }

    fn opaque_bounds_between_rows(
        image: &Image<'_>,
        first_row: u32,
        last_row: u32,
    ) -> (u32, u32, u32, u32) {
        let mut min_x = image.width();
        let mut min_y = image.height();
        let mut max_x = 0;
        let mut max_y = 0;
        for y in first_row..last_row.min(image.height()) {
            for x in 0..image.width() {
                let index = ((y * image.width() + x) * 4 + 3) as usize;
                if image.rgba()[index] <= 16 {
                    continue;
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
        (min_x, min_y, max_x, max_y)
    }

    #[test]
    fn renders_all_styles_at_windows_notification_sizes() {
        for size in [16, 20, 24, 32] {
            for style in TRAY_VISUAL_STYLES {
                let image = render_tray_visual(
                    style,
                    Some(97.0),
                    TrayVisualStatus::Fresh,
                    true,
                    size,
                    size,
                );
                assert_eq!((image.width(), image.height()), (size, size));
                assert!(image.rgba().chunks_exact(4).any(|pixel| pixel[3] > 0));
            }
        }
    }

    #[test]
    fn card_is_a_rectangle_without_a_battery_terminal() {
        let image = render_tray_visual(
            WindowsTrayIconStyle::GradientNumberCard,
            Some(97.0),
            TrayVisualStatus::Fresh,
            true,
            64,
            64,
        );
        let (min_x, min_y, max_x, max_y) = opaque_bounds(&image);
        assert!((max_x - min_x) * 100 >= (max_y - min_y) * 130);
        assert_eq!(image.rgba()[((32 * 64 + 63) * 4 + 3) as usize], 0);
    }

    #[test]
    fn card_is_horizontal_inside_the_windows_notification_cell() {
        for size in [16, 20, 24, 32] {
            let image = render_tray_visual(
                WindowsTrayIconStyle::GradientNumberCard,
                Some(97.0),
                TrayVisualStatus::Fresh,
                true,
                size,
                size,
            );
            let (width, height) = bounds_size(opaque_bounds(&image));
            let dimensions = format!("size={size}, card={width}x{height}");
            assert!(width * 100 >= size * 94, "{dimensions}");
            assert!(height * 100 >= size * 68, "{dimensions}");
            assert!(height * 100 <= size * 82, "{dimensions}");
            assert!(width * 100 >= height * 120, "{dimensions}");
        }
    }

    #[test]
    fn windows_card_number_is_legible_inside_the_notification_cell() {
        let image = render_tray_visual(
            WindowsTrayIconStyle::GradientNumberCard,
            Some(88.0),
            TrayVisualStatus::Fresh,
            true,
            32,
            32,
        );
        let (width, height) = bounds_size(bounds_where(&image, |pixel| {
            pixel[0] >= 245 && pixel[1] >= 245 && pixel[2] >= 245 && pixel[3] >= 200
        }));
        assert!(
            width * 100 >= image.width() * 34,
            "number width was {width}px"
        );
        assert!(
            height * 100 >= image.height() * 45,
            "number height was {height}px"
        );
    }

    #[test]
    fn card_fill_shrinks_with_remaining_quota_without_moving_the_number() {
        let mut fill_widths = Vec::new();
        let mut number_bounds = Vec::new();
        for percent in [5.0, 30.0, 65.0, 97.0] {
            let image = render_tray_visual(
                WindowsTrayIconStyle::GradientNumberCard,
                Some(percent),
                TrayVisualStatus::Fresh,
                true,
                128,
                128,
            );
            let sample_y = image.height() / 5 + 1;
            let gradient_pixels = (0..image.width())
                .filter(|x| {
                    let index = ((sample_y * image.width() + x) * 4) as usize;
                    let pixel = &image.rgba()[index..index + 4];
                    pixel[2] > 200
                        && pixel[2] as i16 - pixel[0] as i16 > 20
                        && pixel[2] as i16 - pixel[1] as i16 > 20
                        && pixel[3] > 200
                })
                .count();
            fill_widths.push(gradient_pixels);
            number_bounds.push(bounds_where(&image, |pixel| pixel == LIGHT_TEXT));
        }

        assert!(fill_widths.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(fill_widths[0] <= 8, "5% fill was {}px", fill_widths[0]);
        assert!(fill_widths[3] >= 55, "97% fill was {}px", fill_widths[3]);
        for bounds in number_bounds {
            let center_x_twice = bounds.0 + bounds.2;
            let center_y_twice = bounds.1 + bounds.3;
            assert!(
                center_x_twice.abs_diff(127) <= 2,
                "number center x2 was {center_x_twice}"
            );
            assert!(
                center_y_twice.abs_diff(127) <= 2,
                "number center y2 was {center_y_twice}"
            );
        }
    }

    #[test]
    fn card_empty_track_is_fixed_blue_gray_and_border_adapts_to_theme() {
        let light = render_tray_visual(
            WindowsTrayIconStyle::GradientNumberCard,
            Some(30.0),
            TrayVisualStatus::Fresh,
            true,
            128,
            128,
        );
        let dark = render_tray_visual(
            WindowsTrayIconStyle::GradientNumberCard,
            Some(30.0),
            TrayVisualStatus::Fresh,
            false,
            128,
            128,
        );
        let sample_index = ((26 * 128 + 110) * 4) as usize;
        assert_eq!(&light.rgba()[sample_index..sample_index + 4], CARD_EMPTY);
        assert_eq!(&dark.rgba()[sample_index..sample_index + 4], CARD_EMPTY);
        assert_ne!(light.rgba(), dark.rgba());
    }

    #[test]
    fn error_status_keeps_the_cached_number_without_an_exclamation_mark() {
        assert_eq!(icon_label(Some(90.0), TrayVisualStatus::Error), "90");
        assert_eq!(icon_label(None, TrayVisualStatus::Error), "--");
        assert_eq!(icon_label(None, TrayVisualStatus::Unavailable), "--");

        for style in TRAY_VISUAL_STYLES {
            let fresh =
                render_tray_visual(style, Some(90.0), TrayVisualStatus::Fresh, true, 32, 32);
            let error =
                render_tray_visual(style, Some(90.0), TrayVisualStatus::Error, true, 32, 32);
            assert_eq!(error.rgba(), fresh.rgba(), "style={style:?}");
        }
    }

    #[test]
    fn gradient_digits_have_no_opaque_plate() {
        let image = render_tray_visual(
            WindowsTrayIconStyle::GradientNumber,
            Some(97.0),
            TrayVisualStatus::Fresh,
            true,
            64,
            64,
        );
        assert_eq!(image.rgba()[3], 0);
        assert_eq!(image.rgba()[((63 * 64 + 63) * 4 + 3) as usize], 0);
        assert!(image
            .rgba()
            .chunks_exact(4)
            .any(|pixel| { pixel[2] > pixel[0] && pixel[2] > pixel[1] && pixel[3] > 180 }));
    }

    #[test]
    fn number_plate_and_card_match_the_concept_proportions() {
        for (style, width_range, height_range) in [
            (WindowsTrayIconStyle::GradientNumberPlate, 57..=62, 40..=45),
            (WindowsTrayIconStyle::GradientNumberCard, 60..=68, 60..=70),
        ] {
            let image =
                render_tray_visual(style, Some(97.0), TrayVisualStatus::Fresh, true, 128, 128);
            let outer = bounds_size(opaque_bounds(&image));
            let number = bounds_size(bounds_where(&image, |pixel| {
                pixel[0] >= 230 && pixel[1] >= 230 && pixel[2] >= 230 && pixel[3] > 16
            }));
            let width_percent = number.0 * 100 / outer.0;
            let height_percent = number.1 * 100 / outer.1;
            let dimensions = format!(
                "style={style:?}, outer={}x{}, number={}x{}, ratio={width_percent}%x{height_percent}%",
                outer.0, outer.1, number.0, number.1
            );
            assert!(width_range.contains(&width_percent), "{dimensions}");
            assert!(height_range.contains(&height_percent), "{dimensions}");
        }
    }

    #[test]
    fn zero_to_ninety_nine_use_seventys_font_size_without_leaving_the_digit_rect() {
        let font = ui_font().expect("embedded tray font should load");
        let rect = RectF::new(48.0, 48.0, 208.0, 208.0);
        let expected_font_size = fitting_font_size(font, "70", rect).unwrap();

        for value in 0..=99 {
            let label = value.to_string();
            assert_eq!(text_sizing_reference(&label), "70", "label={label}");
            assert_eq!(
                fitting_font_size(font, text_sizing_reference(&label), rect),
                Some(expected_font_size),
                "label={label}"
            );

            let mask = rasterize_text_mask(256, 256, &label, rect, GLYPH_HORIZONTAL_SCALE);
            let mut min_x = 256_u32;
            let mut min_y = 256_u32;
            let mut max_x = 0_u32;
            let mut max_y = 0_u32;
            let mut found = false;
            for (index, alpha) in mask.into_iter().enumerate() {
                if alpha <= 16 {
                    continue;
                }
                found = true;
                let x = index as u32 % 256;
                let y = index as u32 / 256;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
            assert!(found, "label={label}");
            assert!(
                min_x >= rect.left.floor() as u32
                    && min_y >= rect.top.floor() as u32
                    && max_x < rect.right.ceil() as u32
                    && max_y < rect.bottom.ceil() as u32,
                "label={label}, bounds=({min_x},{min_y})-({max_x},{max_y})"
            );
        }

        assert_eq!(text_sizing_reference("100"), "100");
        assert_eq!(text_sizing_reference("--"), "--");
    }

    #[test]
    fn two_digit_number_plate_values_keep_a_consistent_visual_height() {
        let number_size = |percent| {
            let image = render_tray_visual(
                WindowsTrayIconStyle::GradientNumberPlate,
                Some(percent),
                TrayVisualStatus::Fresh,
                true,
                64,
                64,
            );
            bounds_size(bounds_where(&image, |pixel| {
                pixel[0] >= 230 && pixel[1] >= 230 && pixel[2] >= 230 && pixel[3] > 16
            }))
        };

        let seventy = number_size(70.0);
        for percent in [71.0, 97.0] {
            let size = number_size(percent);
            assert!(
                size.1.abs_diff(seventy.1) <= 1,
                "70={seventy:?}, {percent}={size:?}"
            );
        }
    }

    #[test]
    fn standalone_number_is_large_beside_windows_system_icons() {
        let image = render_tray_visual(
            WindowsTrayIconStyle::GradientNumber,
            Some(97.0),
            TrayVisualStatus::Fresh,
            true,
            128,
            128,
        );
        let number = bounds_size(opaque_bounds(&image));
        let dimensions = format!("number={}x{}", number.0, number.1);
        assert!(number.0 * 100 >= image.width() * 89, "{dimensions}");
        assert!(number.1 * 100 >= image.height() * 64, "{dimensions}");
    }

    #[test]
    fn progress_number_is_black_on_light_and_white_on_dark() {
        let light = render_tray_visual(
            WindowsTrayIconStyle::NumberProgressBar,
            Some(97.0),
            TrayVisualStatus::Fresh,
            true,
            64,
            64,
        );
        let dark = render_tray_visual(
            WindowsTrayIconStyle::NumberProgressBar,
            Some(97.0),
            TrayVisualStatus::Fresh,
            false,
            64,
            64,
        );
        assert!(light.rgba().chunks_exact(4).any(|pixel| pixel == DARK_TEXT));
        assert!(dark.rgba().chunks_exact(4).any(|pixel| pixel == LIGHT_TEXT));
    }

    #[test]
    fn progress_number_and_bar_fill_the_notification_cell() {
        let image = render_tray_visual(
            WindowsTrayIconStyle::NumberProgressBar,
            Some(97.0),
            TrayVisualStatus::Fresh,
            true,
            128,
            128,
        );
        let number = opaque_bounds_between_rows(&image, 0, 90);
        let bar = opaque_bounds_between_rows(&image, 90, 128);
        let number_width = number.2 - number.0 + 1;
        let number_height = number.3 - number.1 + 1;
        let bar_width = bar.2 - bar.0 + 1;
        let bar_height = bar.3 - bar.1 + 1;

        let dimensions =
            format!("number={number_width}x{number_height}, bar={bar_width}x{bar_height}");
        assert!((88..=98).contains(&number_width), "{dimensions}");
        assert!((63..=70).contains(&number_height), "{dimensions}");
        assert!((118..=124).contains(&bar_width), "{dimensions}");
        assert!((15..=17).contains(&bar_height), "{dimensions}");
        assert!(bar_width * 10 >= number_width * 11);
        assert!(number_height * 10 >= bar_height * 38);
        assert!(bar.1 >= number.3 + 8);
    }

    #[test]
    fn progress_style_uses_most_of_each_windows_icon_cell() {
        for size in [16, 20, 24, 32] {
            let image = render_tray_visual(
                WindowsTrayIconStyle::NumberProgressBar,
                Some(97.0),
                TrayVisualStatus::Fresh,
                true,
                size,
                size,
            );
            let (min_x, min_y, max_x, max_y) = opaque_bounds(&image);
            let content_width = max_x - min_x + 1;
            let content_height = max_y - min_y + 1;
            assert!(content_width * 100 >= size * 85);
            assert!(content_height * 100 >= size * 78);
        }
    }

    #[test]
    fn progress_ring_uses_solid_blue_with_a_neutral_track() {
        let image = render_tray_visual(
            WindowsTrayIconStyle::LogoProgressRing,
            Some(50.0),
            TrayVisualStatus::Fresh,
            true,
            64,
            64,
        );
        assert!(image.rgba().chunks_exact(4).any(|pixel| pixel == RING_BLUE));
        assert!(image
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel == [223, 230, 239, 255]));
    }

    #[test]
    fn preview_command_uses_the_five_live_styles() {
        let previews = render_tray_visual_previews(TrayVisualPlatform::Windows, 24, true)
            .expect("preview PNG encoding should succeed");
        assert_eq!(previews.len(), TRAY_VISUAL_STYLES.len());
        assert!(previews
            .iter()
            .all(|preview| preview.data_url.starts_with("data:image/png;base64,")));
        assert!(previews
            .iter()
            .all(|preview| preview.pixel_width == 24 && preview.pixel_height == 24));
    }

    #[test]
    fn macos_card_and_bar_use_wider_canvases() {
        assert_eq!(
            tray_visual_dimensions(
                WindowsTrayIconStyle::GradientNumberCard,
                TrayVisualPlatform::Macos,
                64,
            ),
            (85, 64)
        );
        assert_eq!(
            tray_visual_dimensions(
                WindowsTrayIconStyle::NumberProgressBar,
                TrayVisualPlatform::Macos,
                64,
            ),
            (80, 64)
        );
    }

    #[test]
    fn native_macos_approved_border_is_thicker_without_moving_the_number() {
        let preview = render_tray_visual(
            WindowsTrayIconStyle::GradientNumberCard,
            Some(97.0),
            TrayVisualStatus::Fresh,
            true,
            85,
            64,
        );
        let native = render_native_macos_tray_visual(
            WindowsTrayIconStyle::GradientNumberCard,
            Some(97.0),
            TrayVisualStatus::Fresh,
            true,
            85,
            64,
        );

        let preview_number = bounds_where(&preview, |pixel| pixel == LIGHT_TEXT);
        let native_number = bounds_where(&native, |pixel| pixel == LIGHT_TEXT);
        assert_eq!(preview_number, native_number);
        assert_ne!(preview.rgba(), native.rgba());

        let border_pixels = |image: &Image<'_>| {
            image
                .rgba()
                .chunks_exact(4)
                .filter(|pixel| *pixel == CARD_BORDER_LIGHT)
                .count()
        };
        assert!(border_pixels(&native) > border_pixels(&preview));
    }

    #[test]
    fn macos_tray_card_uses_the_approved_parameters() {
        assert_eq!(MACOS_CARD_DIGIT_SCALE, 1.0);
        assert_eq!(MACOS_CARD_HUNDRED_DIGIT_SCALE, 1.3);
        assert_eq!(MACOS_CARD_BORDER_WIDTH, 3.75);
    }

    #[test]
    fn native_macos_uses_the_separate_scale_only_for_one_hundred() {
        let render_number_bounds = |percent, native_macos| {
            let image = render_tray_visual_internal(
                WindowsTrayIconStyle::GradientNumberCard,
                Some(percent),
                TrayVisualStatus::Fresh,
                true,
                85,
                64,
                native_macos,
            );
            bounds_size(bounds_where(&image, |pixel| pixel == LIGHT_TEXT))
        };

        assert_eq!(
            render_number_bounds(97.0, true),
            render_number_bounds(97.0, false)
        );
        let native_hundred = render_number_bounds(100.0, true);
        let preview_hundred = render_number_bounds(100.0, false);
        assert!(native_hundred.0 > preview_hundred.0);
        assert!(native_hundred.1 > preview_hundred.1);
    }

    #[test]
    fn native_macos_hundred_digits_stay_inside_the_icon_canvas() {
        for style in [
            WindowsTrayIconStyle::GradientNumberPlate,
            WindowsTrayIconStyle::GradientNumberCard,
            WindowsTrayIconStyle::GradientNumber,
            WindowsTrayIconStyle::NumberProgressBar,
        ] {
            let (width, height) = tray_visual_dimensions(style, TrayVisualPlatform::Macos, 64);
            let image = render_native_macos_tray_visual(
                style,
                Some(100.0),
                TrayVisualStatus::Fresh,
                true,
                width,
                height,
            );
            let number = bounds_where(&image, |pixel| match style {
                WindowsTrayIconStyle::GradientNumberPlate
                | WindowsTrayIconStyle::GradientNumberCard => {
                    pixel[0] >= 230 && pixel[1] >= 230 && pixel[2] >= 230 && pixel[3] > 16
                }
                WindowsTrayIconStyle::GradientNumber => pixel[3] > 16,
                WindowsTrayIconStyle::NumberProgressBar => {
                    pixel[0] < 80 && pixel[1] < 80 && pixel[2] < 100 && pixel[3] > 16
                }
                WindowsTrayIconStyle::LogoProgressRing => false,
            });
            let dimensions = format!("style={style:?}, bounds={number:?}, canvas={width}x{height}");
            assert!(number.0 <= number.2 && number.1 <= number.3, "{dimensions}");
            assert!(number.0 > 0 && number.1 > 0, "{dimensions}");
            assert!(
                number.2 + 1 < width && number.3 + 1 < height,
                "{dimensions}"
            );
        }
    }

    #[test]
    fn macos_digit_scale_enlarges_the_three_digit_quota_without_changing_its_font() {
        let base_rect = RectF::new(68.0, 51.0, 272.0, 205.0);
        let render = |scale: f32| {
            let mut canvas = Canvas::new(340, 256);
            draw_centered_label_scaled(
                &mut canvas,
                "100",
                scale_rect_from_center(base_rect, scale),
                WIDE_CARD_GLYPH_HORIZONTAL_SCALE,
                LabelPaint::Solid(LIGHT_TEXT),
                None,
            );
            Image::new_owned(canvas.pixels, canvas.width, canvas.height)
        };

        let normal = bounds_size(bounds_where(&render(1.0), |pixel| pixel == LIGHT_TEXT));
        let enlarged = bounds_size(bounds_where(&render(1.5), |pixel| pixel == LIGHT_TEXT));
        assert!(
            enlarged.0 > normal.0,
            "normal={normal:?}, enlarged={enlarged:?}"
        );
        assert!(
            enlarged.1 > normal.1,
            "normal={normal:?}, enlarged={enlarged:?}"
        );
    }
}
