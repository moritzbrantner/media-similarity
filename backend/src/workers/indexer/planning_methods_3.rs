impl ImageIndexer {
    async fn index_pdf(
        &self,
        source_image: &SourceImage,
        recorder: &mut IndexRunRecorder,
    ) -> Result<IndexOneOutcome, String> {
        let (settings, workflow) = self.workflow_settings(MediaFileKind::Pdf)?;
        recorder.check_cancelled()?;
        let source_pdf_id = image_id_for_uri(&source_image.id_base);
        let (pdf, full_pdf_url) = source_image
            .with_local_media_path(&settings, |path| {
                let pdf = decode_pdf(path, &settings)?;
                let full_pdf_url = expose_source_pdf(path, &source_pdf_id, &settings)?;
                Ok((pdf, full_pdf_url))
            })
            .await?;
        let document_id_base = format!("{}#document", source_image.id_base);
        let document_id = image_id_for_uri(&document_id_base);
        let mut outcome = IndexOneOutcome::default();
        let mut page_texts = Vec::new();

        let total_pages = pdf.pages.len();
        for (index, page) in pdf.pages.iter().enumerate() {
            recorder.current_part("pdf_page", index, total_pages);
            recorder.check_cancelled()?;
            let page_ocr = extract_media_ocr(&page.media, &settings).unwrap_or_else(|error| {
                tracing::warn!(%error, "PDF page OCR extraction failed");
                Default::default()
            });
            let merged_text = merge_pdf_text(&page.embedded_text, &page_ocr.text);
            if !merged_text.is_empty() {
                page_texts.push(merged_text.clone());
            }
            let page_number = page.page_number;
            let page_context = PdfPayloadContext {
                id_base: format!("{}#page={page_number}", source_image.id_base),
                relative_path: format!("{}#page-{page_number:03}", source_image.relative_path),
                filename: format!("{} page {page_number:03}", source_image.filename),
                path: format!("{}#page={page_number}", source_image.display_path),
                full_pdf_url: full_pdf_url.clone(),
                pdf_page_url: full_pdf_url
                    .as_ref()
                    .map(|url| format!("{url}#page={page_number}")),
                pdf_document_id: Some(document_id.clone()),
                pdf_page_index: Some(page.page_index),
                pdf_page_number: Some(page.page_number),
                pdf_page_count: Some(pdf.page_count),
            };
            let face_analysis = FaceAnalysis::default();
            let payload = self.build_payload(
                source_image,
                &page.media,
                &settings,
                PayloadBuildOptions::new(&face_analysis)
                    .with_pdf_context(&page_context)
                    .with_ocr(OcrAnalysis {
                        text: merged_text,
                        frames: page_ocr.frames,
                    }),
            )?;
            let vector = embed_media_with_cancel(
                self.embedder.clone(),
                page.media.sampled_frames.clone(),
                settings.gif_motion_weight,
                recorder,
            )
            .await?;
            recorder.check_cancelled()?;
            let point_id = payload.id.clone();
            await_with_cancel(self.store.upsert_media(&payload, vector), recorder).await??;
            recorder.committed_point(&point_id);
            outcome.insert(point_id);
        }

        if !workflow.processor_enabled("pdf.build_document_summary") {
            return Ok(outcome);
        }

        recorder.current_part("pdf_document", 0, 1);
        recorder.check_cancelled()?;
        let document_text = merge_pdf_text(&pdf.document_text, &page_texts.join(" "));
        let document_context = PdfPayloadContext {
            id_base: document_id_base,
            relative_path: format!("{}#document", source_image.relative_path),
            filename: format!("{} document", source_image.filename),
            path: format!("{}#document", source_image.display_path),
            full_pdf_url,
            pdf_page_url: None,
            pdf_document_id: None,
            pdf_page_index: None,
            pdf_page_number: None,
            pdf_page_count: Some(pdf.page_count),
        };
        let face_analysis = FaceAnalysis::default();
        let payload = self.build_payload(
            source_image,
            &pdf.document_media,
            &settings,
            PayloadBuildOptions::new(&face_analysis)
                .with_pdf_context(&document_context)
                .with_ocr(OcrAnalysis {
                    text: document_text,
                    frames: Vec::new(),
                }),
        )?;
        let vector = embed_media_with_cancel(
            self.embedder.clone(),
            pdf.document_media.sampled_frames.clone(),
            settings.gif_motion_weight,
            recorder,
        )
        .await?;
        recorder.check_cancelled()?;
        let point_id = payload.id.clone();
        await_with_cancel(self.store.upsert_media(&payload, vector), recorder).await??;
        recorder.committed_point(&point_id);
        outcome.insert(point_id);

        Ok(outcome)
    }
}
