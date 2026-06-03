impl ImageIndexer {
    fn build_payload(
        &self,
        source_image: &SourceImage,
        media: &DecodedMedia,
        settings: &Settings,
        options: PayloadBuildOptions<'_>,
    ) -> Result<ImagePayload, String> {
        let video_scene = options.video_scene;
        let audio_segment = options.audio_segment;
        let pdf_context = options.pdf_context;
        let id_base = if let Some(pdf) = pdf_context {
            pdf.id_base.clone()
        } else if let Some(scene) = video_scene {
            format!("{}#scene={}", source_image.id_base, scene.scene_index + 1)
        } else if let Some(segment) = audio_segment {
            format!(
                "{}#audio-bit={}",
                source_image.id_base,
                segment.scene_index + 1
            )
        } else {
            source_image.id_base.clone()
        };
        let image_id = image_id_for_uri(&id_base);
        let full_audio_url = if let Some(segment) = audio_segment {
            segment.full_audio_url.clone()
        } else if media.kind == MediaKind::Audio {
            source_image
                .local_path()
                .and_then(|path| expose_source_audio(path, &image_id, settings).ok())
                .flatten()
        } else {
            None
        };
        let thumbnail_url = ensure_thumbnail(
            &media.poster,
            &settings.thumbnail_dir,
            &image_id,
            (320, 320),
        )?;
        let animated_thumbnail_url = if options.animated_thumbnail_enabled
            && media.kind == MediaKind::AnimatedGif
        {
            Some(ensure_animated_thumbnail(
                &media.preview_frames,
                &settings.thumbnail_dir,
                &image_id,
                (320, 320),
            )?)
        } else {
            None
        };
        let (width, height) = dimensions(&media.poster);
        let ocr_analysis = options.ocr_override.unwrap_or_else(|| {
            extract_media_ocr(media, settings).unwrap_or_else(|error| {
                tracing::warn!(%error, "OCR extraction failed");
                Default::default()
            })
        });
        let relative_path = if let Some(pdf) = pdf_context {
            pdf.relative_path.clone()
        } else if let Some(scene) = video_scene {
            format!(
                "{}#scene-{:03}",
                source_image.relative_path,
                scene.scene_index + 1
            )
        } else if let Some(segment) = audio_segment {
            format!(
                "{}#audio-bit-{:03}",
                source_image.relative_path,
                segment.scene_index + 1
            )
        } else {
            source_image.relative_path.clone()
        };
        let filename = if let Some(pdf) = pdf_context {
            pdf.filename.clone()
        } else if let Some(scene) = video_scene {
            format!(
                "{} scene {:03}",
                source_image.filename,
                scene.scene_index + 1
            )
        } else if let Some(segment) = audio_segment {
            format!(
                "{} bit {:03}",
                source_image.filename,
                segment.scene_index + 1
            )
        } else {
            source_image.filename.clone()
        };
        let path = if let Some(pdf) = pdf_context {
            pdf.path.clone()
        } else if let Some(scene) = video_scene {
            format!(
                "{}#t={:.3},{:.3}",
                source_image.display_path,
                scene.start.timestamp.seconds(),
                scene.end.timestamp.seconds()
            )
        } else if let Some(segment) = audio_segment {
            format!(
                "{}#t={:.3},{:.3}",
                source_image.display_path, segment.start_seconds, segment.end_seconds
            )
        } else {
            source_image.display_path.clone()
        };
        let full_video_url = video_scene.and_then(|scene| scene.full_video_url.clone());
        let full_pdf_url = pdf_context.and_then(|pdf| pdf.full_pdf_url.clone());
        let pdf_page_url = pdf_context.and_then(|pdf| pdf.pdf_page_url.clone());
        let scene_clip_url = video_scene.and_then(|scene| scene.clip_url.clone());
        let artifacts = generated_artifacts(
            Some(&thumbnail_url),
            animated_thumbnail_url.as_deref(),
            full_video_url.as_deref(),
            full_audio_url.as_deref(),
            full_pdf_url.as_deref(),
            pdf_page_url.as_deref(),
            scene_clip_url.as_deref(),
        );
        Ok(ImagePayload {
            id: image_id,
            path,
            relative_path,
            filename,
            width,
            height,
            size_bytes: source_image.size_bytes,
            modified_at: source_image.modified_at,
            phash: phash_image(&media.poster),
            thumbnail_url: Some(thumbnail_url),
            animated_thumbnail_url,
            media_kind: media.kind.as_str().to_string(),
            frame_count: media.frame_count,
            duration_ms: media.duration_ms,
            full_video_url,
            full_audio_url,
            full_pdf_url,
            pdf_page_url,
            pdf_document_id: pdf_context.and_then(|pdf| pdf.pdf_document_id.clone()),
            pdf_page_index: pdf_context.and_then(|pdf| pdf.pdf_page_index),
            pdf_page_number: pdf_context.and_then(|pdf| pdf.pdf_page_number),
            pdf_page_count: pdf_context.and_then(|pdf| pdf.pdf_page_count),
            audio_analysis: media.audio_analysis.clone(),
            ocr_text: ocr_analysis.text,
            ocr_frames: ocr_analysis.frames,
            visual_embedding_model: Some(self.embedder.model_name().to_string()),
            faces: options.face_analysis.faces.clone(),
            people: options.face_analysis.person_clusters.clone(),
            artifacts,
            tags: Vec::new(),
            photo_metadata: options.photo_metadata.clone(),
            scene_clip_url,
            scene_index: video_scene
                .map(|scene| scene.scene_index)
                .or_else(|| audio_segment.map(|segment| segment.scene_index)),
            scene_start_frame: video_scene.map(|scene| scene.start.frame_index),
            scene_end_frame: video_scene.map(|scene| scene.end.frame_index),
            scene_start_seconds: video_scene
                .map(|scene| scene.start.timestamp.seconds())
                .or_else(|| audio_segment.map(|segment| segment.start_seconds)),
            scene_end_seconds: video_scene
                .map(|scene| scene.end.timestamp.seconds())
                .or_else(|| audio_segment.map(|segment| segment.end_seconds)),
            source_type: source_image.source_type.clone(),
            source_item_uri: Some(source_image.item_uri.clone()),
            indexing_profile: Some(indexing_profile(settings)),
            source_uri: Some(source_image.source_uri.clone()),
        })
    }
}
