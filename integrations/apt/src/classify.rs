/// APT/apt-get subcommands that never mutate the installed package set.
/// Anything else, including unrecognized verbs, defaults to mutating —
/// safer to wrap.
const READ_ONLY_VERBS: &[&str] = &[
    "list",
    "search",
    "show",
    "depends",
    "rdepends",
    "policy",
    "changelog",
    "moo",
    "help",
    "update",
];

/// Flags that make an otherwise-mutating verb a no-op against the real
/// system, even on `install`/`upgrade`.
const FORCES_READ_ONLY: &[&str] = &[
    "--dry-run",
    "--simulate",
    "--just-print",
    "-s",
    "--download-only",
    "-d",
];

/// Checks the first non-flag argument against `READ_ONLY_VERBS`. A
/// value-taking flag placed before the subcommand can be misread as the
/// subcommand itself, same caveat as the DNF adapter.
pub fn is_mutating(args: &[String]) -> bool {
    if args.iter().any(|a| FORCES_READ_ONLY.contains(&a.as_str())) {
        return false;
    }

    match args.iter().find(|a| !a.starts_with('-')) {
        Some(verb) => !READ_ONLY_VERBS.contains(&verb.as_str()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn install_remove_upgrade_are_mutating() {
        for cmd in ["install foo", "remove foo", "upgrade", "full-upgrade"] {
            assert!(is_mutating(&args(cmd)), "{cmd} should be mutating");
        }
    }

    #[test]
    fn search_show_list_update_are_read_only() {
        for cmd in ["search bash", "show bash", "list --installed", "update"] {
            assert!(!is_mutating(&args(cmd)), "{cmd} should be read-only");
        }
    }

    #[test]
    fn leading_flags_are_skipped() {
        assert!(is_mutating(&args("-y install foo")));
        assert!(!is_mutating(&args("--quiet search foo")));
    }

    #[test]
    fn unknown_verb_defaults_to_mutating() {
        assert!(is_mutating(&args("some-future-subcommand")));
    }

    #[test]
    fn no_verb_at_all_is_not_mutating() {
        assert!(!is_mutating(&args("--version")));
        assert!(!is_mutating(&args("")));
    }

    #[test]
    fn dry_run_and_download_only_override_a_mutating_verb() {
        assert!(!is_mutating(&args("install foo --dry-run")));
        assert!(!is_mutating(&args("upgrade -s")));
        assert!(!is_mutating(&args("install foo --download-only")));
    }
}
