# LayerFS Roadmap

**Transactional, layered, recoverable root filesystem architecture for Linux**

Implementation language: Rust
Primary platform: Linux
Initial reference target: Fedora + Btrfs + GRUB + dracut
License: MIT

This document is both the design reference and the implementation roadmap.
Sections 1-40 describe the target architecture; section 41 tracks progress
against it milestone by milestone. Where the code has taken a different path
than a section below describes, the code is authoritative — update the
section rather than trusting it blindly.

Status legend: `[x]` done, `[~]` skeleton/partial, `[ ]` not started.

---

## 1. Overview

LayerFS is a Linux system architecture designed around a simple principle:

> The user should be allowed to modify and break the operating system normally, while always retaining a known-good system underneath it.

LayerFS is **not an immutable Linux distribution**.

The user retains normal root access and can perform operations such as:

```bash
sudo nano /etc/pam.d/login
sudo rm /usr/lib/libfoo.so
sudo cp ./custom-bash /usr/bin/bash
sudo chmod 000 /etc/fstab
```

These commands behave normally and require no LayerFS-specific syntax.

LayerFS transparently redirects system mutations into layered storage so they can be ignored, inspected, reset, or recovered from later.

The intended user experience is indistinguishable from a normal Linux system during everyday use.

LayerFS becomes visible primarily when:

* booting into a recovery checkpoint;
* performing system package transactions;
* inspecting modifications;
* reverting a broken update;
* resetting user overrides;
* repairing the operating system.

LayerFS should be implemented as an independent project which can be installed onto existing Linux distributions.

A future Linux distribution can use LayerFS as one of its core architectural components without coupling LayerFS itself to that distribution.

---

# 2. Project Goals

LayerFS must provide the following properties.

## 2.1 Transparent system modification

Normal filesystem operations must remain normal.

```bash
sudo nano /etc/foo.conf
sudo cp program /usr/bin/program
sudo rm /usr/lib/library.so
```

There should be:

* no confirmation prompt;
* no LayerFS warning;
* no special command;
* no immutable filesystem error;
* no package sandbox;
* no mandatory container.

The user sees a normal writable `/`.

---

## 2.2 Recoverability

The user must always have multiple bootable views of the system.

The core LayerFS checkpoints are:

```text
0 / base
1 / system
2 / safe
3 / normal
```

These checkpoints are hardcoded into LayerFS and form part of its stable semantics.

They are not user-defined configuration.

---

## 2.3 One-update rollback

LayerFS does not maintain arbitrary historical snapshots.

Instead it keeps two update layers:

```text
UPDATE
UPDATE_HEAD
```

`UPDATE` represents the cumulative system update checkpoint.

`UPDATE_HEAD` represents the most recent system transaction.

This allows exactly one system-update rollback without keeping an ever-growing snapshot history.

---

## 2.4 Disposable derived state

The system should conceptually distinguish between:

```text
BASE          canonical
UPDATE        derived
UPDATE_HEAD   derived
OVERRIDE      user-created
DATA          irreplaceable/persistent
```

`UPDATE` and `UPDATE_HEAD` must be reconstructible.

`OVERRIDE` must be inspectable and removable.

`DATA` must never depend on the root system remaining bootable.

---

## 2.5 Distribution independence

LayerFS must not intrinsically depend on:

* Fedora;
* Ubuntu;
* Arch;
* DNF;
* APT;
* Pacman;
* systemd;
* GRUB;
* Btrfs.

However, the initial implementation should intentionally target a constrained environment:

```text
Fedora
Btrfs
OverlayFS
GRUB
dracut
DNF
x86_64
```

Additional integrations can then be added independently.

---

# 3. Non-Goals

LayerFS v1 is not:

* a new Linux kernel filesystem;
* a replacement for OverlayFS;
* a replacement for Btrfs;
* a package manager;
* a backup system;
* a container runtime;
* an immutable distribution;
* a security boundary against root;
* a replacement for Secure Boot;
* a general filesystem version-control system;
* an infinite snapshot manager.

A sufficiently privileged root user can deliberately bypass LayerFS by directly mounting or modifying the backing filesystem.

LayerFS protects primarily against accidental or logical system destruction, not a hostile root user.

---

# 4. Core Architecture

The conceptual root consists of:

```text
OVERRIDE
    ↓
UPDATE_HEAD
    ↓
UPDATE
    ↓
BASE
```

The highest matching filesystem object wins.

This maps directly onto OverlayFS semantics. OverlayFS supports one writable upper layer, multiple lower layers, copy-up, whiteouts, and opaque directories.

For normal operation:

```text
upperdir = override

lowerdirs =
    update-head
    update
    base
```

Conceptually:

```text
/
│
├── OVERRIDE       RW
├── UPDATE_HEAD    RO
├── UPDATE         RO
└── BASE           RO
```

Persistent user data is mounted separately.

---

# 5. Layer Definitions

## 5.1 BASE

`BASE` contains the original known-good operating system.

For a fresh LayerFS distribution, this would be the system shipped by the installation media.

For an existing distribution converted to LayerFS, it is the state of the system when LayerFS is installed.

Properties:

```text
mutable:       no
bootable:      yes
rebuildable:   installation-media dependent
user visible:  indirectly
purpose:       final recovery environment
```

BASE must never be modified by normal LayerFS operations.

It should remain usable even if:

* UPDATE is corrupted;
* UPDATE_HEAD is corrupted;
* OVERRIDE is unusable;
* the package manager breaks;
* user configuration prevents boot.

---

# 5.2 UPDATE

`UPDATE` contains the accumulated changes made by previous system transactions.

It represents the currently consolidated update checkpoint.

Example:

```text
BASE
Fedora installation

UPDATE
all accumulated DNF changes until transaction N
```

