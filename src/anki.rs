use anyhow::{Context, Result};

use crate::format::Format;
use crate::subtitle::{Rect, Subtitle};
use genanki_rs::{Field, Model, Note, Template};
use log::trace;

fn create_note(
    model: Model,
    idx: &str,
    rect: &Rect,
    sub_format: Format<'_>,
    image: Option<&str>,
    audio: Option<&str>,
    media: &mut Vec<String>,
) -> Result<Note> {
    if let Some(image) = image {
        media.push(image.to_string()); //TODO basename only https://docs.rs/genanki-rs/0.3.1/genanki_rs/index.html#media-files
    }
    if let Some(audio) = audio {
        media.push(audio.to_string()); //TODO basename only
    }

    let audio_field = audio.map(to_audio).unwrap_or("".to_string());
    let image_field = image.map(to_image).unwrap_or("".to_string());

    let text_field = match rect {
        Rect::Text(text) => text.clone(),
        Rect::Ass(ass) => ass.text.dialogue.clone(),
        Rect::Bitmap(_) => {
            let sub_image = sub_format.to_string();
            media.push(sub_image.clone()); //TODO basename only
            to_image(&sub_image)
        }
    };

    Note::new(model, vec![idx, &image_field, &audio_field, &text_field])
        .context("Failed to create note")
}

fn to_audio(path: &str) -> String {
    format!("[sound:{}]", path)
}

fn to_image(path: &str) -> String {
    format!("<img src=\"{}\">", path)
}

fn create_notes(
    offset: usize,
    model: &Model,
    subs: &[Subtitle],
    mut sub_format: Format<'_>,
    image_format: Option<Format<'_>>,
    audio_format: Option<Format<'_>>,
    media: &mut Vec<String>,
) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    let mut index = offset;

    for (sub_index, sub) in subs.iter().enumerate() {
        sub_format.set_sub_index(sub_index);
        if !sub.rects.is_empty() {
            sub_format.set_rect_count(sub.rects.len()).unwrap();
        }

        let image = image_format.map(|mut format| format.set_sub_index(sub_index).to_string());
        let audio = audio_format.map(|mut format| format.set_sub_index(sub_index).to_string());
        for (rect_index, rect) in sub.iter().enumerate() {
            sub_format.set_rect_index(rect_index);
            notes.push(create_note(
                model.clone(),
                &index.to_string(),
                rect,
                sub_format,
                image.as_deref(),
                audio.as_deref(),
                media,
            )?);
            index += 1;
        }
    }
    Ok(notes)
}

pub fn create_all_notes(
    files: &Vec<Vec<Subtitle>>,
    sub_format: &str,
    image_format: Option<&str>,
    audio_format: Option<&str>,
) -> Result<(Vec<Note>, Vec<String>)> {
    let model = Model::new(
        8815489913192057416,
        "stos anki model",
        vec![
            Field::new("Sequence indicator"),
            Field::new("Image"),
            Field::new("Audio"),
            Field::new("Text"),
        ],
        vec![Template::new("Card 1")
            .qfmt("{{Text}}")
            .afmt("{{Image}}<br>{{Audio}}<br>{{Text}}")],
    );
    trace!("created note model");

    let mut notes = Vec::new();
    let mut media = Vec::new();

    trace!("converting {} files to notes", files.len());
    for (file_idx, subs) in files.iter().enumerate() {
        let sub_format = Format::new(subs.len(), files.len(), sub_format)?;
        let image_format = if let Some(format) = image_format {
            Some(Format::new(subs.len(), files.len(), format)?)
        } else {
            None
        };
        let audio_format = if let Some(format) = audio_format {
            Some(Format::new(subs.len(), files.len(), format)?)
        } else {
            None
        };

        notes.extend(create_notes(
            notes.len(),
            &model,
            subs,
            sub_format,
            image_format.map(|mut format| *format.set_file_index(file_idx)),
            audio_format.map(|mut format| *format.set_file_index(file_idx)),
            &mut media,
        )?)
    }

    Ok((notes, media))
}
