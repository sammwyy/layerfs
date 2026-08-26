/// DNF subcommands that never mutate the package database. Anything else,
/// including unrecognized verbs, defaults to mutating — safer to wrap.
const READ_ONLY_VERBS: &[&str] = &[
    "list",
    "info",
    "search",
    "provides",
    "repoquery",
    "repolist",
    "repo-list",
    "repoinfo",
    "repo-info",
    "check-update",
    "makecache",
    "help",
    "deplist",
];

/// `--downloadonly` fetches to cache without touching the installed set,
/// even on an otherwise-mutating verb like `install`.
const FORCES_READ_ONLY: &[&str] = &["--downloadonly"];

/// Checks the first non-flag argument against `READ_ONLY_VERBS`. A
/// value-taking flag placed before the subcommand (`-c FILE install x`)
/// can be misread as the subcommand itself.
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
    fn install_remove_upgrade_distro_sync_are_mutating() {
        for cmd in ["install foo", "remove foo", "upgrade", "distro-sync"] {
            assert!(is_mutating(&args(cmd)), "{cmd} should be mutating");
        }
    }

    #[test]
    fn search_info_list_are_read_only() {
        for cmd in ["search bash", "info bash", "list installed"] {
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
    fn downloadonly_overrides_a_mutating_verb() {
        assert!(!is_mutating(&args("install foo --downloadonly")));
        assert!(!is_mutating(&args("upgrade --downloadonly")));
    }
}