UPDATE contains only the filesystem differences necessary to transform BASE into the consolidated system state.

It is read-only during normal operation.

---

# 5.3 UPDATE_HEAD

`UPDATE_HEAD` contains the newest system transaction.

For example:

```text
UPDATE
system state through August 20

UPDATE_HEAD
DNF transaction from August 26
```

The current system becomes:

```text
UPDATE_HEAD > UPDATE > BASE
```

If the latest update is broken, UPDATE_HEAD can be ignored:

```text
UPDATE > BASE
```

This produces the previous system state.

UPDATE_HEAD is therefore effectively the rollback slot.

---

# 5.4 OVERRIDE

OVERRIDE is the normal writable system layer.

Any arbitrary root filesystem modification not performed inside a LayerFS system transaction is written here.

Examples:

```bash
sudo nano /etc/ssh/sshd_config
sudo rm /usr/bin/python
sudo cp ./libfoo.so /usr/lib/libfoo.so
sudo mkdir /opt/custom
```

The applications performing those operations do not know LayerFS exists.

OverlayFS copy-up handles modifications of lower-layer objects automatically. Deleted lower objects are represented using whiteouts.

OVERRIDE is never automatically consolidated into UPDATE.

It belongs to the administrator.

---

# 5.5 DATA

DATA represents persistent user and application data.

It is not another OverlayFS lower layer.

Instead, DATA is a collection of persistent mounts inserted into the assembled root.

Typical candidates include:

```text
/home
/root
/srv
```

Additional persistent paths may eventually include selected application state underneath `/var`.

Global `/var/lib` persistence should **not** initially be enabled automatically.

Package managers keep important system databases under locations such as:

```text
/var/lib/rpm
/var/lib/dpkg
/var/lib/apt
```

Persisting all of `/var/lib` independently of the system layers would make filesystem rollback inconsistent with package-manager state.

LayerFS should therefore distinguish:

```text
system state
persistent application state
user data
```

rather than blindly treating `/var` as DATA.

---

# 6. Checkpoints

Checkpoints are hardcoded LayerFS boot modes.

They are not snapshots.

A checkpoint is a predefined composition of LayerFS components.

The stable mapping is:

```text
0 = base
1 = system
2 = safe
3 = normal
```

Both numeric and string identifiers must be accepted.

```text
0       base
1       system
2       safe
3       normal
```

---

## 6.1 Checkpoint 0 — Base

```text
BASE
```

No UPDATE.

No UPDATE_HEAD.

No OVERRIDE.

No normal DATA mounts.

Purpose:

* factory recovery;
* repairing LayerFS itself;
* inspecting damaged layers;
* recovering from completely broken updates.

Kernel command line:

```text
layerfs.checkpoint=0
```

or:

```text
layerfs.checkpoint=base
```

---

## 6.2 Checkpoint 1 — System

```text
UPDATE_HEAD
    ↓
UPDATE
    ↓
BASE
```

No OVERRIDE.

No user DATA.

Purpose:

> Does the current updated operating system itself boot correctly?

This isolates system failures from both administrator changes and persistent user state.

---

## 6.3 Checkpoint 2 — Safe

System root:

```text
UPDATE_HEAD
    ↓
UPDATE
    ↓
BASE
```

plus:

```text
DATA
```

No OVERRIDE.

Purpose:

> Boot my real machine and my data, but ignore all arbitrary modifications I made to the operating system.

---

## 6.4 Checkpoint 3 — Normal

```text
OVERRIDE
    ↓
UPDATE_HEAD
    ↓
UPDATE
    ↓
BASE
```

plus DATA.

This is the normal operating environment.

---

# 7. Update Rollback

Update rollback is deliberately orthogonal to the four checkpoints.

The checkpoint determines how much machine/user state is included.

The update selector determines whether UPDATE_HEAD participates.

Default:

```text
layerfs.head=on
```

Rollback:

```text
layerfs.head=off
```

For example, the GRUB entry:

```text
Previous System Update
```

could internally use:

```text
layerfs.checkpoint=safe layerfs.head=off
```

resulting in:

```text
UPDATE
    ↓
BASE

+ DATA
```

This preserves exactly four checkpoint definitions while still allowing rollback of the latest system transaction.

---

# 8. GRUB Integration

GRUB should expose several generated entries.

Example:

```text
Fedora Linux
Fedora Linux — Safe Mode
Fedora Linux — System Only
Fedora Linux — Previous Update
Fedora Linux — Base Recovery
```

Internally:

```text
Normal
layerfs.checkpoint=normal

Safe
layerfs.checkpoint=safe

System
layerfs.checkpoint=system

Previous Update
layerfs.checkpoint=safe layerfs.head=off

Base
layerfs.checkpoint=base
```

GRUB supports independent menu entries and arbitrary kernel command-line parameters, making this model straightforward to integrate.

The user should additionally be able to edit the kernel command line manually from GRUB.

---

# 9. Boot Architecture

LayerFS performs root assembly from the initramfs.

Boot sequence:

```text
UEFI / BIOS
      ↓
GRUB
      ↓
Linux kernel
      ↓
initramfs
      ↓
layerfs-init
      ↓
read /proc/cmdline
      ↓
discover LayerFS storage
      ↓
validate metadata
      ↓
resolve checkpoint
      ↓
mount backing layers
      ↓
assemble OverlayFS
      ↓
mount DATA
      ↓
switch_root
      ↓
real PID 1
```

For Fedora, LayerFS should initially integrate through a custom dracut module.

dracut explicitly supports third-party modules and boot-stage hooks, making it possible to insert LayerFS assembly before switching to the real root.

Eventually integrations can be provided for:

```text
dracut
mkinitcpio
initramfs-tools
```

