//! Modifica minima di file .ini: cambia o aggiunge una chiave lasciando intatto il resto.

fn section_name(line: &str) -> Option<&str> {
    let line = line.trim();
    line.strip_prefix('[')?.strip_suffix(']').map(str::trim)
}

fn key_of(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.starts_with([';', '#']) {
        return None;
    }
    let end = line.find(['=', ':'])?;
    Some(line[..end].trim())
}

/// Legge il valore di `key` nella sezione `section` (nomi senza distinzione di maiuscole).
pub fn get(text: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines() {
        if let Some(name) = section_name(line) {
            inside = name.eq_ignore_ascii_case(section);
        } else if inside && key_of(line).is_some_and(|k| k.eq_ignore_ascii_case(key)) {
            let value = &line[line.find(['=', ':'])? + 1..];
            return Some(value.trim().to_string());
        }
    }
    None
}

/// Imposta `key = value` nella sezione, creando sezione e chiave se mancano.
pub fn set(text: &str, section: &str, key: &str, value: &str) -> String {
    let entry = format!("{key} = {value}");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut inside = false;
    let mut section_end = None;
    for i in 0..lines.len() {
        if let Some(name) = section_name(&lines[i]) {
            if inside {
                break;
            }
            inside = name.eq_ignore_ascii_case(section);
            if inside {
                section_end = Some(i + 1);
            }
        } else if inside {
            if key_of(&lines[i]).is_some_and(|k| k.eq_ignore_ascii_case(key)) {
                lines[i] = entry;
                return lines.join("\n") + "\n";
            }
            if !lines[i].trim().is_empty() {
                section_end = Some(i + 1);
            }
        }
    }
    match section_end {
        Some(at) => lines.insert(at, entry),
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!("[{section}]"));
            lines.push(entry);
        }
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_key_in_the_right_section() {
        let text = "[A]\nkey = 1\n\n[B]\nkey = 2\n";
        let out = set(text, "b", "KEY", "9");
        assert_eq!(out, "[A]\nkey = 1\n\n[B]\nKEY = 9\n");
        assert_eq!(get(&out, "A", "key").as_deref(), Some("1"));
        assert_eq!(get(&out, "B", "key").as_deref(), Some("9"));
    }

    #[test]
    fn adds_key_at_the_end_of_its_section() {
        let text = "[A]\nx = 1\n\n[B]\ny = 2\n";
        assert_eq!(set(text, "A", "z", "3"), "[A]\nx = 1\nz = 3\n\n[B]\ny = 2\n");
    }

    #[test]
    fn adds_missing_section() {
        assert_eq!(set("", "A", "k", "v"), "[A]\nk = v\n");
        assert_eq!(set("[A]\nx = 1\n", "B", "k", "v"), "[A]\nx = 1\n\n[B]\nk = v\n");
    }

    #[test]
    fn ignores_commented_keys_and_empty_values() {
        let text = "[A]\n; k = old\nk = \n";
        assert_eq!(get(text, "A", "k").as_deref(), Some(""));
        assert_eq!(set(text, "A", "k", "v"), "[A]\n; k = old\nk = v\n");
    }
}
