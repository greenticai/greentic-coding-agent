#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heading<'a> {
    pub level: usize,
    pub title: &'a str,
    pub line_number: usize,
}

pub fn extract_markdown_headings(input: &str) -> Vec<Heading<'_>> {
    let mut headings = Vec::new();

    for (line_number, line) in input.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut level = 0usize;

        while level < bytes.len() && bytes[level] == b'#' {
            level += 1;
        }

        if level == 0 || level > 6 || bytes.get(level) != Some(&b' ') {
            continue;
        }

        let title = line[level + 1..].trim();
        if title.is_empty() {
            continue;
        }

        headings.push(Heading {
            level,
            title,
            line_number: line_number + 1,
        });
    }

    headings
}

pub fn build_heading_index(input: &str) -> Vec<String> {
    let mut entries = Vec::new();

    for heading in extract_markdown_headings(input) {
        let slug = slugify(heading.title);
        entries.push(format!(
            "{}:{}:{}",
            heading.level, heading.line_number, slug
        ));
    }

    entries.sort_unstable();
    entries.dedup();
    entries
}

pub fn repeated_heading_index_workload(input: &str, repeats: usize) -> usize {
    let repeated = input.repeat(repeats);
    let mut total = 0usize;

    for entry in build_heading_index(&repeated) {
        total = total.saturating_add(entry.len());
    }

    total
}

fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut last_was_dash = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.starts_with('-') {
        slug.remove(0);
    }

    slug
}

#[cfg(test)]
mod tests {
    use super::{build_heading_index, extract_markdown_headings};

    #[test]
    fn extracts_markdown_headings_with_line_numbers() {
        let input = "\
Intro
# Top
## Child
### Deeper
####    Trimmed
";

        let headings = extract_markdown_headings(input);

        assert_eq!(headings.len(), 4);
        assert_eq!(headings[0].title, "Top");
        assert_eq!(headings[0].line_number, 2);
        assert_eq!(headings[3].title, "Trimmed");
    }

    #[test]
    fn builds_stable_heading_index_entries() {
        let input = "\
# Alpha Beta
## Alpha Beta
## Symbols & Spaces
";

        let index = build_heading_index(input);

        assert_eq!(
            index,
            vec![
                "1:1:alpha-beta".to_string(),
                "2:2:alpha-beta".to_string(),
                "2:3:symbols-spaces".to_string(),
            ]
        );
    }
}