The LayerFS core itself must not depend on any one initramfs generator.

---

# 10. Kernel Command-Line Interface

LayerFS boot parameters:

```text
layerfs.checkpoint=<name|number>
layerfs.head=<on|off>
layerfs.debug=<0|1>
layerfs.store=<device-spec>
```

Examples:

```text
layerfs.checkpoint=normal
layerfs.checkpoint=3

layerfs.checkpoint=safe
layerfs.checkpoint=2

layerfs.checkpoint=base

layerfs.checkpoint=safe layerfs.head=off
```

Invalid values must fail safely.

An unknown checkpoint should never silently boot NORMAL.

Recommended fallback:

```text
invalid configuration
        ↓
BASE checkpoint
        ↓
display diagnostic
```

---

# 11. System Transactions

The most important distinction in LayerFS is:

```text
normal mutation
vs
system transaction
```

Normal mutation:

```text
writes → OVERRIDE
```

System transaction:

```text
writes → staged UPDATE_HEAD
```

A system transaction is an explicit execution context created by LayerFS.

It should not be detected by observing process names.

Incorrect:

```text
if process_name == "dnf":
    ...
```

Correct conceptual interface:

```text
layerfs transaction begin system
        ↓
create private mount namespace
        ↓
assemble transaction root
        ↓
execute package manager
        ↓
validate
        ↓
commit
```

---

# 12. Package Manager Integration

LayerFS itself does not implement package management.

Instead it provides adapters.

Initial adapters:

```text
layerfs-dnf
layerfs-apt
layerfs-pacman
```

Potential future adapters:

```text
layerfs-zypper
layerfs-xbps
layerfs-apk
```

From the user's perspective:

```bash
sudo dnf upgrade
```

continues to be the interface.

The Fedora integration transparently executes mutating DNF operations inside a LayerFS system transaction.

---

## 12.1 Important package database rule

A package manager's mutating operations should normally all use the same LayerFS system state.

For example:

```bash
dnf install
dnf remove
dnf upgrade
dnf distro-sync
```

should normally all be system transactions.

Routing `dnf install` to OVERRIDE while routing `dnf upgrade` to UPDATE_HEAD would create package-database consistency problems because databases such as RPM's are themselves filesystem objects.

Therefore the initial rule should be:

> Once a package manager is LayerFS-managed, every operation that mutates its package database is a system transaction.

Read-only commands remain ordinary:

```bash
dnf search
dnf info
dnf list
```

---

# 13. System Transaction Root

Suppose:

```text
BASE
UPDATE
UPDATE_HEAD
OVERRIDE
```

are active.

A normal process sees:

```text
OVERRIDE > UPDATE_HEAD > UPDATE > BASE
```

A package update must **not** see OVERRIDE.

Instead LayerFS creates a private mount namespace containing:

```text
STAGING_HEAD
      ↓
CONSOLIDATED_UPDATE_NEXT
      ↓
BASE
```

The package manager believes it is operating on a normal root filesystem.

Its writes are captured by `STAGING_HEAD`.

Relevant virtual filesystems are mounted/bound into the transaction environment:

```text
/proc
/sys
/dev
/run
```

Network connectivity remains shared unless explicitly isolated.

No container semantics are required.

This is filesystem transaction isolation, not application sandboxing.

---

# 14. UPDATE / UPDATE_HEAD Lifecycle

Suppose the current state is:

```text
UPDATE      = system through transaction 41
UPDATE_HEAD = transaction 42
```

A new system transaction must create transaction 43.

LayerFS must not destructively merge the active layers first.

Instead:

```text
UPDATE
UPDATE_HEAD
    │
    │ prepare
    ▼
UPDATE.next
HEAD.next
```

Process:

```text
1. Acquire exclusive system transaction lock.

2. Clone UPDATE → UPDATE.next.

3. Squash current UPDATE_HEAD into UPDATE.next.

4. Create empty writable HEAD.next.

5. Assemble:
       HEAD.next > UPDATE.next > BASE

6. Execute package manager inside that root.

7. Validate resulting filesystem.

8. Freeze UPDATE.next.

9. Freeze HEAD.next.

10. Atomically commit both as active state.

11. Release transaction lock.

12. Garbage-collect old UPDATE and UPDATE_HEAD.
```

If anything fails before step 10:

```text
active UPDATE      unchanged
active UPDATE_HEAD unchanged
```

The failed staging state can simply be deleted.

---

# 15. Layer Squashing

Squashing is a first-class LayerFS operation.

Conceptually:

```text
squash(A, B) → C
```

such that:

```text
C > BASE
```

produces the same view as:

```text
B > A > BASE
```

This cannot be implemented as:

```bash
cp -a B/* A/
```

because OverlayFS layers may contain:

* whiteouts;
* opaque directories;
* symlinks;
* hardlinks;
* xattrs;
* POSIX ACLs;
* file capabilities;
* device nodes;
* ownership metadata.

OverlayFS uses whiteouts and opaque-directory metadata to represent deletions from lower filesystems.

LayerFS therefore needs its own deterministic layer merge implementation.

This should eventually live in:

```text
layerfs-core::squash
```

and operate directly on stored layer trees.

---

# 16. Storage Backends

LayerFS should expose an internal storage abstraction.

```rust
trait StorageBackend {
    fn prepare_layer(...);
    fn clone_layer(...);
    fn freeze_layer(...);
    fn delete_layer(...);
    fn activate_state(...);
    fn verify_layer(...);
}
```

Initial implementations:

```text
BtrfsBackend
DirectoryBackend
```

---

# 17. Btrfs Backend

Btrfs should be the first-class MVP backend.

Reasons:

* subvolumes;
* CoW;
* cheap writable snapshots;
* read-only snapshots;
* efficient staging;
* atomic-ish filesystem operations;
* existing Fedora deployment.

