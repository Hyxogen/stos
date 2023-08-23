use super::SubtitleBundle;
use crate::subtitle::Dialogue;
use anyhow::{Context, Result};
use genanki_rs::{Field, Model, Note, Template};

fn to_audio<S: AsRef<str>>(path: S) -> String {
    format!("[sound:{}]", path.as_ref())
}

fn to_image<S: AsRef<str>>(path: S) -> String {
    format!("<img src=\"{}\">", path.as_ref())
}

pub fn create_notes<'a, I>(subs: I) -> Result<Vec<Note>>
where
    I: Iterator<Item = &'a SubtitleBundle>,
{
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
            .qfmt("{{Image}}<br>{{Audio}}<br><h1 style=\"text-align: center\">{{Text}}")
            .afmt("{{Image}}<br>{{Audio}}<br><h1 style=\"text-align: center\">{{Text}}")],
    );

    let mut res = Vec::new();

    for (model, (idx, sub)) in std::iter::repeat(model).zip(subs.enumerate()) {
        let idx = format!("{}", idx);
        let image = sub.image().map(to_image).unwrap_or("".to_string());
        let audio = sub.audio().map(to_audio).unwrap_or("".to_string());
        let diag = match sub.sub().dialogue() {
            Dialogue::Text(text) => text.clone(),
            Dialogue::Ass(ass) => ass.text.dialogue.clone(),
            Dialogue::Bitmap(_) => sub.sub_image().map(to_image).unwrap_or("".to_string()),
        };

        res.push(
            Note::new(model, vec![&idx, &image, &audio, &diag]).context("Failed to create note")?,
        )
    }
    Ok(res)
}
