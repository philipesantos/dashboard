//! The only way an image reaches the window (FR-028, FR-029; research R12). GPUI's `img` turns a
//! string into a fetch, a file read or an asset load, and its `svg` can read a file by path; both
//! are disallowed everywhere else in the workspace (`clippy.toml`). Here they take only bytes the
//! host holds: the status icons of `DESIGN.md`, built from their path data in the binary, and,
//! for later features, images already given as bytes. No `ImageSource` is built from a string or
//! a path, and no plugin byte reaches this module in this feature.
#![allow(clippy::disallowed_methods)]

use design_tokens::Icon;
use gpui::{Hsla, Image, ImageFormat, Img, Pixels, Rgba, Styled, img};
use std::sync::Arc;

/// An image from bytes the caller already holds.
pub fn image_from_bytes(image: Arc<Image>) -> Img {
    img(image)
}

fn hex(color: Hsla) -> String {
    let rgba = Rgba::from(color);
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    )
}

/// The SVG document of a status icon in `color`, from `DESIGN.md`'s path data.
pub fn status_icon_svg(icon: Icon, color: Hsla) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16"><path fill-rule="evenodd" fill="{}" d="{}"/></svg>"#,
        hex(color),
        icon.path()
    )
}

/// A status icon of `DESIGN.md`, drawn in `color` at `size`.
pub fn status_icon(icon: Icon, color: Hsla, size: Pixels) -> Img {
    image_from_bytes(Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        status_icon_svg(icon, color).into_bytes(),
    )))
    .size(size)
}