Btrfs snapshots share extents through copy-on-write and can be created read-only, which is useful for staging and freezing LayerFS state.

Example:

```text
Btrfs top-level
│
└── layerfs
    ├── base
    ├── update
    ├── update-head
    ├── override
    ├── data
    ├── staging
    └── state
```

These should preferably be Btrfs subvolumes.

Example internal IDs:

```text
@layerfs-base
@layerfs-update
@layerfs-head
@layerfs-override
@layerfs-data
```

BASE, UPDATE, and active UPDATE_HEAD are read-only.

OVERRIDE remains writable.

During transactions:

```text
@layerfs-update-next    RW
@layerfs-head-next      RW
```

are created.

After successful validation they become read-only and are activated.

---

# 18. Generic Directory Backend

LayerFS should eventually work without Btrfs.

For example, on ext4:

```text
.layerfs-store/
├── base/
├── update/
├── update-head/
├── override/
├── data/
├── staging/
└── state/
```

OverlayFS only requires that its upper filesystem support the required metadata/xattr behaviour; its lower layers may be read-only.

The generic backend will be less efficient because cloning and squashing may require filesystem-level copying.

That is acceptable.

LayerFS correctness must not depend on Btrfs-specific behaviour.

---

# 19. Backing Store Visibility

The backing store should not normally appear inside the final root.

During boot:

```text
/run/layerfs-store
```

may temporarily expose the backing filesystem.

After assembling the final OverlayFS, the direct backing-store mount should be detached from the final mount namespace.

Users should normally see:

```text
/
```

not:

```text
/.layerfs/base
/.layerfs/update
```

This prevents accidental mutation of lower layers.

Root can deliberately recover or expose them through `layerctl`.

---

# 20. Rust Technology Stack

LayerFS should use Rust stable with the current Rust edition.

Recommended baseline:

```text
Language:       Rust
Edition:        2024
Async runtime:  none
Primary syscall library: rustix
Serialization:  serde
Error handling: custom errors / thiserror where useful
CLI parsing:    lexopt or minimal custom parser
Testing:        cargo test + QEMU integration tests
Build system:   Cargo workspace
```

`rustix` provides low-level safe wrappers around POSIX/Linux syscall-style APIs and is a good fit for mounts, file descriptors, namespaces and other Linux-specific operations without writing most system interaction directly through raw `libc`.

Use raw `libc` only where required for unsupported ioctls or kernel APIs.

---

# 21. Dependency Philosophy

Early-boot code should have very few dependencies.

Avoid:

```text
tokio
async-std
large CLI frameworks
DBus
HTTP stacks
OpenSSL
heavy logging frameworks
```

`layerfs-init` should ideally depend only on:

```text
layerfs-core
rustix
small parsing utilities
```

It should be possible to statically link the initramfs binary using musl.

Example target:

```text
x86_64-unknown-linux-musl
```

The normal `layerctl` host binary can be less restrictive.

---

# 22. Rust Workspace

Recommended initial repository:

```text
layerfs/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
│
├── crates/
│   ├── layerfs-core/
│   │   └── src/
│   │
│   ├── layerfs-init/
│   │   └── src/
│   │
│   ├── layerctl/
│   │   └── src/
│   │
│   ├── layerfs-storage/
│   │   └── src/
│   │
│   └── layerfs-transaction/
│       └── src/
│
├── integrations/
│   ├── dracut/
│   ├── grub/
│   ├── dnf/
│   ├── apt/
│   └── pacman/
│
├── tests/
│   ├── integration/
│   └── qemu/
│
├── scripts/
│
└── xtask/
    └── src/
```

Do not split every small abstraction into a crate.

The initial workspace should remain relatively compact.

---

# 23. `layerfs-core`

Pure LayerFS semantics.

Responsibilities:

```text
Checkpoint
Layer
LayerStack
BootOptions
State
UpdateState
squash()
validation
path classification
metadata parsing
```

Important types:

```rust
#[repr(u8)]
pub enum Checkpoint {
    Base = 0,
    System = 1,
    Safe = 2,
    Normal = 3,
}
```

Parser:

```text
"0" | "base"   → Base
"1" | "system" → System
"2" | "safe"   → Safe
"3" | "normal" → Normal
```

Checkpoint composition should be implemented directly in code.

No TOML configuration should define checkpoint behaviour.

---

# 24. `layerfs-init`

Small early-userspace binary.

Responsibilities:

```text
parse /proc/cmdline
locate root device
locate LayerFS metadata
validate active state
select checkpoint
mount layers
assemble OverlayFS
mount DATA
switch_root
emit recovery diagnostics
```

It should not:

* update packages;
* squash layers;
* perform network access;
* parse complex configuration;
* run a daemon.

Boot correctness is more important than convenience.

---

# 25. `layerctl`

Administrative CLI.

Initial commands:

```text
layerctl status
layerctl inspect
layerctl diff
layerctl reset
layerctl verify
layerctl rollback
layerctl rebuild
layerctl checkpoint
layerctl install
layerctl doctor
```

Examples:

```bash
layerctl status

layerctl diff override
layerctl diff update-head

layerctl reset /etc/pam.d/login
layerctl reset /usr/bin/bash

layerctl rollback update

layerctl verify

layerctl checkpoint safe
```

`layerctl checkpoint safe` could configure the next boot rather than changing the currently mounted root.

---

# 26. State Metadata

LayerFS requires a very small durable metadata store.

Example conceptual state:

```json
{
  "version": 1,
  "base": "base",
  "update": "update-42",
  "update_head": "head-43",
  "override": "override",
  "transaction": null
}
```

Metadata commits must survive abrupt power loss.

Use:

