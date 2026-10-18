//! Images, from a file on disk to something a model can be sent.
//!
//! Named `picture` because `image` is the crate that does the decoding.
//!
//! Two things happen here that a provider would otherwise do for us, worse.
//! A screenshot off a 4K display is 3840 pixels wide and several megabytes;
//! providers refuse an image past five megabytes and scale a large one down on
//! their side after billing the upload. So a large image is scaled here, once,
//! to the size past which a model sees no more detail. And what is not an
//! image at all is told apart by its bytes, not by its extension: a `.png`
//! that is really HTML would be a 400 that takes the whole conversation with
//! it, because the block stays in the history of every request after.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use image::{DynamicImage, ImageFormat, ImageReader};

/// The longest edge a model is given. Anthropic scales anything larger down to
/// about this before the model sees it; the rest would be paid for and thrown
/// away.
const MAX_EDGE: u32 = 1_568;

/// How large an encoded image may be and still go as it is. Providers draw the
/// line at five megabytes; this leaves room for whichever side of base64 they
/// measure on.
const MAX_SENT_BYTES: usize = 3_500_000;

/// What is read from disk at all. A file past this is not a screenshot.
const MAX_FILE_BYTES: u64 = 40 * 1024 * 1024;

/// Past this many pixels the decoded image is gigabytes of memory, and a file
/// that claims them is more likely a bomb than a picture.
const MAX_PIXELS: u64 = 100_000_000;

/// The edge of a preview: a chip in the composer, a card in the feed.
const PREVIEW_EDGE: u32 = 320;

/// Where pasted images are kept, under the project's `.yaca`.
const PASTED_DIR: &str = "pasted";

/// An image ready to be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub media_type: &'static str,
    /// Base64. Shared, because the history is copied into every request and
    /// an image in it would otherwise be copied a megabyte at a time.
    pub data: Arc<str>,
    pub width: u32,
    pub height: u32,
    /// The size on disk, when it was scaled down from it.
    pub scaled_from: Option<(u32, u32)>,
}

#[derive(Debug, thiserror::Error)]
pub enum PictureError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("file is larger than {} MB", MAX_FILE_BYTES / 1024 / 1024)]
    TooLarge,
    #[error("not an image in PNG, JPEG, GIF, WebP or BMP format")]
    NotAPicture,
    #[error("image is too large: {0}x{1}")]
    TooManyPixels(u32, u32),
    #[error("failed to decode image: {0}")]
    Broken(String),
}

/// Whether the file is an image this module can read, by its first bytes.
pub fn is_picture(path: &Path) -> bool {
    use std::io::Read;

    let mut head = [0u8; 32];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(n) = file.read(&mut head) else {
        return false;
    };
    format_of(&head[..n]).is_some()
}

/// Reads an image and makes it fit to send. Blocking: decoding a large image
/// takes a while, and callers on the runtime move it off.
pub fn load(path: &Path) -> Result<Picture, PictureError> {
    if std::fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Err(PictureError::TooLarge);
    }
    prepare(std::fs::read(path)?)
}

/// The same from bytes already in hand.
pub fn prepare(bytes: Vec<u8>) -> Result<Picture, PictureError> {
    let format = format_of(&bytes).ok_or(PictureError::NotAPicture)?;
    let (width, height) = reader(&bytes, format)
        .into_dimensions()
        .map_err(|e| PictureError::Broken(e.to_string()))?;
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(PictureError::TooManyPixels(width, height));
    }

    // Decoded even when it will go as it is: a header that reads fine says
    // nothing about the rest of the file, and an image the provider cannot
    // decode is refused on this request and on every one after it.
    let decoded = reader(&bytes, format)
        .decode()
        .map_err(|e| PictureError::Broken(e.to_string()))?;

    // As it is, when it already fits: no generation lost to re-encoding, and
    // an animated GIF stays animated.
    let fits = width.max(height) <= MAX_EDGE && bytes.len() <= MAX_SENT_BYTES;
    if fits && sendable(format) {
        return Ok(Picture {
            media_type: media_type(format),
            data: BASE64.encode(&bytes).into(),
            width,
            height,
            scaled_from: None,
        });
    }

    let scaled = fit(decoded, MAX_EDGE);
    // A photograph stays a JPEG; everything else is assumed to be a
    // screenshot, where JPEG smears exactly the text the model has to read.
    let (encoded, media_type) = if format == ImageFormat::Jpeg {
        (jpeg(&scaled)?, "image/jpeg")
    } else {
        let png = encode(&scaled, ImageFormat::Png)?;
        if png.len() <= MAX_SENT_BYTES {
            (png, "image/png")
        } else {
            (jpeg(&scaled)?, "image/jpeg")
        }
    };

    let scaled_from = (scaled.width(), scaled.height()) != (width, height);
    Ok(Picture {
        media_type,
        data: BASE64.encode(&encoded).into(),
        width: scaled.width(),
        height: scaled.height(),
        scaled_from: scaled_from.then_some((width, height)),
    })
}

