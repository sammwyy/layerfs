const READ_ONLY_SHORT: &[char] = &['Q', 'F', 'T', 'V', 'h'];
const READ_ONLY_LONG: &[&str] = &["--query", "--files", "--deptest", "--version", "--help"];

const MUTATING_SHORT: &[char] = &['R', 'U', 'D'];
const MUTATING_LONG: &[&str] = &["--remove", "--upgrade", "--database"];

/// Modifiers that make `-S`/`--sync` informational instead of installing.
const SYNC_READ_ONLY_MODIFIERS: &[char] = &['s', 'i', 'l', 'p', 'w'];
const SYNC_READ_ONLY_MODIFIERS_LONG: &[&str] =
    &["--search", "--info", "--list", "--print", "--downloadonly"];

fn short_chars(arg: &str) -> impl Iterator<Item = char> + '_ {
    arg.strip_prefix('-')
        .filter(|rest| !rest.starts_with('-'))
        .into_iter()
        .flat_map(str::chars)
}

/// Reads pacman's operation (`-S`/`-R`/`-U`/`-Q`/...) and, for `-S`, whether
/// an informational modifier (`-Ss`, `-Si`, `-Sw`, ...) downgrades it to
/// read-only. Unrecognized non-empty invocations default to mutating.
pub fn is_mutating(args: &[String]) -> bool {
    if args.is_empty() {
        return false;
    }

    let shorts: Vec<char> = args.iter().flat_map(|a| short_chars(a)).collect();
    let has_long = |set: &[&str]| args.iter().any(|a| set.contains(&a.as_str()));

    if shorts.iter().any(|c| READ_ONLY_SHORT.contains(c)) || has_long(READ_ONLY_LONG) {
        return false;
    }
    if shorts.iter().any(|c| MUTATING_SHORT.contains(c)) || has_long(MUTATING_LONG) {
        return true;
    }

    let is_sync = shorts.contains(&'S') || args.iter().any(|a| a == "--sync");
    if is_sync {
        let read_only_modifier = shorts.iter().any(|c| SYNC_READ_ONLY_MODIFIERS.contains(c))
            || has_long(SYNC_READ_ONLY_MODIFIERS_LONG);
        return !read_only_modifier;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn install_remove_upgrade_are_mutating() {
        for cmd in ["-S vim", "-Syu", "-R vim", "-U /tmp/pkg.pkg.tar.zst"] {
            assert!(is_mutating(&args(cmd)), "{cmd} should be mutating");
        }
    }

    #[test]
    fn query_search_info_are_read_only() {
        for cmd in ["-Q", "-Qi vim", "-Ss vim", "-Si vim", "-Fl", "-V", "-h"] {
            assert!(!is_mutating(&args(cmd)), "{cmd} should be read-only");
        }
    }

    #[test]
    fn sync_downloadonly_overrides_to_read_only() {
        assert!(!is_mutating(&args("-Sw vim")));
        assert!(!is_mutating(&args("--sync --downloadonly vim")));
    }

    #[test]
    fn bare_refresh_without_upgrade_is_still_mutating() {
        assert!(is_mutating(&args("-Sy")));
    }

    #[test]
    fn no_args_is_not_mutating() {
        assert!(!is_mutating(&args("")));
    }

    #[test]
    fn unrecognized_args_default_to_mutating() {
        assert!(is_mutating(&args("--some-future-flag")));
    }

    #[test]
    fn database_op_is_mutating() {
        assert!(is_mutating(&args("-D --asdeps vim")));
    }
}
