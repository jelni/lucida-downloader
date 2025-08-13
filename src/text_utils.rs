use std::borrow::Cow;

use crate::models::Track;

pub fn sanitize_file_name(name: &str) -> String {
    name.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_")
}

pub fn parse_enclosed_value<'a>(start_marker: &str, end_marker: &str, text: &'a str) -> &'a str {
    let start_index = text
        .find(start_marker)
        .unwrap_or_else(|| panic!("{start_marker} not found in {text}"))
        + start_marker.len();

    let end_index = text[start_index..]
        .find(end_marker)
        .unwrap_or_else(|| panic!("{end_marker} not found in {text}"))
        + start_index;

    &text[start_index..end_index]
}

pub fn format_track_stem(
    track: &Track,
    track_number: Option<u32>,
    track_count: u32,
    is_grouped_single: bool,
) -> String {
    let track_number_and_artist = if is_grouped_single {
        Cow::Borrowed("")
    } else {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss
        )]
        let track_number = track_number.map_or_else(String::new, |track_number| {
            format!(
                "{track_number:00$}. ",
                (track_count as f32).log10().floor() as usize + 1
            )
        });

        let artist = if let [artist, ..] = track.artists.as_slice() {
            format!("{} - ", sanitize_file_name(&artist.name))
        } else {
            String::new()
        };

        Cow::Owned(track_number + &artist)
    };

    format!(
        "{track_number_and_artist}{}",
        sanitize_file_name(&track.title)
    )
}