/// A small copy of an image as a `data:` URL, for the interface to draw.
///
/// The interface gets pixels rather than a path because the webview cannot
/// read the disk, and opening the disk to it for the sake of thumbnails would
/// open it for everything else on the page too.
pub fn preview(path: &Path) -> Result<String, PictureError> {
    if std::fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Err(PictureError::TooLarge);
    }
    let bytes = std::fs::read(path)?;
    let format = format_of(&bytes).ok_or(PictureError::NotAPicture)?;
    let (width, height) = reader(&bytes, format)
        .into_dimensions()
        .map_err(|e| PictureError::Broken(e.to_string()))?;
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(PictureError::TooManyPixels(width, height));
    }
    let decoded = reader(&bytes, format)
        .decode()
        .map_err(|e| PictureError::Broken(e.to_string()))?;
    let small = encode(&fit(decoded, PREVIEW_EDGE), ImageFormat::Png)?;
    Ok(format!("data:image/png;base64,{}", BASE64.encode(small)))
}

/// What an attachment is, for the tile the interface draws for it (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum AttachmentKind {
    Image,
    File,
    Folder,
    /// Nothing is there any more. The message it was attached to still says
    /// it was, so the tile stays and says this instead.
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentLook {
    pub kind: AttachmentKind,
    /// A `data:` URL, for an image.
    pub preview: Option<String>,
}

/// Looks at an attachment. Never fails: whatever cannot be drawn as an image
/// is a file, and a file is drawn by its name.
pub fn look(path: &Path) -> AttachmentLook {
    let kind = |kind| AttachmentLook {
        kind,
        preview: None,
    };
    let Ok(meta) = std::fs::metadata(path) else {
        return kind(AttachmentKind::Missing);
    };
    if meta.is_dir() {
        return kind(AttachmentKind::Folder);
    }
    // The first bytes before the whole file: a dropped video is a file too,
    // and reading a gigabyte to learn that is not worth a tile.
    if !is_picture(path) {
        return kind(AttachmentKind::File);
    }
    match preview(path) {
        Ok(url) => AttachmentLook {
            kind: AttachmentKind::Image,
            preview: Some(url),
        },
        Err(_) => kind(AttachmentKind::File),
    }
}

/// The image whole, as a `data:` URL, for the interface's viewer.
///
/// As it is on disk wherever a browser can draw that; a BMP goes as PNG. Not
/// scaled: the viewer exists to look closer than the preview allows, and the
/// model's 1568 pixels are the model's limit, not the human's.
pub fn original(path: &Path) -> Result<String, PictureError> {
    if std::fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Err(PictureError::TooLarge);
    }
    let bytes = std::fs::read(path)?;
    let format = format_of(&bytes).ok_or(PictureError::NotAPicture)?;
    if sendable(format) {
        return Ok(format!(
            "data:{};base64,{}",
            media_type(format),
            BASE64.encode(&bytes)
        ));
    }
    let decoded = reader(&bytes, format)
        .decode()
        .map_err(|e| PictureError::Broken(e.to_string()))?;
    let png = encode(&decoded, ImageFormat::Png)?;
    Ok(format!("data:image/png;base64,{}", BASE64.encode(png)))
}

/// Keeps an image that arrived without a file — pasted or dropped into the
/// composer, or returned by an MCP server — under the project's `.yaca`, and
/// returns where.
///
/// A file, because everything after this point speaks in paths: the
/// attachment chip, the journal, the card of a reopened session. Named by its
/// content, so pasting the same screenshot twice keeps one copy.
pub fn stash(project_root: &Path, bytes: &[u8]) -> Result<PathBuf, PictureError> {
    let format = format_of(bytes).ok_or(PictureError::NotAPicture)?;
    let dir = crate::project::dir(project_root).join(PASTED_DIR);
    std::fs::create_dir_all(&dir)?;

    let name = blake3::hash(bytes).to_hex();
    let ext = format.extensions_str().first().copied().unwrap_or("png");
    let path = dir.join(format!("{}.{ext}", &name[..16]));
    if !path.exists() {
        std::fs::write(&path, bytes)?;
    }
    Ok(path)
}

