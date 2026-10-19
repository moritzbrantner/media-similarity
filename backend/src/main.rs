#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--video-ocr-json")) {
        let command = args
            .next()
            .ok_or("missing OCR command")?
            .into_string()
            .map_err(|_| "OCR command is not UTF-8")?;
        let language = args
            .next()
            .ok_or("missing OCR language argument")?
            .into_string()
            .map_err(|_| "OCR language is not UTF-8")?;
        let image = args.next().ok_or("missing OCR image path")?;
        if args.next().is_some() {
            return Err("unexpected video OCR argument".into());
        }
        let json = image_similarity_service::workers::media::ocr::video_ocr_json(
            std::path::Path::new(&image),
            command,
            (!language.is_empty()).then_some(language),
        )?;
        println!("{json}");
        return Ok(());
    }
    image_similarity_service::app::run().await
}
