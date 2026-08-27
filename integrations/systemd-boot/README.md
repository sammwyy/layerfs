# systemd-boot integration

Generate LayerFS BLS entries into the ESP's `loader/entries` directory:

```bash
layerfs-systemd-boot --boot-store /boot/layerfs --esp-prefix /layerfs --store /dev/vda \
  --entries-dir /boot/loader/entries --integrations dnf,apt \
  --rdinit /sbin/layerfs-init
```

The command emits Normal, Safe, System, Previous Update, and Base Recovery
entries. Each resolves its kernel and initramfs from the corresponding
registered LayerFS boot-artifact generation, falling back to the next lower
tier when necessary. `--esp-prefix` is the boot store's path from the ESP,
which systemd-boot uses instead of the host filesystem path.