/// [`stash`] for bytes that crossed a boundary as base64, which is how the
/// webview hands over what was pasted.
pub fn stash_encoded(project_root: &Path, base64: &str) -> Result<PathBuf, PictureError> {
    let bytes = BASE64
        .decode(base64.trim())
        .map_err(|e| PictureError::Broken(e.to_string()))?;
    stash(project_root, &bytes)
}

/// About how many tokens an image of this size takes in the window.
///
/// Anthropic's own rule of thumb, used for every provider: the point is to
/// know when the window is filling (§5.11), not to bill.
pub fn tokens(width: u32, height: u32) -> u32 {
    (u64::from(width) * u64::from(height) / 750).min(u64::from(u32::MAX)) as u32
}

fn format_of(bytes: &[u8]) -> Option<ImageFormat> {
    let format = image::guess_format(bytes).ok()?;
    matches!(
        format,
        ImageFormat::Png
            | ImageFormat::Jpeg
            | ImageFormat::Gif
            | ImageFormat::WebP
            | ImageFormat::Bmp
    )
    .then_some(format)
}

/// The formats both wire shapes accept. BMP is read, and sent as PNG.
fn sendable(format: ImageFormat) -> bool {
    format != ImageFormat::Bmp
}

fn media_type(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        _ => "image/png",
    }
}

fn reader(bytes: &[u8], format: ImageFormat) -> ImageReader<Cursor<&[u8]>> {
    ImageReader::with_format(Cursor::new(bytes), format)
}

/// Scales down to fit a square of `edge`, keeping the proportions. Never up.
fn fit(image: DynamicImage, edge: u32) -> DynamicImage {
    if image.width().max(image.height()) <= edge {
        return image;
    }
    image.resize(edge, edge, image::imageops::FilterType::Triangle)
}

fn encode(image: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, PictureError> {
    let mut out = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut out), format)
        .map_err(|e| PictureError::Broken(e.to_string()))?;
    Ok(out)
}

