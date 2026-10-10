//! Looking at an image file. The caption is the model's (§5.4.1): why it
//! looked, the interface cannot know.
//!
//! Apart from `Read` because what comes back is not text. `Read` returns
//! numbered lines the model can quote and edit by; this returns pixels, by a
//! different road (`ContentBlock::Image`), and one tool answering in two
//! kinds depending on the file would leave the model guessing which it got.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::picture::{self, PictureError};

pub struct ReadImage;

#[async_trait]
impl Tool for ReadImage {
    fn name(&self) -> &'static str {
        "ReadImage"
    }

    fn description(&self) -> &'static str {
        "Look at an image file in the project: a screenshot, a design mock, a \
         diagram, a chart, a photo. PNG, JPEG, GIF, WebP and BMP. The image is \
         shown to you right after the results of this batch of calls; a large one \
         is scaled down to 1568 pixels on its long edge first. \
         Use it for images the human did not attach — the ones they attached you \
         already see. It is also how you check the look of something you built: \
         take a screenshot with a command, then read it. For text files, \
         including SVG, use Read."
    }

    fn properties(&self) -> Value {
        json!({
            "path": {
                "type": "string",
                "description": "Path to the image, relative to the project root.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Looking changes nothing and asks nothing.
    fn concurrent(&self) -> bool {
        true
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан path"))?;

        // Before the disk is touched: there is nothing this could give a
        // model that cannot see, and a result saying "here is the image"
        // with no image after it would have it describe what it never saw.
        if !ctx.vision {
            return Err(ToolError::failed_with(
                "модель не видит изображений",
                "The model in use does not accept images, so the file cannot be \
                 shown to you. Say so, rather than guessing at what is in it.",
            ));
        }

        let abs = ctx.ws.resolve_existing(raw)?;
        if abs.is_dir() {
            return Err(ToolError::failed(format!("{raw} — каталог, не файл")));
        }
        let name = ctx.ws.display(&abs);

        // Decoding a large image takes a while, and the rest of the batch
        // should not wait behind it.
        let loaded = tokio::task::spawn_blocking(move || picture::load(&abs))
            .await
            .map_err(|_| ToolError::failed("чтение изображения не завершилось"))?;
        let picture = loaded.map_err(|err| match err {
            PictureError::NotAPicture => ToolError::failed_with(
                format!("{name}: {err}"),
                "If this is a text file, read it with Read.",
            ),
            other => ToolError::failed(format!("{name}: {other}")),
        })?;
        ctx.check_cancelled()?;

        let mut text = format!("{name}: {}×{}", picture.width, picture.height);
        if let Some((width, height)) = picture.scaled_from {
            text.push_str(&format!(", уменьшено с {width}×{height}"));
        }
        Ok(ToolOutput::new(text).picture(name, picture))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::tests::png;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn an_image_comes_back_under_its_name_with_its_size() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("shot.png"), png(64, 48)).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadImage
            .run(&ctx, json!({ "path": "shot.png" }))
            .await
            .unwrap();

        assert_eq!(out.text, "shot.png: 64×48");
        assert_eq!(out.pictures.len(), 1);
        assert_eq!(out.pictures[0].0, "shot.png");
        assert_eq!(out.pictures[0].1.media_type, "image/png");
    }

    #[tokio::test]
    async fn a_scaled_image_says_what_it_was() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("wide.png"), png(3_136, 100)).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadImage
            .run(&ctx, json!({ "path": "wide.png" }))
            .await
            .unwrap();

        // Coordinates the model reads off the image are of the scaled one,
        // and it has to know that to say anything about the original.
        assert_eq!(out.text, "wide.png: 1568×50, уменьшено с 3136×100");
    }

    #[tokio::test]
    async fn a_text_file_is_sent_back_to_read() {
        let dir = TempDir::new();
        dir.write("notes.md", "# notes");
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadImage
            .run(&ctx, json!({ "path": "notes.md" }))
            .await
            .unwrap_err();
        assert!(err.for_model().contains("Read"), "{}", err.for_model());
    }

    #[tokio::test]
    async fn the_project_boundary_holds_for_images_too() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadImage
            .run(&ctx, json!({ "path": "../outside.png" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_model_that_cannot_see_is_told_so_instead_of_shown_nothing() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("shot.png"), png(8, 8)).unwrap();
        let (mut ctx, _ws) = ctx_for(&dir);
        ctx.vision = false;

        let err = ReadImage
            .run(&ctx, json!({ "path": "shot.png" }))
            .await
            .unwrap_err();
        assert!(err.for_model().contains("does not accept images"));
    }
}
