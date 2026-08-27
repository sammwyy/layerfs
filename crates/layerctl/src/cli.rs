use std::path::PathBuf;

/// Which boot loader `checkpoint` should target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bootloader {
    SystemdBoot,
    Grub,
}

#[derive(Debug)]
pub enum Command {
    Status,
    Inspect {
        layer: String,
    },
    Diff {
        layer: String,
    },
    Reset {
        path: PathBuf,
    },
    Verify,
    Transaction {
        program: String,
        args: Vec<String>,
    },
    BootRegister {
        name: String,
        kernel: PathBuf,
        initramfs: PathBuf,
    },
    Rollback {
        target: String,
    },
    Rebuild {
        target: String,
    },
    Checkpoint {
        name: String,
        bootloader: Bootloader,
        esp: PathBuf,
        grub_cfg: PathBuf,
        grubenv: PathBuf,
    },
    Install {
        source: PathBuf,
        integrations: Vec<String>,
        adapter_bins: Vec<(String, PathBuf)>,
        grub_entries: Option<PathBuf>,
    },
    ApplyNow {
        live_root: PathBuf,
    },
    Doctor,
}

#[derive(Debug)]
pub struct Invocation {
    /// Store root, from `--store <path>`. Falls back to a fixed default
    /// when absent; see `store::resolve`.
    pub store: Option<PathBuf>,
    pub command: Command,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("missing argument: {0}")]
    MissingArgument(&'static str),
}

/// Parses `layerctl [--store <path>] <command> [args...]`; `--store` is only
/// recognized before the command name, everything after is passed through.
pub fn parse(args: impl Iterator<Item = String>) -> Result<Invocation, CliError> {
    let mut args = args.peekable();
    let mut store = None;

    while let Some(arg) = args.peek() {
        if arg != "--store" {
            break;
        }
        args.next();
        store = Some(PathBuf::from(
            args.next()
                .ok_or(CliError::MissingArgument("--store value"))?,
        ));
    }

    let command_name = args.next().ok_or(CliError::MissingArgument("command"))?;

    let command = match command_name.as_str() {
        "status" => Command::Status,
        "inspect" => Command::Inspect {
            layer: args.next().ok_or(CliError::MissingArgument("layer"))?,
        },
        "diff" => Command::Diff {
            layer: args.next().ok_or(CliError::MissingArgument("layer"))?,
        },
        "reset" => Command::Reset {
            path: args.next().ok_or(CliError::MissingArgument("path"))?.into(),
        },
        "verify" => Command::Verify,
        "transaction" => {
            let mut rest: Vec<String> = args.collect();
            if rest.first().map(String::as_str) == Some("--") {
                rest.remove(0);
            }
            if rest.is_empty() {
                return Err(CliError::MissingArgument("command to run"));
            }
            let program = rest.remove(0);
            Command::Transaction {
                program,
                args: rest,
            }
        }
        "boot-register" => {
            let name = args.next().ok_or(CliError::MissingArgument("name"))?;
            let mut kernel = None;
            let mut initramfs = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--kernel" => kernel = args.next(),
                    "--initramfs" => initramfs = args.next(),
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }
            let (Some(kernel), Some(initramfs)) = (kernel, initramfs) else {
                return Err(CliError::MissingArgument("--kernel and --initramfs"));
            };
            Command::BootRegister {
                name,
                kernel: kernel.into(),
                initramfs: initramfs.into(),
            }
        }
        "rollback" => Command::Rollback {
            target: args.next().ok_or(CliError::MissingArgument("target"))?,
        },
        "rebuild" => Command::Rebuild {
            target: args.next().ok_or(CliError::MissingArgument("target"))?,
        },
        "checkpoint" => {
            let name = args.next().ok_or(CliError::MissingArgument("name"))?;
            let mut bootloader = Bootloader::SystemdBoot;
            let mut esp = PathBuf::from("/boot/efi");
            let mut grub_cfg = PathBuf::from("/boot/grub2/grub.cfg");
            let mut grubenv = PathBuf::from("/boot/grub2/grubenv");
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--bootloader" => {
                        bootloader = match args
                            .next()
                            .ok_or(CliError::MissingArgument("--bootloader value"))?
                            .as_str()
                        {
                            "systemd-boot" => Bootloader::SystemdBoot,
                            "grub" => Bootloader::Grub,
                            other => return Err(CliError::UnknownCommand(other.to_string())),
                        }
                    }
                    "--esp" => {
                        esp = args
                            .next()
                            .ok_or(CliError::MissingArgument("--esp value"))?
                            .into()
                    }
                    "--grub-cfg" => {
                        grub_cfg = args
                            .next()
                            .ok_or(CliError::MissingArgument("--grub-cfg value"))?
                            .into()
                    }
                    "--grubenv" => {
                        grubenv = args
                            .next()
                            .ok_or(CliError::MissingArgument("--grubenv value"))?
                            .into()
                    }
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }
            Command::Checkpoint {
                name,
                bootloader,
                esp,
                grub_cfg,
                grubenv,
            }
        }
        "install" => {
            let mut source = None;
            let mut integrations = Vec::new();
            let mut adapter_bins = Vec::new();
            let mut grub_entries = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--source" => source = args.next(),
                    "--integrations" => {
                        integrations = args
                            .next()
                            .ok_or(CliError::MissingArgument("--integrations value"))?
                            .split(',')
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect()
                    }
                    "--adapter-bin" => {
                        let value = args
                            .next()
                            .ok_or(CliError::MissingArgument("--adapter-bin value"))?;
                        let (name, path) = value
                            .split_once('=')
                            .ok_or(CliError::MissingArgument("--adapter-bin value (name=path)"))?;
                        adapter_bins.push((name.to_string(), path.into()));
                    }
                    "--grub-entries" => {
                        grub_entries = Some(
                            args.next()
                                .ok_or(CliError::MissingArgument("--grub-entries value"))?
                                .into(),
                        )
                    }
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }
            Command::Install {
                source: source.ok_or(CliError::MissingArgument("--source"))?.into(),
                integrations,
                adapter_bins,
                grub_entries,
            }
        }
        "apply-now" => {
            let mut live_root = PathBuf::from("/");
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--live-root" => {
                        live_root = args
                            .next()
                            .ok_or(CliError::MissingArgument("--live-root value"))?
                            .into()
                    }
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }
            Command::ApplyNow { live_root }
        }
        "doctor" => Command::Doctor,
        other => return Err(CliError::UnknownCommand(other.to_string())),
    };

    Ok(Invocation { store, command })
}