```text
write temporary file
fsync(file)
rename(temp, state)
fsync(parent directory)
```

Optionally retain:

```text
state.json
state.previous.json
```

At boot, LayerFS should reject malformed state rather than guessing.

---

# 27. Transaction Locking

Only one system transaction may exist at a time.

Example:

```text
/run/layerfs/transaction.lock
```

Locking should use kernel-backed advisory locking rather than checking for existence of a PID file.

A transaction record should include:

```text
transaction ID
type
started timestamp
active staging layers
package-manager adapter
state
```

Possible states:

```text
PREPARING
RUNNING
VALIDATING
COMMITTING
COMMITTED
FAILED
```

---

# 28. Power-Failure Safety

Power loss must be treated as a normal failure case.

An interrupted transaction before COMMIT must leave the previous system bootable.

Never mutate the active UPDATE layer directly.

Never mutate the active UPDATE_HEAD directly.

Only staging objects may be writable.

State transition:

```text
ACTIVE
  update=A
  head=B

        ↓ build

STAGING
  update.next=C
  head.next=D

        ↓ atomic metadata commit

ACTIVE
  update=C
  head=D
```

A crash before metadata commit leaves:

```text
A + B
```

active.

Staging objects can be garbage-collected during the next boot.

---

# 29. Validation

Before committing a system transaction, LayerFS should perform basic structural validation.

MVP checks:

```text
/usr exists
/etc exists
/bin or /usr/bin exists
configured init binary exists
ELF interpreter exists
critical mount layout valid
LayerFS metadata valid
```

Later distro adapters can provide additional checks.

Fedora example:

```text
rpm database readable
kernel/initramfs generated
DNF transaction successful
```

Validation must not attempt to prove that the OS will boot.

Rollback exists precisely because some failures can only be discovered during boot.

---

# 30. Boot Artifact Management

Root rollback is useless if the only installed kernel/initramfs is broken.

LayerFS therefore also needs to treat boot artifacts transactionally.

Conceptually maintain:

```text
BOOT_BASE
BOOT_UPDATE
BOOT_HEAD
```

At minimum retain:

```text
base kernel + initramfs
previous consolidated kernel + initramfs
current kernel + initramfs
```

GRUB entries must select boot artifacts compatible with the requested system state.

Longer term, Unified Kernel Images may simplify this model, but LayerFS v1 should support normal Fedora/GRUB layouts.

---

# 31. Retrofit Installation

LayerFS should eventually install onto an existing Linux system.

User interface:

```bash
sudo layerctl install
```

The current running root should not be converted destructively in-place.

Preferred process:

```text
running OS
    ↓
layerctl install
    ↓
inspect filesystem
    ↓
install boot/initramfs integration
    ↓
schedule migration
    ↓
reboot
    ↓
LayerFS migration initramfs
    ↓
create BASE
create UPDATE
create UPDATE_HEAD
create OVERRIDE
extract DATA
    ↓
validate
    ↓
install LayerFS boot entries
    ↓
reboot normal
```

This avoids racing against modifications occurring on the live root filesystem.

---

# 32. Initial Fedora Strategy

Fedora is the ideal first target because it provides a useful combination of:

```text
Btrfs
dracut
GRUB
DNF
modern kernel
```

Initial supported configuration should intentionally be narrow.

Example:

```text
Fedora 4x
x86_64
Btrfs root
UEFI
GRUB
dracut
DNF
```

Do not attempt Ubuntu, Arch, ext4, systemd-boot and encrypted/LVM configurations simultaneously.

Prove the architecture first.

---

# 33. Existing Fedora Migration

Assume:

```text
@root
@home
```

The offline migration can conceptually create:

```text
@layerfs-base
@layerfs-update
@layerfs-head
@layerfs-override
@layerfs-data
```

`@layerfs-base` becomes a read-only snapshot of the original system.

Btrfs supports snapshots and read-only subvolumes, making this migration relatively inexpensive compared with copying the complete root.

---

# 34. Repair Environment

BASE should expose LayerFS repair utilities.

Example:

```console
LayerFS Base Recovery

# layerctl status
# layerctl inspect override
# layerctl verify
# layerctl reset /etc
# layerctl rollback update
```

The broken layers can be mounted separately for inspection.

Example:

```text
/run/layerfs/repair/
├── update
├── update-head
├── override
└── data
```

This makes recovery possible entirely from inside the installed machine.

A Live USB should not normally be required.

---

# 35. Reset Semantics

Resetting an overridden path means removing the corresponding OVERRIDE representation.

Example:

```bash
sudo rm /usr/bin/python
```

OverlayFS creates a whiteout.

Then:

```bash
layerctl reset /usr/bin/python
```

removes that override/whiteout.

The lower version immediately becomes visible again.

Likewise:

```bash
layerctl reset /etc/ssh/sshd_config
```

restores the version supplied by:

```text
UPDATE_HEAD
UPDATE
or BASE
```

depending on where the effective file originates.

---

# 36. Update Rebuild

UPDATE and UPDATE_HEAD are derived state.

LayerFS should support:

```bash
layerctl rebuild updates
```

Conceptually:

```text
discard UPDATE
discard UPDATE_HEAD
        ↓
start from BASE
        ↓
replay/reinstall required package state
        ↓
construct new UPDATE
```

Exact reconstruction requires package-version availability.

LayerFS should therefore preserve a package manifest where supported.

Example:

```text
kernel-core = ...
systemd = ...
glibc = ...
coreutils = ...
```

For LayerFS's own future distribution, repositories could retain historical snapshots to make exact reconstruction deterministic.

---

# 37. Security Model

BASE, UPDATE and UPDATE_HEAD should be mounted read-only during normal operation.

The backing store should not be exposed in the normal namespace.

Optional future integrity mechanisms:

