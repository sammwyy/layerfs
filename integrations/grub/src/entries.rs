use layerfs_core::Checkpoint;

/// Boot artifact paths and store location shared by every generated entry.
/// Paths are passed through verbatim, exactly as GRUB will resolve them —
/// this crate has no opinion on partition layout (section 30, boot
/// artifact transactions, is not implemented yet).
pub struct Options {
    pub linux: String,
    pub initrd: String,
    pub store: String,
    /// Adapter names to activate this boot, e.g. `["dnf", "apt"]`.
    pub integrations: Vec<String>,
    /// Appended verbatim after the `layerfs.*` parameters on every entry
    /// (e.g. `console=ttyS0` for a serial-only test boot, or distro
    /// parameters like `quiet rhgb`). Empty by default.
    pub extra_cmdline: String,
}

struct Entry {
    title: &'static str,
    checkpoint: Checkpoint,
    head_off: bool,
}

/// The five hardcoded GRUB entries from section 8 of the design notes.
/// Order matters: it is GRUB's default entry index.
const ENTRIES: [Entry; 5] = [
    Entry {
        title: "Fedora Linux",
        checkpoint: Checkpoint::Normal,
        head_off: false,
    },
    Entry {
        title: "Fedora Linux — Safe Mode",
        checkpoint: Checkpoint::Safe,
        head_off: false,
    },
    Entry {
        title: "Fedora Linux — System Only",
        checkpoint: Checkpoint::System,
        head_off: false,
    },
    Entry {
        title: "Fedora Linux — Previous Update",
        checkpoint: Checkpoint::Safe,
        head_off: true,
    },
    Entry {
        title: "Fedora Linux — Base Recovery",
        checkpoint: Checkpoint::Base,
        head_off: false,
    },
];

/// Renders the five checkpoint menu entries as GRUB configuration syntax,
/// suitable as the stdout of an `/etc/grub.d/` script.
pub fn render(opts: &Options) -> String {
    let mut out = String::new();

    for entry in &ENTRIES {
        let mut cmdline = format!("layerfs.checkpoint={}", entry.checkpoint.name());
        if entry.head_off {
            cmdline.push_str(" layerfs.head=off");
        }
        cmdline.push_str(" layerfs.store=");
        cmdline.push_str(&opts.store);
        if !opts.integrations.is_empty() {
            cmdline.push_str(" layerfs.integrations=");
            cmdline.push_str(&opts.integrations.join(","));
        }
        if !opts.extra_cmdline.is_empty() {
            cmdline.push(' ');
            cmdline.push_str(&opts.extra_cmdline);
        }

        out.push_str(&format!(
            "menuentry '{title}' {{\n    linux {linux} {cmdline}\n    initrd {initrd}\n}}\n",
            title = entry.title,
            linux = opts.linux,
            cmdline = cmdline,
            initrd = opts.initrd,
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            linux: "/boot/vmlinuz".to_string(),
            initrd: "/boot/initramfs.img".to_string(),
            store: "/run/layerfs-store".to_string(),
            integrations: Vec::new(),
            extra_cmdline: String::new(),
        }
    }

    #[test]
    fn emits_five_entries_in_spec_order() {
        let out = render(&opts());
        let titles: Vec<_> = out.lines().filter(|l| l.starts_with("menuentry")).collect();
        assert_eq!(
            titles,
            [
                "menuentry 'Fedora Linux' {",
                "menuentry 'Fedora Linux — Safe Mode' {",
                "menuentry 'Fedora Linux — System Only' {",
                "menuentry 'Fedora Linux — Previous Update' {",
                "menuentry 'Fedora Linux — Base Recovery' {",
            ]
        );
    }

    #[test]
    fn previous_update_is_safe_with_head_off() {
        let out = render(&opts());
        let block = out.split("Previous Update").nth(1).unwrap();
        let linux_line = block
            .lines()
            .find(|l| l.trim_start().starts_with("linux"))
            .unwrap();
        assert!(linux_line.contains("layerfs.checkpoint=safe"));
        assert!(linux_line.contains("layerfs.head=off"));
    }

    #[test]
    fn every_entry_carries_the_store_path() {
        let out = render(&opts());
        assert_eq!(out.matches("layerfs.store=/run/layerfs-store").count(), 5);
    }

    #[test]
    fn base_recovery_has_no_head_flag() {
        let out = render(&opts());
        let block = out.split("Base Recovery").nth(1).unwrap();
        let linux_line = block
            .lines()
            .find(|l| l.trim_start().starts_with("linux"))
            .unwrap();
        assert!(linux_line.contains("layerfs.checkpoint=base"));
        assert!(!linux_line.contains("layerfs.head"));
    }

    #[test]
    fn extra_cmdline_is_appended_to_every_entry() {
        let mut opts = opts();
        opts.extra_cmdline = "console=ttyS0".to_string();
        let out = render(&opts);
        assert_eq!(out.matches("console=ttyS0").count(), 5);
    }

    #[test]
    fn empty_extra_cmdline_adds_no_trailing_space() {
        let out = render(&opts());
        assert!(!out.contains("layerfs-store \n"));
    }

    #[test]
    fn integrations_list_is_joined_and_repeated_on_every_entry() {
        let mut opts = opts();
        opts.integrations = vec!["dnf".to_string(), "apt".to_string()];
        let out = render(&opts);
        assert_eq!(out.matches("layerfs.integrations=dnf,apt").count(), 5);
    }

    #[test]
    fn no_integrations_flag_when_list_is_empty() {
        let out = render(&opts());
        assert!(!out.contains("layerfs.integrations"));
    }
}
