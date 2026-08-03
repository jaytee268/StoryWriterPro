pub fn editor_content_to_plain_text(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut tag = String::new();
    let mut inside = false;
    for character in content.chars() {
        if character == '<' {
            inside = true;
            tag.clear();
            continue;
        }
        if inside {
            if character == '>' {
                let lower = tag.to_ascii_lowercase();
                if lower.starts_with("br") {
                    output.push('\n');
                }
                if lower.starts_with("/p")
                    || lower.starts_with("/div")
                    || lower.starts_with("/li")
                    || lower.starts_with("/blockquote")
                {
                    output.push('\n');
                }
                inside = false;
            } else {
                tag.push(character);
            }
        } else {
            output.push(character);
        }
    }
    output
        .replace('\u{00a0}', " ")
        .replace("\n\n\n", "\n\n")
        .trim_end()
        .to_string()
}
