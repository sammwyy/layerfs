# LayerFS and minid

LayerFS remains the initramfs root provider. It discovers and assembles the
LayerFS root, records the generic boot mount claims at
`/etc/diskd/boot-mounts.toml`, then execs minid without replacing PID 1.

Install minid as `/sbin/minid` in the assembled root and pass:

```text
rdinit=/sbin/layerfs-init layerfs.init=/sbin/minid
```

The post-root minid configuration starts `busd`, `devd`, `diskd --system`, and
`serviced`. `diskd` consumes the claim file as a generic ownership boundary;
it does not need to know which root provider wrote it.
