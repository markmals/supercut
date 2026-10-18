//! Turning feed HTML and file tags into text worth storing and comparing.

/// Plain text from a feed field: tags dropped, entities decoded, whitespace collapsed.
pub fn plain_text(html: &str) -> String {
    let stripped = strip_tags(html);
    let decoded = html_escape::decode_html_entities(&stripped);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A title reduced for matching: lowercase words, no punctuation, single spaces. A leading
/// episode number (`13. Treehouse Terror`) is dropped, because file tags carry one where feeds
/// usually do not, and `w/` counts as `with`, which the two spell differently just as often.
pub fn normalize_title(title: &str) -> String {
    without_leading_number(title.trim())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| match word.to_lowercase().as_str() {
            "w" => "with".to_owned(),
            lowercase => lowercase.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Only a number followed by punctuation counts, so `911 Calls` keeps its number.
fn without_leading_number(title: &str) -> &str {
    let digits = title.len() - title.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return title;
    }
    let rest = title[digits..].trim_start();
    match rest.strip_prefix(['.', ')', '-', ':']) {
        Some(rest) => rest.trim_start(),
        None => title,
    }
}

/// Entities are decoded after this, so markup written as `&lt;b&gt;` survives as text.
fn strip_tags(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut tag = String::new();
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                if separates_text(&tag) {
                    text.push(' ');
                }
            }
            _ if in_tag => tag.push(character),
            _ => text.push(character),
        }
    }
    text
}

/// Block and break tags stand for a gap; inline tags like `<em>` can sit mid-word.
fn separates_text(tag: &str) -> bool {
    let name = tag
        .trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "p" | "br"
            | "div"
            | "li"
            | "ul"
            | "ol"
            | "tr"
            | "td"
            | "th"
            | "blockquote"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_strips_markup_and_keeps_words_apart() {
        let html =
            "<p>Burt &amp; Joan</p><p>Call <a href=\"https://example.com/x?y=1\">the line</a>.</p>";
        assert_eq!(plain_text(html), "Burt & Joan Call the line.");
    }

    #[test]
    fn plain_text_keeps_inline_tags_from_splitting_words() {
        assert_eq!(plain_text("neigh<em>bor</em>hood\n\n  listen"), "neighborhood listen");
    }

    #[test]
    fn normalized_titles_ignore_case_punctuation_and_spacing() {
        assert_eq!(
            normalize_title("Treehouse Terror,  Toys—from Burnt w/ Mitch Silpa"),
            normalize_title("treehouse terror! toys from burnt w mitch silpa")
        );
        assert_ne!(normalize_title("Bonus Room 12"), normalize_title("Bonus Room 13"));
    }

    #[test]
    fn normalized_titles_ignore_a_numbered_prefix_but_not_a_number_in_the_title() {
        assert_eq!(normalize_title("13. Treehouse Terror"), normalize_title("Treehouse Terror"));
        assert_eq!(normalize_title("7 - Treehouse Terror"), normalize_title("Treehouse Terror"));
        assert_ne!(normalize_title("911 Calls"), normalize_title("Calls"));
    }

    #[test]
    fn normalized_titles_spell_out_the_w_abbreviation() {
        assert_eq!(
            normalize_title("Stolen Wood w/ Bobby"),
            normalize_title("Stolen Wood with Bobby")
        );
    }
}
