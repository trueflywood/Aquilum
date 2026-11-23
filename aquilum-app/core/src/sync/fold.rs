use unicode_normalization::UnicodeNormalization;

pub fn fold_path(path: &str) -> String {
    path.nfc().collect::<String>().to_lowercase()
}

pub fn same_fold(left: &str, right: &str) -> bool {
    fold_path(left) == fold_path(right)
}

pub fn local_case_conflicts(paths: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    let mut groups: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for path in paths {
        let path = path.as_ref().to_owned();
        groups.entry(fold_path(&path)).or_default().push(path);
    }
    let mut conflicts: Vec<String> = groups.into_values().filter(|group| group.len() > 1).flatten().collect();
    conflicts.sort();
    conflicts
}

#[cfg(test)]
mod tests {
    use super::{fold_path, local_case_conflicts, same_fold};

    #[test]
    fn cyrillic_case_folds_together() {
        assert!(same_fold("Заметки/А.md", "заметки/а.md"));
        assert_eq!(fold_path("Заметки/А.md"), fold_path("заметки/а.md"));
    }

    #[test]
    fn lists_every_path_in_a_case_group() {
        assert_eq!(
            local_case_conflicts(["Note.md", "other.md", "note.md"]),
            vec!["Note.md".to_owned(), "note.md".to_owned()]
        );
        assert!(local_case_conflicts(["Note.md", "Other.md"]).is_empty());
    }
}
