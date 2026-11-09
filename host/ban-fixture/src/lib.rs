//! One function per GPUI method `clippy.toml` bans: native prompts, URL and system openers,
//! credentials, and the image, SVG and asset loaders a plugin string could reach. Clippy must
//! refuse every one (HS-12, FR-019, FR-029).
#![allow(dead_code, unused_must_use, clippy::let_underscore_future)]

use gpui::{
    AnyImageCache, App, AsyncWindowContext, ImageCache, ImageCacheItem, Img, ImgResourceLoader,
    PathPromptOptions, Platform, PlatformWindow, PromptLevel, Resource, RetainAllImageCache, Svg,
    Window, img, svg,
};
use std::path::Path;

fn window_prompt(window: &mut Window, cx: &mut App) {
    let _ = window.prompt(PromptLevel::Info, "Allow?", None, &["Allow"], cx);
}
fn async_prompt(cx: &mut AsyncWindowContext) {
    let _ = cx.prompt(PromptLevel::Info, "Allow?", None, &["Allow"]);
}
fn platform_window_prompt(window: &dyn PlatformWindow) {
    let _ = window.prompt(PromptLevel::Info, "Allow?", None, &[]);
}
fn prompt_for_paths(cx: &App) {
    let _ = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: None,
    });
}
fn prompt_for_new_path(cx: &App) {
    let _ = cx.prompt_for_new_path(Path::new("/tmp"), None);
}
fn app_open_url(cx: &App) {
    cx.open_url("https://attacker.example");
}
fn platform_open_url(platform: &dyn Platform) {
    platform.open_url("https://attacker.example");
}
fn open_with_system(cx: &App) {
    cx.open_with_system(Path::new("/etc/passwd"));
}
fn reveal_path(cx: &App) {
    cx.reveal_path(Path::new("/etc/passwd"));
}
fn write_credentials(cx: &App) {
    let _ = cx.write_credentials("https://attacker.example", "user", b"secret");
}
fn read_credentials(cx: &App) {
    let _ = cx.read_credentials("https://attacker.example");
}
fn delete_credentials(cx: &App) {
    let _ = cx.delete_credentials("https://attacker.example");
}
fn image_from_a_string() -> Img {
    img("https://attacker.example/pixel.png")
}
fn svg_from_a_path() -> Svg {
    svg().external_path("/etc/passwd")
}
fn use_asset(window: &mut Window, cx: &mut App) {
    let _ = window.use_asset::<ImgResourceLoader>(&Resource::Embedded("x".into()), cx);
}
fn fetch_asset(cx: &mut App) {
    let _ = cx.fetch_asset::<ImgResourceLoader>(&Resource::Embedded("x".into()));
}
fn get_asset(window: &mut Window, cx: &mut App) {
    let _ = window
        .get_asset::<ImgResourceLoader>(&Resource::Uri("https://attacker.example".into()), cx);
}
fn image_cache_item(cx: &mut App) -> ImageCacheItem {
    ImageCacheItem::new(&Resource::Path(Path::new("/etc/passwd").into()), cx)
}
fn retain_all_load(cache: &mut RetainAllImageCache, window: &mut Window, cx: &mut App) {
    let _ = cache.load(&Resource::Path(Path::new("/etc/passwd").into()), window, cx);
}
fn any_image_cache_load(cache: &AnyImageCache, window: &mut Window, cx: &mut App) {
    let _ = cache.load(&Resource::Path(Path::new("/etc/passwd").into()), window, cx);
}
fn image_cache_load(cache: &mut dyn ImageCache, window: &mut Window, cx: &mut App) {
    let _ = cache.load(&Resource::Path(Path::new("/etc/passwd").into()), window, cx);
}
fn platform_prompt_for_paths(platform: &dyn Platform) {
    let _ = platform.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: None,
    });
}
fn platform_prompt_for_new_path(platform: &dyn Platform) {
    let _ = platform.prompt_for_new_path(Path::new("/tmp"), None);
}
fn platform_open_with_system(platform: &dyn Platform) {
    platform.open_with_system(Path::new("/etc/passwd"));
}
fn platform_reveal_path(platform: &dyn Platform) {
    platform.reveal_path(Path::new("/etc/passwd"));
}
fn platform_write_credentials(platform: &dyn Platform) {
    let _ = platform.write_credentials("https://attacker.example", "user", b"secret");
}
fn platform_read_credentials(platform: &dyn Platform) {
    let _ = platform.read_credentials("https://attacker.example");
}
fn platform_delete_credentials(platform: &dyn Platform) {
    let _ = platform.delete_credentials("https://attacker.example");
}