/// JPEG has no transparency, so what was transparent goes white — the colour
/// it was drawn against in nearly every screenshot.
fn jpeg(image: &DynamicImage) -> Result<Vec<u8>, PictureError> {
    let rgba = image.to_rgba8();
    let mut flat = image::RgbImage::new(rgba.width(), rgba.height());
    for (to, from) in flat.pixels_mut().zip(rgba.pixels()) {
        let alpha = u32::from(from[3]);
        for channel in 0..3 {
            let mixed = (u32::from(from[channel]) * alpha + 255 * (255 - alpha)) / 255;
            to[channel] = mixed as u8;
        }
    }
    encode(&DynamicImage::ImageRgb8(flat), ImageFormat::Jpeg)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// A real PNG of the given size: a flat colour with a diagonal, so that
    /// scaling has something to keep.
    pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
        let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([30, 90, 160, 255]));
        for i in 0..width.min(height) {
            image.put_pixel(i, i, image::Rgba([255, 255, 255, 255]));
        }
        encode(&DynamicImage::ImageRgba8(image), ImageFormat::Png).unwrap()
    }

    fn decoded(picture: &Picture) -> DynamicImage {
        let bytes = BASE64.decode(picture.data.as_bytes()).unwrap();
        image::load_from_memory(&bytes).unwrap()
    }

    #[test]
    fn a_small_image_goes_as_it_is() {
        let bytes = png(64, 48);
        let picture = prepare(bytes.clone()).unwrap();

        assert_eq!(picture.media_type, "image/png");
        assert_eq!((picture.width, picture.height), (64, 48));
        assert_eq!(picture.scaled_from, None);
        // Byte for byte: nothing is lost to a re-encode nobody needed.
        assert_eq!(BASE64.decode(picture.data.as_bytes()).unwrap(), bytes);
    }

    #[test]
    fn a_large_image_is_scaled_to_fit_and_says_so() {
        let picture = prepare(png(3_840, 2_160)).unwrap();

        assert_eq!(picture.width, MAX_EDGE);
        assert_eq!(picture.height, 882);
        assert_eq!(picture.scaled_from, Some((3_840, 2_160)));
        // What was sent is what was claimed.
        let sent = decoded(&picture);
        assert_eq!((sent.width(), sent.height()), (picture.width, picture.height));
    }

    #[test]
    fn a_tall_image_is_fitted_by_its_long_edge() {
        let picture = prepare(png(400, 4_000)).unwrap();
        assert_eq!((picture.width, picture.height), (157, MAX_EDGE));
    }

    #[test]
    fn a_photograph_stays_a_jpeg() {
        let photo = jpeg(&image::load_from_memory(&png(2_400, 1_600)).unwrap()).unwrap();
        let picture = prepare(photo).unwrap();

        assert_eq!(picture.media_type, "image/jpeg");
        assert_eq!(picture.width, MAX_EDGE);
    }

    #[test]
    fn a_bitmap_is_read_and_sent_as_png() {
        let source = image::load_from_memory(&png(40, 30)).unwrap();
        let picture = prepare(encode(&source, ImageFormat::Bmp).unwrap()).unwrap();

        // Neither wire shape takes BMP.
        assert_eq!(picture.media_type, "image/png");
        assert_eq!((picture.width, picture.height), (40, 30));
    }

    #[test]
    fn what_is_not_an_image_is_told_by_its_bytes() {
        let dir = TempDir::new();
        dir.write("fake.png", "<html>not a picture</html>");
        std::fs::write(dir.path().join("real.dat"), png(8, 8)).unwrap();

        // The extension is a claim; the bytes are the fact.
        assert!(!is_picture(&dir.path().join("fake.png")));
        assert!(is_picture(&dir.path().join("real.dat")));
        assert!(matches!(
            load(&dir.path().join("fake.png")),
            Err(PictureError::NotAPicture)
        ));
    }

    #[test]
    fn a_cut_file_is_an_error_and_not_a_panic() {
        let mut bytes = png(200, 200);
        bytes.truncate(60);
        assert!(matches!(prepare(bytes), Err(PictureError::Broken(_))));
    }

    #[test]
    fn a_preview_is_a_small_data_url() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("shot.png"), png(1_600, 800)).unwrap();

        let url = preview(&dir.path().join("shot.png")).unwrap();
        let data = url.strip_prefix("data:image/png;base64,").unwrap();
        let small = image::load_from_memory(&BASE64.decode(data).unwrap()).unwrap();
        assert_eq!((small.width(), small.height()), (PREVIEW_EDGE, 160));
    }

    #[test]
    fn an_attachment_is_told_apart_by_what_is_on_disk() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("shot.png"), png(40, 20)).unwrap();
        dir.write("fake.png", "<html>");
        dir.write("notes", "no extension");
        dir.mkdir("src");
        let at = |name: &str| look(&dir.path().join(name));

        assert_eq!(at("shot.png").kind, AttachmentKind::Image);
        assert!(at("shot.png").preview.unwrap().starts_with("data:image/png;base64,"));
        // The name is a claim; a tile that promised a picture and showed
        // nothing would be the interface lying about what was attached.
        assert_eq!(at("fake.png"), AttachmentLook { kind: AttachmentKind::File, preview: None });
        assert_eq!(at("notes").kind, AttachmentKind::File);
        assert_eq!(at("src").kind, AttachmentKind::Folder);
        assert_eq!(at("gone.png").kind, AttachmentKind::Missing);
    }

    #[test]
    fn the_viewer_gets_the_image_at_its_own_size() {
        let dir = TempDir::new();
        let bytes = png(3_000, 200);
        std::fs::write(dir.path().join("wide.png"), &bytes).unwrap();
        dir.write("notes.md", "# notes");

        let url = original(&dir.path().join("wide.png")).unwrap();
        let data = url.strip_prefix("data:image/png;base64,").unwrap();
        // Byte for byte: what the model got was scaled, what the human
        // zooms into is not.
        assert_eq!(BASE64.decode(data).unwrap(), bytes);
        assert!(original(&dir.path().join("notes.md")).is_err());
    }

    #[test]
    fn a_pasted_image_is_kept_once_under_the_project() {
        let dir = TempDir::new();
        let bytes = png(16, 16);

        let first = stash(dir.path(), &bytes).unwrap();
        let again = stash(dir.path(), &bytes).unwrap();
        assert_eq!(first, again);
        assert!(first.starts_with(crate::project::dir(dir.path())));
        assert_eq!(std::fs::read(&first).unwrap(), bytes);
        // Text pasted by mistake is not something to keep as an image.
        assert!(stash(dir.path(), b"hello").is_err());
        // The webview's road leads to the same file.
        assert_eq!(stash_encoded(dir.path(), &BASE64.encode(&bytes)).unwrap(), first);
        assert!(stash_encoded(dir.path(), "not base64 at all!").is_err());
    }

    #[test]
    fn a_screenshot_weighs_about_what_the_provider_says() {
        // 1568×882 is what a 4K screenshot becomes: a little under 2k tokens.
        assert_eq!(tokens(1_568, 882), 1_843);
    }
}