```text
fs-verity
signed manifests
Secure Boot
signed update metadata
TPM measurements
```

These are intentionally outside the MVP.

LayerFS should never claim to defend against a malicious root user.

A root user with raw block-device access can ultimately bypass filesystem-level policy.

---

# 38. Logging

Early boot logs should be minimal and deterministic.

Example:

```text
layerfs: checkpoint=normal
layerfs: base=base
layerfs: update=update-42
layerfs: head=head-43
layerfs: mounting overlay
layerfs: mounting data
layerfs: switching root
```

When:

```text
layerfs.debug=1
```

additional mount and metadata details can be printed.

Avoid requiring journald inside the initramfs.

---

# 39. Test Strategy

LayerFS needs unusually aggressive destructive testing.

Unit tests are insufficient.

Testing should consist of:

```text
unit tests
filesystem integration tests
QEMU boot tests
power-loss tests
package-manager tests
migration tests
```

---

# 40. QEMU Test Harness

Create automated bootable test images.

Example scenarios:

### Test 1 — Normal override

```text
boot normal
modify /etc/example
reboot
verify modification persists
```

### Test 2 — Safe checkpoint

```text
modify /etc/example in normal
boot safe
verify modification disappears
boot normal
verify modification returns
```

### Test 3 — Destructive override

```text
delete critical binary
verify normal fails
boot safe
verify safe succeeds
```

### Test 4 — Broken UPDATE_HEAD

```text
perform system transaction
damage HEAD
boot normally → failure
boot with head=off
verify previous UPDATE boots
```

### Test 5 — BASE recovery

```text
damage UPDATE
damage HEAD
damage OVERRIDE
boot base
verify repair environment works
```

### Test 6 — Power loss

Terminate QEMU at every stage of a system transaction.

After every simulated crash:

```text
one valid LayerFS system must remain bootable
```

---

# 41. MVP Development Plan

## Milestone 0 — Development environment

Progress:

- [x] Cargo workspace (`crates/`, `xtask/`, `tests/integration/`)
- [~] Fedora QEMU image — no installed Fedora disk image yet, but `scripts/qemu-smoke.sh` boots the real host's kernel (Fedora 42, 6.19.x) under QEMU/KVM against a throwaway initramfs; full installroot-based image is still open
- [x] automated kernel/initramfs boot — `scripts/qemu-smoke.sh` boots a real kernel with `layerfs-init`'s mount logic as `rdinit=/init`, loads `overlay.ko` by hand (no dracut yet), and powers off via `reboot(2)`, all unattended
- [x] basic integration-test harness — cross-crate `cargo test` crate, an unprivileged-namespace OverlayFS example, and now a real QEMU boot harness

Build:

```text
Rust workspace
Fedora QEMU image
automated kernel/initramfs boot
basic integration-test harness
```

No LayerFS functionality yet.

---

## Milestone 1 — Manual OverlayFS root

Progress:

- [x] BASE mount (bind-mount path in `layerfs-init::mount::assemble` for a single read-only layer)
- [x] OVERRIDE mount (OverlayFS upperdir/workdir path in `layerfs-init::mount::assemble`)
- [x] OverlayFS assembly (`layerfs-init::mount::assemble`, real `mount(2)` via rustix)
- [x] copy-up / deletions (whiteouts) / remount persistence — verified against a real kernel OverlayFS mount in an unprivileged user+mount namespace via `cargo run -p layerfs-init --example overlay_smoke`
- [x] verified against a real kernel boot, not just a namespace — `scripts/qemu-smoke.sh` boots the host kernel under QEMU with `layerfs-init`'s actual `resolve_stack`/`assemble`/`mount_data` running as PID 1

Implement:

```text
BASE
OVERRIDE
```

Boot:

```text
OVERRIDE > BASE
```

Verify:

```text
copy-up
deletions
whiteouts
reboots
```

---

## Milestone 2 — Four checkpoints

Progress:

- [x] `Checkpoint` enum with numeric/name parsing (`layerfs-core::Checkpoint`)
- [x] `layerfs.checkpoint` / `layerfs.head` / `layerfs.debug` / `layerfs.store` cmdline parsing (`layerfs-core::BootOptions`)
- [x] fail-safe rejection of invalid checkpoint values (no silent NORMAL fallback)
- [x] directory-backend layer discovery (`layerfs-storage::discover`) and checkpoint→stack resolution (`layerfs-init::mount::resolve_stack`), including `head=off` dropping UPDATE_HEAD
- [x] DATA bind-mounted alongside the overlay for `safe`/`normal` (`layerfs-init::mount::mount_data`, `layerfs_core::DATA_MOUNTS`); verified end to end (existing content visible, writes land in the backing store, correct nested unmount order) via `overlay_smoke`
- [ ] wired into `main.rs`'s actual boot path end to end (currently reachable only via explicit `layerfs.store=`, not real storage/BASE discovery on a booted machine)
- [ ] `switch_root` (Milestone 8/boot-artifact territory — deliberately out of scope here)

Add:

```text
UPDATE
UPDATE_HEAD
DATA
```

Implement:

```text
layerfs.checkpoint=0
layerfs.checkpoint=1
layerfs.checkpoint=2
layerfs.checkpoint=3
```

and aliases:

```text
base
system
safe
normal
```

---

## Milestone 3 — GRUB integration

Progress:

