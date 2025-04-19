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
