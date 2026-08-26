# dracut integration

dracut module that installs `layerfs-init` into the initramfs and hooks it
in before the real root switch. Not implemented yet — see ROADMAP.md section 9.

Expected layout once implemented:

```
integrations/dracut/
├── module-setup.sh
└── layerfs-init.sh
```
