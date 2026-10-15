//! YAML frontmatter at the top of skill, agent, and command files.

use serde::Deserialize;

/// The frontmatter fields harness-hanger reads. Other keys are ignored.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Document<'a> {
    pub frontmatter: Frontmatter,
    pub body: &'a str,
}

impl Document<'_> {
    /// The frontmatter description, else the first non-empty line of the body, as Claude Code does.
    pub fn description(&self) -> Option<String> {
        self.frontmatter
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
            .or_else(|| first_line(self.body))
            .map(str::to_owned)
    }
}

/// Splits a Markdown file into its frontmatter and body. A file without frontmatter has an empty one.
///
/// Frontmatter that is not valid YAML is read line by line, because Claude Code accepts it too.
/// Published plugins rely on that, for example with an unquoted `: ` inside a description.
pub(crate) fn parse(text: &str) -> Document<'_> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some((yaml, body)) = split(text) else {
        return Document {
            frontmatter: Frontmatter::default(),
            body: text,
        };
    };
    let frontmatter = if yaml.trim().is_empty() {
        Frontmatter::default()
    } else {
        serde_saphyr::from_str(yaml).unwrap_or_else(|_| read_lines(yaml))
    };
    Document { frontmatter, body }
}

/// Reads top-level `key: value` lines, skipping values that YAML would not read as plain text.
fn read_lines(yaml: &str) -> Frontmatter {
    let mut frontmatter = Frontmatter::default();
    for line in yaml.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        let plain = !value.is_empty() && !value.starts_with(['[', '{', '|', '>', '&', '*']);
        let value = plain.then(|| unquote(value).to_owned());
        match key {
            "name" => frontmatter.name = value,
            "description" => frontmatter.description = value,
            _ => {}
        }
    }
    frontmatter
}

fn unquote(value: &str) -> &str {
    ['"', '\'']
        .into_iter()
        .find_map(|quote| value.strip_prefix(quote)?.strip_suffix(quote))
        .unwrap_or(value)
}

/// Returns the YAML between the opening and closing `---` lines, and the text after them.
fn split(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

fn first_line(body: &str) -> Option<&str> {
    body.lines()
        .map(|line| line.trim().trim_start_matches('#').trim_start())
        .find(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_name_and_description() {
        let doc = parse("---\nname: review\ndescription: Reviews code.\nmodel: opus\n---\nBody\n");
        assert_eq!(doc.frontmatter.name.as_deref(), Some("review"));
        assert_eq!(doc.description().as_deref(), Some("Reviews code."));
        assert_eq!(doc.body, "Body\n");
    }

    #[test]
    fn reads_folded_multi_line_descriptions() {
        let doc = parse("---\ndescription: >-\n  First part\n  and second part.\n---\n");
        assert_eq!(
            doc.description().as_deref(),
            Some("First part and second part.")
        );
    }

    #[test]
    fn handles_crlf_and_a_byte_order_mark() {
        let doc = parse("\u{feff}---\r\nname: x\r\n---\r\nBody\r\n");
        assert_eq!(doc.frontmatter.name.as_deref(), Some("x"));
        assert_eq!(doc.body, "Body\r\n");
    }

    #[test]
    fn falls_back_to_the_first_body_line_for_the_description() {
        let doc = parse("---\nname: x\n---\n\n# Deploy the app\n\nSteps.\n");
        assert_eq!(doc.description().as_deref(), Some("Deploy the app"));
    }

    #[test]
    fn a_file_without_frontmatter_is_all_body() {
        let doc = parse("Just text\n");
        assert!(doc.frontmatter.name.is_none());
        assert_eq!(doc.description().as_deref(), Some("Just text"));
    }

    #[test]
    fn an_unclosed_frontmatter_is_treated_as_body() {
        let doc = parse("---\nname: x\n");
        assert!(doc.frontmatter.name.is_none());
    }

    #[test]
    fn reads_frontmatter_that_is_not_valid_yaml() {
        let doc = parse(
            "---\nname: 'hunter'\ndescription: Use it when: errors are swallowed.\nargument-hint: [system] [--source <path>]\n---\n",
        );
        assert_eq!(doc.frontmatter.name.as_deref(), Some("hunter"));
        assert_eq!(
            doc.description().as_deref(),
            Some("Use it when: errors are swallowed.")
        );
    }

    #[test]
    fn skips_values_that_are_not_plain_text_in_invalid_yaml() {
        let doc = parse("---\nname: [unclosed\ndescription: |\n---\nBody\n");
        assert_eq!(doc.frontmatter.name, None);
        assert_eq!(doc.description().as_deref(), Some("Body"));
    }
}