- [x] generates the five hardcoded checkpoint entries (Normal, Safe Mode, System Only, Previous Update, Base Recovery) as GRUB configuration syntax (`integrations/grub`, binary `layerfs-grub-entries`), installable directly as an executable `/etc/grub.d/` script
- [x] verified for real: `scripts/qemu-grub-smoke.sh` runs `grub2-script-check` on the generated `grub.cfg`, builds an actual bootable ISO with `grub2-mkrescue`, and boots it under QEMU/KVM — GRUB itself renders the menu, selects an entry, and chainloads the kernel + `layerfs-init` initramfs; checked for both the Normal and Base Recovery entries to prove GRUB is passing distinct `layerfs.checkpoint=` values through correctly, not just parsing without error
- [ ] not installed into an actual `/etc/grub.d/` (Milestone 9, retrofit installation)
- [ ] `--linux`/`--initrd` are passed through verbatim rather than discovered from tracked boot artifacts (Milestone 8)

Generate checkpoint entries.

Support:

```text
Normal
Safe
System
Base
Previous Update
```

---

## Milestone 4 — `layerctl`

Progress:

- [x] `status` — lists discovered layer paths for a `--store <path>` (`crates/layerctl/src/store.rs`, `commands.rs`)
- [x] `inspect <layer>` — walks a layer's raw tree, tagging OverlayFS whiteouts (`crates/layerctl/src/walk.rs`)
- [x] `diff <layer>` — classifies each entry as added/modified/removed against the next layer down
- [x] `reset <path>` — removes a path's override representation (file, whiteout, or directory), restoring the lower layer's version
- [x] `verify` — MVP structural checks against BASE (`/usr`, `/etc`, `/bin` or `/usr/bin`) (`crates/layerctl/src/verify.rs`)
- [ ] `rollback` / `rebuild` / `checkpoint` / `install` / `doctor` — depend on the transaction engine and package-manager adapters, still stubs

Verified against a real store layout (including an actual unprivileged OverlayFS whiteout device) built in a scratch directory: `status`, `inspect override`, `diff override`, `verify`, and `reset` (plus its correct failure on a second reset) all behaved as expected, and `layerctl status` against a nonexistent default store fails safely without creating anything.

Implement:

```text
status
inspect
diff
reset
verify
```

At this stage LayerFS already becomes useful for manual experimentation.

---

## Milestone 5 — Transaction engine

Progress:

