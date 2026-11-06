use crate::attachment_keywords::ATTACHMENT_KEYWORDS;

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|character| !matches!(*character, '\u{0300}'..='\u{036f}'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn matches_keyword(text: &str, keyword: &str) -> bool {
    text.match_indices(keyword).any(|(start, _)| {
        !text[..start]
            .chars()
            .next_back()
            .is_some_and(word_character)
            && !text[start + keyword.len()..]
                .chars()
                .next()
                .is_some_and(word_character)
    })
}

fn detect(subject: &str, body: &str, languages: &[(&str, &[&str])]) -> bool {
    let subject = normalize(subject);
    let body = normalize(body);
    languages
        .iter()
        .flat_map(|(_, words)| words.iter())
        .any(|keyword| {
            let keyword = normalize(keyword);
            !keyword.is_empty()
                && (matches_keyword(&subject, &keyword) || matches_keyword(&body, &keyword))
        })
}

#[tauri::command(async)]
pub fn mentions_attachment(subject: String, body: String) -> Result<bool, String> {
    // Match the local draft size bound before scanning frontend-controlled text.
    if subject.len().saturating_add(body.len()) > 36 * 1024 * 1024 {
        return Err("Message is too large to check for attachment references".into());
    }
    Ok(detect(&subject, &body, ATTACHMENT_KEYWORDS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_english_and_spanish_in_subject_or_body() {
        for body in [
            "Please see the attached invoice.",
            "I am attaching the report.",
            "The attachments have the details.",
            "Please find the enclosed document.",
            "Te envío el documento adjunto.",
            "ADJUNTÉ los archivos.",
            "Adjunte\u{301} los archivos.",
            "Adjuntamos los documentos.",
            "Revisa los anexos.",
        ] {
            assert!(detect("Invoice", body, ATTACHMENT_KEYWORDS), "{body}");
        }
        assert!(detect("Attached documents", "Thanks!", ATTACHMENT_KEYWORDS));
        assert!(detect(
            "Documentos adjuntos",
            "Gracias.",
            ATTACHMENT_KEYWORDS
        ));
        for (_, words) in ATTACHMENT_KEYWORDS {
            for keyword in *words {
                assert!(
                    detect("", &format!("({keyword})"), ATTACHMENT_KEYWORDS),
                    "{keyword}"
                );
            }
        }
    }

    #[test]
    fn does_not_match_unrelated_text_or_parts_of_words() {
        for body in [
            "",
            "Thank you!",
            "Estamos juntos.",
            "attachmentless",
            "reattached",
            "anexos123",
            "pre_adjuntos",
            "éattachmenté",
        ] {
            assert!(!detect("Hello", body, ATTACHMENT_KEYWORDS), "{body}");
        }
    }

    #[test]
    fn additional_languages_and_phrases_need_only_catalog_entries() {
        let languages: &[(&str, &[&str])] = &[
            ("fr", &["pièce jointe"]),
            ("ru", &["приложение"]),
            ("example", &["file (attached)"]),
        ];
        assert!(detect("", "Voici la pièce jointe.", languages));
        assert!(detect("", "Смотрите приложение.", languages));
        assert!(detect("", "file (attached)", languages));
        assert!(!detect("", "file attached", languages));
        assert!(!detect(
            "",
            "Do not match an empty keyword",
            &[("empty", &[""])]
        ));
    }

    #[test]
    fn bounds_frontend_input() {
        assert!(mentions_attachment("Invoice".into(), "Attached is the file.".into()).unwrap());
        assert!(mentions_attachment(String::new(), "x".repeat(36 * 1024 * 1024 + 1)).is_err());
    }
}
