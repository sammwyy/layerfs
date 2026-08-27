# systemd-boot integration

Generate LayerFS BLS entries into the ESP's `loader/entries` directory:

```bash
layerfs-systemd-boot --boot-store /path/to/store/boot --store /dev/vda \
  --entries-dir /boot/loader/entries --integrations dnf,apt \
  --rdinit /sbin/layerfs-init
```

The command emits Normal, Safe, System, Previous Update, and Base Recovery
entries. Each resolves its kernel and initramfs from the corresponding
registered LayerFS boot-artifact generation, falling back to the next lower
tier when necessary.