- [x] `flock`-based `TransactionLock` (`layerfs-transaction::lock`)
- [x] real `DirectoryBackend`: `prepare_layer`/`clone_layer` (recursive copy preserving symlinks and OverlayFS whiteouts, `layerfs-storage::copy_tree`), `freeze_layer`/`delete_layer` (permission-bit based; true immutability is the Btrfs backend's job), `verify_layer`
- [x] private mount namespace: `Transaction::stage` calls `unshare(CLONE_NEWNS)` before mounting, so a transaction never sees OVERRIDE and its mount doesn't leak into the parent namespace (section 13)
- [x] staging upper wired to a real `StorageBackend`: `HEAD.next > UPDATE.next > BASE` assembled via the same `layerfs_storage::overlay::assemble` boot uses (moved there from `layerfs-init` so both share one mount implementation)
- [x] atomic metadata commit: `layerfs-storage::generations` — `update`/`update-head` are symlinks into `generations/`, repointed via `symlink` + `rename` (a single atomic syscall per pointer), never by renaming the named path itself
- [x] failure cleanup: `Transaction`'s `Drop` deletes any staged (uncommitted) generations and unmounts the transaction root — verified for command failure (nonzero exit), validation failure, and the squash-required guard, all leaving the store byte-for-byte unchanged
- [x] `layerctl transaction -- <program> [args...]` dev command — chroots `program` into the assembled transaction root
- [x] verified for real (needs `CAP_SYS_ADMIN`/`CAP_SYS_CHROOT`, run under `unshare --map-root-user --mount`): bootstrap transaction commits correctly (symlinks point at frozen, read-only generations; BASE untouched); a second transaction against an active UPDATE_HEAD is refused with a clear message; a failing command and a failing validation both discard cleanly with no trace in `generations/`
- [x] squashing an active UPDATE_HEAD into UPDATE.next — see Milestone 6; verified with two chained real transactions (second transaction sees and preserves the first's content, correct GC of superseded generations)

Implement:

```text
private mount namespace
staging upper
transaction lock
atomic metadata commit
failure cleanup
```

Initially execute a trivial command rather than DNF.

Example:

```bash
layerctl transaction -- bash
```

for development only.

---

## Milestone 6 — Layer squash

Progress:

- [x] `squash()` implemented for real in `layerfs-storage::squash` (moved from `layerfs-core`, which would otherwise need a circular dependency on `layerfs-storage` for the same whiteout/xattr primitives `copy_tree` already uses)
- [x] whiteouts and opaque directories handled correctly and recursively: whiteouts/opaque markers from the upper layer are preserved in the squashed output (not resolved away), since the result still needs to hide whatever ends up mounted below it; matching directories merge recursively rather than one side winning wholesale
- [x] symlinks handled (via `copy_tree`)
- [ ] xattrs beyond the OverlayFS opaque marker, hardlinks (copied as independent files), capabilities — same tradeoff as the rest of `DirectoryBackend`
- [ ] a path changing type between the two layers (file→dir or dir→file) is resolved as "upper's type wins outright," not modeled as an implicit whiteout-then-create — real package managers essentially never do this
- [x] wired into `Transaction::stage`: an active UPDATE_HEAD is now squashed into UPDATE.next instead of blocking the transaction
- [x] verified for real: 15 unit tests (shadowing, whiteout persistence in both directions, recursive merge, type change) plus 2 more requiring genuine root for `trusted.*` xattrs (`#[ignore]`d — see below) — a fake-root user namespace cannot set `trusted.*` even on a tmpfs it mounted itself, confirmed empirically; the same getxattr/setxattr logic is separately exercised against `user.*` to cover the code path without needing root
- [x] end-to-end: two chained real `layerctl transaction` runs against an unprivileged namespace, second transaction's UPDATE.next containing the first transaction's squashed content, verified by actually mounting the resulting three-layer stack and reading it back
- [x] a real bug was found and fixed this way: GC after commit was deleting the (already-repointed) `update`/`update-head` symlink instead of the superseded generation it used to point to

Implement correct:

```text
UPDATE + UPDATE_HEAD → UPDATE.next
```

including:

```text
whiteouts
opaque directories
metadata
xattrs
hardlinks
symlinks
capabilities
```

This is one of the most critical correctness milestones.

---

## Milestone 7 — DNF adapter

Progress:

- [x] shared adapter runner (`crates/layerfs-adapter`, core, not distro-specific): given a name, default binary, and an `is_mutating` classifier, handles exec passthrough or a full stage → chrooted execute → validate → commit transaction, propagating the real exit code — reusable as-is by `apt`/`pacman` adapters, which only need to supply their own classification
- [x] `layerfs-dnf` (`integrations/dnf`) supplies only DNF's classification on top of that: explicit read-only-verb allowlist (`search`/`info`/`list`/...), everything else defaults to mutating, `--downloadonly` forces read-only regardless of verb — 7 unit tests, plus 2 for the adapter runner's env-var naming
- [x] control via `LAYERFS_<NAME>_BIN`/`LAYERFS_STORE` env vars only, so argv is passed through untouched and never collides with the wrapped binary's own flags
- [x] built as separate, optional crates: `integrations/dnf` (and future `integrations/apt`, `integrations/pacman`) are excluded from the workspace's `default-members`, so plain `cargo build` stays distro-agnostic; `layerfs-adapter` itself is a normal default member since it's core, not distro glue
- [x] verified for real (unprivileged namespace + a musl stand-in binary in place of dnf, since a full Fedora installroot is out of scope here): a mutating verb stages a transaction and the stand-in's write lands in the new UPDATE_HEAD; a read-only verb and `install --downloadonly` both create zero new generations
- [ ] not installed over a real `/usr/bin/dnf` (Milestone 9) or exercised against an actual `dnf` transaction against a real Fedora root — no realistic BASE fixture exists yet for that

Make Fedora package transactions use LayerFS.

Goal:

```bash
sudo dnf upgrade
```

without requiring the user to invoke LayerFS manually.

Verify rollback.

---

## Milestone 8 — Boot artifact transactions

Progress:

- [ ] not started

Capture:

```text
kernel
initramfs
GRUB-compatible boot artifacts
```

and make BASE/update rollback boot-complete.

---

## Milestone 9 — Retrofit installer

Progress:

- [ ] not started (`layerctl install` returns "not implemented")

Support converting an existing Fedora Btrfs installation.

```bash
sudo layerctl install
```

followed by an offline migration reboot.

---

## Milestone 10 — Additional platforms

Progress:

- [ ] not started

After Fedora/Btrfs is stable:

```text
Arch + mkinitcpio + Pacman
Ubuntu + initramfs-tools + APT
generic ext4 backend
systemd-boot
```

---

# 42. Suggested Initial Implementation Order

Do **not** begin with package-manager interception.

The first useful prototype should only prove this:

```text
Fedora root
    ↓
BASE
+
OVERRIDE
    ↓
OverlayFS /
    ↓
normal boot
```

Then prove:

```text
normal → sees override
safe   → ignores override
base   → ignores everything
```

Only after boot composition is reliable should UPDATE/UPDATE_HEAD transactions be implemented.

The hardest pieces are expected to be:

```text
1. correct initramfs root assembly
2. layer squashing
3. transaction crash safety
4. package-manager state consistency
5. transactional boot artifacts
6. migration from existing installations
```

Everything else should remain secondary until these work.

---

# 43. Final Architecture

The final conceptual system is:

```text
                         NORMAL ROOT

                      ┌──────────────┐
                      │   OVERRIDE   │ RW
                      ├──────────────┤
                      │ UPDATE_HEAD  │ RO
                      ├──────────────┤
                      │    UPDATE    │ RO
                      ├──────────────┤
                      │     BASE     │ RO
                      └──────────────┘
                              │
                              │
                       persistent mounts
                              │
                      ┌──────────────┐
                      │     DATA     │ RW
                      └──────────────┘
```

System mutation paths:

```text
ordinary root operation
        ↓
OVERRIDE
```

```text
package-manager mutation
        ↓
LayerFS system transaction
        ↓
UPDATE_HEAD
```

Update progression:

```text
BASE
  │
UPDATE
  │
UPDATE_HEAD
  │
new transaction
  ▼
squash old HEAD into UPDATE.next
  +
create new HEAD.next
  │
atomic commit
  ▼
new UPDATE + new UPDATE_HEAD
```

Boot views:

```text
BASE
    = factory recovery

SYSTEM
    = current system only

SAFE
    = current system + user data

NORMAL
    = current system + user data + overrides
```

Latest-update rollback:

```text
HEAD OFF
    = previous consolidated system
```

---

# 44. Design Philosophy

LayerFS should follow five architectural rules.

### Rule 1

**Do not prevent the administrator from modifying Linux.**

LayerFS exists to make modification recoverable, not forbidden.

### Rule 2

**Known-good state must never be modified in place.**

All potentially destructive state transitions happen in staging storage first.

### Rule 3

**Recovery semantics must remain small and predictable.**

Exactly four checkpoints:

```text
base
system
safe
normal
```

No arbitrary checkpoint graph.

### Rule 4

**System update history is bounded.**

LayerFS stores:

```text
consolidated update
+
latest update
```

not an unlimited snapshot history.

### Rule 5

**LayerFS should disappear during normal use.**

The best normal interaction with LayerFS is no interaction at all.

The user should simply run Linux.

When Linux breaks, LayerFS is there underneath it.
