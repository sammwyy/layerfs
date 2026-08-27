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
- [x] Fedora QEMU image — `scripts/qemu-fedora-installroot-smoke.sh` builds a real `dnf --installroot` Fedora 42 system (systemd, systemd-udev) as LayerFS's `base`, boots it under QEMU/KVM through `layerfs-init`'s actual root assembly and `switch_root`, and reaches a real login prompt: `basic.target`, `multi-user.target`, and `graphical.target` all reached, with `systemd-udevd`, `dbus-broker`, and `systemd-logind` running as real services, not a throwaway marker binary
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
- [x] `main.rs` discovers an already-mounted store by scanning `/proc/self/mountinfo` for a mount containing `base`; `layerfs.store=<path>` accepts an explicit mounted path, while `layerfs.store=/dev/...` mounts an unencrypted Btrfs store at `/run/layerfs-store`. `scripts/qemu-btrfs-store-smoke.sh` creates a loop-mounted Btrfs image in a privileged container, presents it to QEMU as `/dev/vda`, and verifies that the actual initramfs mounts, composes, and switches to it. `layerfs.store=UUID=<uuid>` / `LABEL=<label>` resolve to a real device path — but **not** via `/dev/disk/by-*` udev symlinks as first implemented and "verified for real" against this machine's already-booted state: `rdinit=` (how `layerfs-init` is actually invoked — a genuine kernel parameter that replaces the initramfs's own `/init` outright, not a dracut convention) means dracut's `/init` and therefore its udev never run at all during a real boot, so those symlinks never exist. Caught by actually booting a probe binary the same way (`rdinit=/probe`) and observing `/dev/disk` doesn't exist — the kind of check the original "real" verification skipped. Fixed with `layerfs-init/src/device_scan.rs`: reads each block device's Btrfs superblock directly (magic/fsid/label offsets confirmed against a real `mkfs.btrfs -L <label>` image's on-disk bytes with `xxd`, not from memory of the kernel struct), used as a fallback when the udev symlink isn't there. `PARTUUID=` has no such fallback (would need GPT/MBR parsing) and only resolves if something else already populated `/dev/disk` — documented as such rather than silently pretending to work. The whole resolve is retried for up to 30s (`DEVICE_WAIT_TIMEOUT`) since, per the same finding, nothing else waits for the device to appear either. Verified for real this time the way it has to be: `scripts/qemu-uuid-store-smoke.sh` boots a real kernel via `rdinit=` (no udev, no `/dev/disk` symlinks of any kind present) against a Btrfs image with a fixed UUID and `layerfs.store=UUID=<that uuid>`, and reaches `QEMU-SWITCH-ROOT: PASS` — plus a privileged-container test resolving a real loop device by both its real UUID and real label. `layerfs.subvol=<name>` mounts a specific Btrfs subvolume instead of the device's default one, matching section 33's migrated layout where LayerFS lives alongside other subvolumes (`@root`, `@home`, ...) rather than owning the whole filesystem — verified for real (`scripts/btrfs-subvol-smoke.sh`) against a loop-mounted Btrfs image with a real, non-default `layerfs` subvolume in a privileged container. `layerfs.luks=<device-spec>` (with an optional `layerfs.luks_key=<path>`) unlocks an encrypted store device before mounting: `layerfs-init/src/luks.rs` shells out to `cryptsetup luksOpen`, matching the codebase's existing pattern of shelling to `btrfs` for backend operations it doesn't reimplement — `cryptsetup` handles keyfile vs. interactive-passphrase prompting itself, so `layerfs-init` doesn't need any termios/console handling of its own. Verified for real in a privileged container against an actual `cryptsetup luksFormat --type luks2` device: unlocking with a keyfile via the `layerfs-init` library function directly, and the full `layerfs.luks=`+`layerfs.store=` pipeline unlocking a real LUKS2 device and mounting a real Btrfs filesystem inside it. Needs `cryptsetup`/`dm-crypt` embedded in the initramfs (documented per integration; dracut's own `crypt` module already does this) — not exercised under a full QEMU `rdinit=` boot, since that would mainly be testing dracut/mkinitcpio's own well-established crypt-module packaging rather than LayerFS logic.
- [x] `switch_root`: `layerfs-init` now assembles at `/sysroot`, carries `/dev`, `/proc`, `/sys`, and `/run` into the new root, then moves the assembled mount to `/`, chroots, and execs `/sbin/init`. Verified by `scripts/qemu-switch-root-smoke.sh`: the actual initramfs binary hands off to `/sbin/init` from the composed OverlayFS root, which reads an OVERRIDE-shadowed marker and powers off QEMU.
- [x] Fedora dracut module: `integrations/dracut/module-setup.sh` embeds `layerfs-init`, includes OverlayFS, and emits `rdinit=/sbin/layerfs-init`; `scripts/dracut-smoke.sh` builds an isolated image with the real dracut binary and confirms both payloads are present. An earlier revision of this note claimed a dracut `cmdline` hook made the initqueue wait for the store device before `layerfs-init` runs, "verified for real" by checking the hook landed in the built image and passed `sh -n`. That hook was dead code and has been removed: `rdinit=/sbin/layerfs-init` means the kernel execs `layerfs-init` directly in place of the initramfs's own `/init`, so dracut's `/init` — and every hook it would have run, including that one — never executes at all on a real boot. Confirmed by booting a probe binary the exact same way (`rdinit=/probe`, no dracut `/init` involved) and observing none of dracut's normal boot-time setup happened. The actual fix for "backing device might not be there yet" now lives in `layerfs-init` itself as a bounded retry loop around device resolution, not in a dracut hook that can't run under this boot architecture. LUKS activation now works too (`layerfs.luks=`); the module's `README.md` documents that a real image also needs `--add crypt` for `cryptsetup`/`dm-crypt` to be present.
- [x] GRUB entries accept `--rdinit <path>` and repeat it across all five checkpoint entries. The real GRUB/QEMU harness now supplies `rdinit=/init`, proving GRUB passes the selected initramfs entrypoint through to the booted kernel.

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
- [x] `--integrations dnf,apt` bakes `layerfs.integrations=` into every entry's cmdline, parsed by `layerfs_core::BootOptions` the same way as checkpoint/head/store — re-chosen per boot, no separate config file
- [x] installed into an actual `/etc/grub.d/`: `layerctl install --grub-entries <path-to-built-binary>` copies it in as `etc/grub.d/41_layerfs`, executable, so `grub2-mkconfig` picks it up on the installed system — errs if the source tree has no `etc/grub.d` (not a GRUB system) rather than silently doing nothing. Verified for real: a fixture with an existing `etc/grub.d/10_linux` gets `41_layerfs` installed alongside it, executable, and actually running it against real registered boot artifacts (`layerctl boot-register`) produces valid GRUB entries; a source tree without `etc/grub.d` is correctly rejected
- [x] kernel/initramfs paths were already discovered from tracked boot artifacts (`layerfs_storage::boot::discover`, `integrations/grub/src/main.rs`), not passed as raw `--linux`/`--initrd` flags — this note was stale by the time it was written; verified for real against real registered boot artifacts

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
- [x] `install` — see Milestone 9
- [x] `rollback update` — discards the active UPDATE_HEAD, holding `transaction.lock` so it can't race a concurrent transaction; refuses a target other than `"update"` and refuses when there's no active UPDATE_HEAD to discard. Verified for real (`unshare --map-root-user --mount --user`): two chained transactions correctly squash the first's content into UPDATE (per `stage`'s squash-on-commit logic) leaving UPDATE_HEAD holding only the second's; `rollback update` discards UPDATE_HEAD leaving UPDATE (and its content) intact; a second rollback correctly refuses with nothing left to roll back
- [x] `checkpoint <name> [--bootloader systemd-boot|grub] [--esp <path>] [--grub-cfg <path>] [--grubenv <path>]` — implements the section 25 hint directly: configures the *next* boot rather than touching the currently mounted root. `name` is one of the four canonical checkpoints (`layerfs_core::Checkpoint`, numeric or named); for systemd-boot (the default) it maps to the matching BLS entry (`layerfs-<name>.conf`) and rewrites `<esp>/loader/loader.conf`'s `default` line (preserving every other line), written via the same write-temp-then-rename pattern used for metadata commits elsewhere; for GRUB it maps to a `menuentry --id 'layerfs-<name>'` (each of the five hardcoded entries in `integrations/grub` now carries a stable `--id` alongside its title, matching the systemd-boot entry names 1:1) and writes `saved_entry=layerfs-<name>` directly into the fixed 1024-byte `grubenv` block format (signature line + `key=value` entries + `#` padding), preserving any other variables already there. Refuses if that checkpoint's entry doesn't exist (grepping the rendered `grub.cfg` for GRUB, since its entries live in one file rather than as separate files the way BLS ones do) or if `name` isn't one of the four checkpoints, rather than silently writing a default the boot loader can't resolve. GRUB additionally needs `GRUB_DEFAULT=saved` set in the system's own GRUB config (a `/etc/default/grub` setting outside LayerFS's control) for `saved_entry` to actually take effect — `checkpoint`'s output says so explicitly rather than implying success
- [x] verified for real: registered a boot generation, generated real systemd-boot entries with `layerfs-systemd-boot` into a fixture ESP, ran `layerctl checkpoint safe --esp <esp>` and confirmed `loader.conf` correctly gained `default layerfs-safe.conf`; a second call for a checkpoint with no generated entry (`base`) is correctly refused, as is a bogus checkpoint name. For GRUB: generated real entries with the updated `layerfs-grub-entries`, confirmed the `--id`-bearing output still passes real `grub2-script-check`; ran `layerctl checkpoint safe --bootloader grub` and confirmed the real `grub2-editenv <path> list` tool reads back `saved_entry=layerfs-safe` from a file `layerctl` wrote with no GRUB tooling involved; separately confirmed the reverse — a `grubenv` created and populated by real `grub2-editenv` (with an extra unrelated variable) is correctly read, updated, and preserved by `layerctl checkpoint`, round-tripping cleanly through `grub2-editenv list` again afterward. Plus 12 unit tests across both `layerctl` (`loader.conf` creation/replacement, `grubenv` round-trip, other-vars preservation, both rejection paths for each bootloader) and `layerfs-grub` (entry `--id` rendering)
- [~] `rebuild` remains a stub; `doctor` now reports read-only store health: layer discovery, backend validation, generation consistency, structural BASE checks, and registered boot-artifact completeness. `rebuild` needs a package-manifest/replay design before it can safely reconstruct UPDATE/UPDATE_HEAD.

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
- [x] real `BtrfsBackend` (was a stub returning `NotImplemented` everywhere): shells out to the `btrfs` CLI for `subvolume create`/`snapshot`/`delete` and `property set ro`; `layerfs_storage::detect_backend` picks it automatically via `statfs` when the store root sits on Btrfs, `DirectoryBackend` otherwise — `layerctl install`/`transaction` now use this instead of hardcoding `DirectoryBackend`. Verified for real against a loop-mounted Btrfs image in a privileged container: subvolume create/CoW-snapshot/read-only-freeze (write correctly rejected)/delete, and a real `install` producing an actual Btrfs subvolume for `base`. Surfaced and fixed a real bug in the process: `install_cmd` moved `home`/`root`/`srv` out of `base` into `data/` via `rename`, which returns `EXDEV` across a Btrfs subvolume boundary even on the same filesystem — fixed with a copy+delete fallback (`layerctl::commands::move_dir`)
- [x] private mount namespace: `Transaction::stage` calls `unshare(CLONE_NEWNS)` before mounting, so a transaction never sees OVERRIDE and its mount doesn't leak into the parent namespace (section 13)
- [x] `/proc`, `/sys`, `/dev`, `/run` bound (recursively) into the transaction root after assembly, unmounted again in both `commit()` and the failure-cleanup `Drop`, so a real package manager's scriptlets (dracut, ldconfig, systemd-sysusers, ...) see the same virtual filesystems they would on a live system — previously the transaction root had none of these, silently breaking any real transaction whose scriptlets touched them. Verified for real (`unshare --map-root-user --mount --user`, a static musl probe binary chrooted via `layerctl transaction`): `/proc/version` and `/dev/null` are both readable from inside the transaction root
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
- [x] generic xattrs, hardlinks, and capabilities: `copy_tree` now carries every xattr, preserves inode identity for hardlinked regular files, and therefore carries `security.capability`. Verified with a normal `user.*` xattr test and a privileged Docker test using real `setcap`/`getcap`.
- [x] a path changing type between the two layers (file→dir or dir→file) is resolved as "upper's type wins outright" (`merge_from_b` falls through to a plain `copy_tree` whenever the upper entry's type doesn't match the lower's), not modeled as an implicit whiteout-then-create — real package managers essentially never do this. Covered for both directions: `upper_type_change_wins_outright` (file→dir) and `upper_type_change_dir_to_file_wins_outright` (dir→file)
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
- [x] exercised against an actual `dnf` transaction against a real Fedora root, through the *installed* adapter, not just an env-var override: `scripts/dnf-adapter-smoke.sh` bootstraps a minimal Fedora 42 base with real `dnf --installroot`, runs `layerctl install --integrations dnf --adapter-bin dnf=<built layerfs-dnf>` to activate it exactly as a retrofit install would, then invokes plain `dnf` (now `usr/bin/dnf`, a symlink to `layerfs-dnf`) to `install which` — the real `dnf` resolver/downloader/RPM transaction runs chrooted into the staged transaction root, commits into a fresh `UPDATE_HEAD`, and leaves the base layer untouched
- [x] a real, non-obvious bug was found and fixed here: `activate_integrations` used to `remove_file` the real binary outright and symlink `usr/bin/dnf` straight to a `layerfs-dnf` file that nothing ever installed — so activation destroyed the only copy of the real `dnf` and left a symlink pointing at a nonexistent target; worse, `layerfs-dnf`'s own default fallback binary name was the literal string `"dnf"`, so even a present wrapper would have execed itself in an infinite loop instead of the real thing. Fixed by having `layerctl install --adapter-bin <name>=<path>` actually copy the adapter binary in as `layerfs-<name>`, preserving the real binary alongside it as `<real_name>.layerfs-real` instead of deleting it, and changing each adapter's default fallback (`dnf.layerfs-real`, `apt-get.layerfs-real`, `pacman.layerfs-real`) to match — covered by new `layerctl` unit tests (`wraps_real_binary_and_preserves_it`, `rejects_missing_adapter_binary`, `rejects_unknown_integration`, `skips_absent_real_binary_without_error`) and verified for real by the smoke test above

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

- [x] `layerfs-storage::boot` — BOOT_BASE/BOOT_UPDATE/BOOT_HEAD generations, reusing the same symlink-swap primitives as root's UPDATE/UPDATE_HEAD (`generations::new_generation_path`/`activate`/`current`); `register()` copies a kernel+initramfs pair into a new generation and atomically activates it
- [x] `layerctl boot-register <name> --kernel <path> --initramfs <path>` — manual registration primitive
- [x] wired into `layerctl transaction`'s commit path (so both the dev command and every real adapter, which all funnel through it): `layerfs_storage::boot::find_new_kernel` scans the just-committed `UPDATE_HEAD` for a `boot/vmlinuz-<version>` with a matching `boot/initramfs-<version>.img` — since `head_next` starts empty for every transaction (`stage()` calls `prepare_layer(&head_next, None)`), anything found there was written by *this* transaction's copy-up, not inherited from a lower layer. When present, it's registered as the new `head` boot generation automatically; a transaction that never touches `/boot` is a silent no-op. Building the initramfs itself still relies on dracut running as an RPM scriptlet inside the transaction (needs the `/proc`/`/sys`/`/dev`/`/run` binds added alongside this), not on LayerFS driving dracut directly
- [x] verified for real (`unshare --map-root-user --mount --user`, a static musl stand-in that writes `boot/vmlinuz-9.9.9-test` + `boot/initramfs-9.9.9-test.img` instead of a real kernel install): `layerctl transaction` prints `registered boot generation: ...` and `layerctl status` shows the new `boot: head` generation active immediately after commit; new unit tests in `layerfs-storage::boot` (`find_new_kernel_pairs_matching_version`, `find_new_kernel_ignores_vmlinuz_without_matching_initramfs`, `find_new_kernel_picks_most_recently_modified`, `find_new_kernel_none_when_boot_untouched`) cover the selection logic
- [x] `layerfs-grub-entries` resolves kernel/initramfs per entry instead of one path shared by all five: Normal/Safe/System use BOOT_HEAD, Previous Update uses BOOT_UPDATE, Base Recovery uses BOOT_BASE — each falling back to the next lower tier if its own generation is missing, and an entry is skipped entirely (not emitted with a broken path) if no tier resolves at all
- [x] `layerctl status` now also reports the three boot generations
- [x] verified for real: `scripts/qemu-boot-artifacts-smoke.sh` registers the real host kernel under two distinguishable initramfs images as BASE and HEAD, generates real GRUB entries, and boots both the Normal and Base Recovery entries under QEMU/KVM — each printed the artifact name baked into the initramfs GRUB actually loaded, proving per-entry selection end to end, not just that the paths look right

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

- [x] `layerctl install --source <dir>` implemented for the offline, single-step case: copies a static source tree into a new `base`, extracts the `DATA_MOUNTS` paths (`home`/`root`/`srv`) out into `data/`, creates an empty `override`, validates the result — refuses if a `base` already exists or the source isn't a directory
- [x] verified for real: installed from a scratch fixture tree, confirmed `home`/`root`/`srv` landed under `data/` and not `base/`, `/var/lib/rpm` correctly stayed in `base/` (not treated as data), a second install over the same store refuses, and the resulting store composes correctly as a real overlay mount (base content plus override shadowing both visible)
- [ ] not the full section 31 flow — that reboots into a dedicated migration initramfs specifically to avoid copying a live, concurrently-changing filesystem; `install` here requires an already-static source (a mounted image, a snapshot, anything not being written to), and doesn't touch GRUB/dracut/boot artifacts at all
- [x] `layerctl install --integrations dnf,apt --adapter-bin dnf=<path> --adapter-bin apt=<path>` activates the matching adapters: installs each given built adapter binary as `layerfs-<name>` inside the new base, renames each present real binary (`usr/bin/dnf`, `usr/bin/apt-get`/`usr/bin/apt`, `usr/bin/pacman`) to `<real_name>.layerfs-real` (preserved, not deleted), and symlinks the real name to `layerfs-<name>` — the GRUB side (baking `layerfs.integrations=` into the cmdline, Milestone 3) was already in place, this was the missing activation step. An unrecognized integration name, or one missing its `--adapter-bin`, is rejected at install time rather than silently booting with it inactive or dangling; a real binary absent from the source tree (package not installed) is skipped rather than an error
- [x] verified for real: `layerctl` unit tests (`wraps_real_binary_and_preserves_it`, `rejects_missing_adapter_binary`, `rejects_unknown_integration`, `skips_absent_real_binary_without_error`) plus `scripts/dnf-adapter-smoke.sh` installing over a real Fedora root, confirming the resulting `usr/bin/dnf` symlink actually drives a real `dnf` transaction end to end (see Milestone 7)

Support converting an existing Fedora Btrfs installation.

```bash
sudo layerctl install
```

followed by an offline migration reboot.

---

## Milestone 10 — Additional platforms

Progress:

- [x] generic ext4 (or any non-Btrfs) backend — already satisfied by `DirectoryBackend` plus `layerfs_storage::detect_backend`'s automatic fallback (Milestone 5); no ext4-specific code needed since it never had native subvolume/snapshot support to use
- [x] Pacman adapter (`integrations/pacman`, `layerfs-pacman`) — same `layerfs-adapter` runner as DNF, its own `classify::is_mutating` reading pacman's operation flag (`-S`/`-R`/`-U`/`-Q`/`-F`/`-D`/`-T`/`-V`/`-h`, short or long, since pacman takes an operation rather than a verb); `-S` is mutating unless paired with an informational modifier (`-Ss`/`-Si`/`-Sl`/`-Sp`/`-Sw`/`--downloadonly`), `-D` is conservatively mutating, unrecognized non-empty invocations default to mutating like DNF's unknown-verb case
- [x] verified for real end to end (`unshare --map-root-user --mount --user`, a statically-linked musl stand-in `pacman` chrooted into a real transaction root): `-S vim` mutates the store and hot-applies live with no reboot, content changing from `v1` to `v2` in a stand-in live root
- [x] APT adapter (`integrations/apt`, `layerfs-apt`) — same runner and pattern as the Pacman adapter, wraps `apt-get` (not `apt`, whose CLI is documented as unstable for scripting); `classify::is_mutating` mirrors DNF's allowlist-of-read-only-verbs approach (`list`/`search`/`show`/`update`/... are read-only, everything else including unknown verbs is mutating), with `--dry-run`/`-s`/`--just-print`/`--download-only`/`-d` forcing read-only regardless of verb
- [x] verified for real end to end, same method as the Pacman adapter: `install curl` mutates the store and hot-applies live with no reboot
- [~] initramfs-tools integration: `integrations/initramfs-tools/hooks/layerfs` embeds `layerfs-init` plus OverlayFS and Btrfs modules; `scripts/initramfs-tools-smoke.sh` builds a real Debian initramfs from extracted kernel modules and verifies all three payloads. mkinitcpio integration: `integrations/mkinitcpio/install/layerfs` embeds `layerfs-init` and requests OverlayFS/Btrfs when modular; `scripts/mkinitcpio-smoke.sh` generates a real Arch initramfs from extracted kernel modules and verifies the binary plus the modular OverlayFS payload. `layerfs-systemd-boot` generates the five BLS entries from registered boot artifacts, including the matching checkpoint, fallback tier, store, integrations, and `rdinit` options; `scripts/qemu-systemd-boot-smoke.sh` verifies OVMF → systemd-boot → BLS → LayerFS root handoff with a temporary ESP and Btrfs store. The boot loader must still provide `rdinit=/sbin/layerfs-init` and make `layerfs.store=/dev/...` available.

After Fedora/Btrfs is stable:

```text
Arch + mkinitcpio + Pacman
Ubuntu + initramfs-tools + APT
generic ext4 backend
systemd-boot
```

---

## Milestone 11 — Live apply (not in the original design notes)

Progress:

- [x] `layerfs_storage::overlay::hot_apply` — assembles OVERRIDE over UPDATE_HEAD/UPDATE over BASE (each scoped to the subtree being applied) fresh from the store's real layers, then atomically swaps it in with `mount --move`. Already-open files on already-running processes keep their old content (standard Unix replace-while-open semantics); new opens see the change immediately.
- [x] `layerctl apply-now [--live-root <path>]` — manual trigger.
- [x] `layerfs_storage::risk` — classifies a committed UPDATE_HEAD as safe or risky to hot-apply by path prefix (`usr/lib*`, `lib*`, `boot`, `usr/lib/systemd` are risky: shared libraries/kernel/systemd already loaded by running processes wouldn't actually update by swapping the mount, only new process launches would, which is a real correctness hazard, not just a cosmetic one).
- [x] `layerfs-adapter` calls `apply-now` automatically after a successful commit when the update is classified safe; otherwise it reports that a reboot is required, matching the fail-safe-toward-caution rule used everywhere else in this codebase.
- [x] a real, non-obvious bug was found and fixed here: the adapter originally ran the transaction in-process via `layerfs-transaction` directly. `Transaction::stage` unshares into a private mount namespace for isolation — but since that happened in the *same* process that would later try to hot-apply, the hot-apply mounts landed in a namespace that died with the process, invisible to the real system the whole time despite reporting success. Fixed by having the adapter spawn `layerctl transaction` as a separate child process instead of linking the transaction engine directly — the child's private namespace is destroyed when it exits, leaving the adapter process in the real one for `apply-now` to actually affect.
- [x] verified for real: registered a store, bind-mounted a fixed directory as a stand-in "live root", ran a real transaction against it, confirmed the live root did *not* change before `apply-now`, then confirmed it did after — for both the manual `layerctl apply-now` path and the adapter's automatic path; separately confirmed a "risky" update (touching `usr/lib64`) is committed but correctly left unapplied
- [x] a second real bug was found and fixed here: `hot_apply`'s snapshot step used a non-recursive bind mount, so submounts nested under the live root (`/proc`, `/sys`, `/home`, ...) weren't captured, and `mount --move` onto `/` would orphan them — still mounted in the kernel but unreachable from any path. Fixed by never touching `/` at all: `layerfs_storage::risk::hot_applicable_scopes` restricts hot-apply to top-level directories that don't normally have their own nested mounts (`usr`, `opt`), and `layerfs_storage::live_update::apply` (now the shared entry point used by both `layerctl apply-now` and the adapter) applies each scope independently, refusing with `RequiresReboot` if the update touches anything outside that allowlist (e.g. `etc`)
- [x] verified for real: created a fake `/proc`-style submount (real bind mount with distinct content) nested under the stand-in live root, ran a scoped `usr` hot-apply, and confirmed the submount survived intact and reachable; separately confirmed an update touching `etc` is rejected with "cannot apply live: touches a path outside usr/opt or a shared library/kernel" instead of being (incorrectly) applied
- [x] repeated applies within a boot session now reconcile correctly instead of accumulating: a real bug was found here (not just the "not a merge" cosmetic concern this item started as, but an actual failure) — `hot_apply` built its new stack on top of a bind-mounted *snapshot of whatever was currently at `target`*, so each `apply-now` nested a fresh OverlayFS mount on top of the previous `apply-now`'s OverlayFS mount. Content-wise this happened to still read correctly, but three chained applies against the same scope hit the kernel's OverlayFS stacking-depth limit and failed outright with a bare `Invalid argument (os error 22)`, and every apply left the previous mount behind as a shadowed, unreachable mount that only grew across the session. Fixed by dropping the live-snapshot entirely and having `hot_apply` compose straight from the store's real layers every time (`override(scope) > update_head(scope) > update(scope) > base(scope)`, mirroring the normal boot stack but scoped) plus explicitly unmounting whatever was previously at `target` before `mount --move`-ing the fresh assembly in, so each apply is self-contained and the mount count stays flat instead of growing
- [x] verified for real (`unshare --map-root-user --mount --user`): 8 chained transactions each writing a distinct marker file under `/usr`, with `apply-now` after every one — before the fix, the 3rd apply failed with `os error 22`; after the fix all 8 succeed, all 8 markers are simultaneously visible in the live root afterward, and `/proc/self/mountinfo`'s count for the live root stays constant across all 8 applies instead of growing. Also caught and fixed a regression the `/proc`/`/sys`/`/dev`/`/run` transaction bind-mount work (Milestone 5) introduced along the way: `mount_virtual_filesystems`' `mkdir` for those mountpoints copied empty `proc`/`sys`/`dev`/`run` directories into `UPDATE_HEAD` whenever `BASE` didn't already have them, which then read as "touches a path outside usr/opt" and permanently blocked hot-apply for *any* future transaction, not just ones that actually changed anything outside `usr`/`opt`. Fixed by adding `layerfs_core::VIRTUAL_MOUNTS` and having `layerctl install` seed `BASE` with those four directories up front, the same way `filesystem`-type packages do on a real distro, so binding onto them is a pure mount operation with no copy-up

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
