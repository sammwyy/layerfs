use layerfs_core::Checkpoint;

/// Resolved kernel/initramfs paths for one boot artifact generation.
pub struct BootTierPaths {
    pub kernel: String,
    pub initramfs: String,
}

/// Store location and boot artifact generations shared by every entry.
/// `update`/`head` are optional — a fresh install may only have `base`.
pub struct Options {
    pub base: Option<BootTierPaths>,
    pub update: Option<BootTierPaths>,
    pub head: Option<BootTierPaths>,
    pub store: String,
    /// Adapter names to activate this boot, e.g. `["dnf", "apt"]`.
    pub integrations: Vec<String>,
    /// Appended verbatim after the `layerfs.*` parameters on every entry
    /// (e.g. `console=ttyS0` for a serial-only test boot, or distro
    /// parameters like `quiet rhgb`). Empty by default.
    pub extra_cmdline: String,
    /// Initramfs program selected through `rdinit`, when LayerFS owns it.
    pub rdinit: Option<String>,
}

#[derive(Clone, Copy)]
enum BootTier {
    Head,
    Update,
    Base,
}

struct Entry {
    title: &'static str,
    checkpoint: Checkpoint,
    head_off: bool,
    boot: BootTier,
}

/// The five hardcoded GRUB entries from section 8 of the design notes.
/// Order matters: it is GRUB's default entry index. Each entry's boot
/// tier must match its rootfs state (section 30): entries that include
/// UPDATE_HEAD boot the newest kernel, Previous Update boots the one
/// before it, and Base Recovery boots the original factory kernel.
const ENTRIES: [Entry; 5] = [
    Entry {
        title: "Fedora Linux",
        checkpoint: Checkpoint::Normal,
        head_off: false,
        boot: BootTier::Head,
    },
    Entry {
        title: "Fedora Linux — Safe Mode",
        checkpoint: Checkpoint::Safe,
        head_off: false,
        boot: BootTier::Head,
    },
    Entry {
        title: "Fedora Linux — System Only",
        checkpoint: Checkpoint::System,
        head_off: false,
        boot: BootTier::Head,
    },
    Entry {
        title: "Fedora Linux — Previous Update",
        checkpoint: Checkpoint::Safe,
        head_off: true,
        boot: BootTier::Update,
    },
    Entry {
        title: "Fedora Linux — Base Recovery",
        checkpoint: Checkpoint::Base,
        head_off: false,
        boot: BootTier::Base,
    },
];

/// Renders the checkpoint menu entries as GRUB configuration syntax,
/// suitable as the stdout of an `/etc/grub.d/` script. An entry whose
/// required boot tier (and fallbacks) has no registered artifacts is
/// skipped rather than pointing GRUB at a kernel that doesn't exist.
pub fn render(opts: &Options) -> String {
    let mut out = String::new();

    for entry in &ENTRIES {
        let Some(boot) = resolve_tier(opts, entry.boot) else {
            continue;
        };

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
        if let Some(rdinit) = &opts.rdinit {
            cmdline.push_str(" rdinit=");
            cmdline.push_str(rdinit);
        }

        out.push_str(&format!(
            "menuentry '{title}' {{\n    linux {linux} {cmdline}\n    initrd {initrd}\n}}\n",
            title = entry.title,
            linux = boot.kernel,
            cmdline = cmdline,
            initrd = boot.initramfs,
        ));
    }

    out
}

/// Falls back to a lower tier when the requested one has no artifacts —
/// booting an older kernel against a newer rootfs is safer than emitting
/// a menu entry pointing at nothing.
fn resolve_tier(opts: &Options, tier: BootTier) -> Option<&BootTierPaths> {
    match tier {
        BootTier::Head => opts
            .head
            .as_ref()
            .or(opts.update.as_ref())
            .or(opts.base.as_ref()),
        BootTier::Update => opts.update.as_ref().or(opts.base.as_ref()),
        BootTier::Base => opts.base.as_ref(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(label: &str) -> BootTierPaths {
        BootTierPaths {
            kernel: format!("/boot/{label}/vmlinuz"),
            initramfs: format!("/boot/{label}/initramfs.img"),
        }
    }

    fn opts() -> Options {
        Options {
            base: Some(tier("base")),
            update: Some(tier("update")),
            head: Some(tier("head")),
            store: "/run/layerfs-store".to_string(),
            integrations: Vec::new(),
            extra_cmdline: String::new(),
            rdinit: None,
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
    fn rdinit_is_repeated_on_every_entry() {
        let mut opts = opts();
        opts.rdinit = Some("/sbin/layerfs-init".to_string());
        let out = render(&opts);
        assert_eq!(out.matches("rdinit=/sbin/layerfs-init").count(), 5);
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

    #[test]
    fn each_entry_uses_its_own_boot_tier() {
        let out = render(&opts());
        let normal = out.split("'Fedora Linux' {").nth(1).unwrap();
        assert!(normal.contains("/boot/head/vmlinuz"));

        let previous = out.split("Previous Update").nth(1).unwrap();
        assert!(previous.contains("/boot/update/vmlinuz"));

        let base = out.split("Base Recovery").nth(1).unwrap();
        assert!(base.contains("/boot/base/vmlinuz"));
    }

    #[test]
    fn missing_head_falls_back_to_update_then_base() {
        let mut opts = opts();
        opts.head = None;
        let out = render(&opts);
        let normal = out.split("'Fedora Linux' {").nth(1).unwrap();
        assert!(normal.contains("/boot/update/vmlinuz"));

        opts.update = None;
        let out = render(&opts);
        let normal = out.split("'Fedora Linux' {").nth(1).unwrap();
        assert!(normal.contains("/boot/base/vmlinuz"));
    }

    #[test]
    fn missing_base_skips_base_recovery_entirely() {
        let mut opts = opts();
        opts.base = None;
        let out = render(&opts);
        assert!(!out.contains("Base Recovery"));
        // The other four entries fall back to update, since base is gone too.
        assert_eq!(
            out.lines().filter(|l| l.starts_with("menuentry")).count(),
            4
        );
    }
}
