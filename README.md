# hnm — HackerOS Nix Manager

`hnm` is a lightweight package manager for HackerOS that gives you access to
the entire **nixpkgs** repository (~100,000 packages) without having to deal
with Nix flakes, NixOS modules, or `configuration.nix`. It drives a
dedicated `nix-env` profile at `~/.hnm/profile`, the same way
[Flox](https://flox.dev) uses Nix under the hood.

Full documentation (with terminal demos, color scheme, and a command
reference) lives in [`docs/hnm.html`](docs/hnm.html), also published at
<https://hackeros-linux-system.github.io/HackerOS-Website/tools-docs/hnm.html>.

```
$ hnm install ripgrep bat htop
```

> **Built into HackerOS** — `hnm` ships pre-installed on every HackerOS
> edition. It's designed for HackerOS specifically and doesn't require
> manual installation there.

---

## Features

| | |
|---|---|
| 📦 **nixpkgs access** | All ~100,000 packages from nixpkgs-unstable, searchable instantly via a local index. |
| ⚡ **No flakes / NixOS** | Uses plain `nix-env` — no heavy system setup required. |
| 🔄 **Generations** | Every change creates a new generation. Roll back with one command. |
| 📌 **Version pinning** | Pin packages to a specific version; `hnm update` skips them and `hnm remove` protects them unless forced. |
| 🩺 **Doctor** | Diagnostics for your Nix install, HNM state, and profile/state drift. |
| 🚀 **Auto-bootstrap** | `hnm` installs Nix automatically if it's missing — just run any command. |
| 🗒️ **`.hk` config** | Settings live in `~/.config/hnm/config.hk`, parsed with [`hk-parser`](https://crates.io/crates/hk-parser) — see [Configuration](#configuration). |

## Quickstart

```sh
# Search for a package
hnm search ripgrep

# Install packages
hnm install ripgrep bat htop

# Show information about a package
hnm info ripgrep

# List packages you've installed via hnm
hnm list -i

# Browse everything available in the local index
hnm list

# Update everything (channel + index + installed packages)
hnm update

# Remove a package (asks for confirmation unless --force)
hnm remove htop

# Check Nix installation
hnm check

# Run full diagnostics
hnm doctor
```

## Building from source

```sh
hl run build.hl
# equivalent to:
#   cd source-code && cargo build --release
```

`hnm` needs **Rust ≥ 1.85** (see `rust-version` in `source-code/Cargo.toml`).
That requirement comes from `hk-parser`'s `clap`-based CLI companion binary,
which needs edition-2024 support in the toolchain — `hnm`'s own code stays
on edition 2021. If you don't have Nix yet:

```sh
hl run install-nix.hl
```

## Command reference

### Package commands

| Command | Description |
|---|---|
| `hnm search <query> [--json]` | Search the local nixpkgs index. |
| `hnm install <pkg...> [--no-env]` | Install one or more packages. `--no-env` skips the PATH activation hint printed at the end. |
| `hnm remove <pkg...> [-f\|--force]` | Remove one or more packages. Without `--force`, pinned packages are skipped and you get a y/N confirmation prompt listing what will be removed. |
| `hnm update [pkg...]` | Refresh the nixpkgs channel, rebuild the local index, and upgrade installed (non-pinned) packages. |
| `hnm upgrade` | Self-update `hnm` from GitHub releases. |
| `hnm info <pkg>` | Show detailed information about a package (live query). |
| `hnm list [-i\|--installed] [--json]` | Without a flag: browse the full local package index. With `-i`: only packages installed via `hnm`. |
| `hnm which <pkg>` | Show the binary path of an installed package. |
| `hnm pin <pkg> [version]` | Pin a package so `hnm update`/`hnm remove` leave it alone. |
| `hnm unpin <pkg>` | Remove a pin. |
| `hnm rollback [gen]` | Roll back to a previous `nix-env` generation and resync tracked state. |
| `hnm gc` | Run Nix garbage collection to free store space. |

### System commands

| Command | Description |
|---|---|
| `hnm unpack` | Bootstrap Nix: install it, source the profile, register the nixpkgs-unstable channel, patch shell rc files. |
| `hnm check` | Verify the Nix installation (prints `nix`/`nix-env` versions). |
| `hnm doctor` | Full diagnostics: binaries, channels, profile, store, state/config, PATH, and state-vs-profile drift. |
| `hnm env activate [-y\|--yes]` | Print (or, with `--yes`, automatically apply) the shell rc changes needed to use the HNM profile. |
| `hnm env deactivate [-y\|--yes]` | Print (or apply) the removal of that same block. |
| `hnm env status` | Show current environment / PATH / patch status. |
| `hnm clean` | Remove HNM's download cache and temp files (not the Nix store — use `hnm gc` for that). |
| `hnm version` | Show HNM and Nix version info. |
| `hnm help` | Show the command reference. |

### Aliases

`hnm i` → install · `hnm rm` / `hnm uninstall` → remove · `hnm up` → update · `hnm ls` → list

## Configuration

As of **v0.2**, `hnm` stores its configuration as **`.hk`**
(`~/.config/hnm/config.hk`) — HackerOS's own config format — instead of
TOML. See the format spec at
<https://hackeros-linux-system.github.io/HackerOS-Website/tools-docs/hk.html>
and the crate that parses/serializes it,
[`hk-parser`](https://crates.io/crates/hk-parser).

```hk
! HNM configuration — HackerOS Nix Manager
! format: https://hackeros-linux-system.github.io/HackerOS-Website/tools-docs/hk.html

[hnm]
-> nix_channel     => "https://nixos.org/channels/nixpkgs-unstable"
-> profile_dir     => "/home/user/.hnm/profile"
-> auto_gc         => false
-> max_generations => 10
```

| Key | Default | Description |
|---|---|---|
| `nix_channel` | nixpkgs-unstable URL | Channel used for package resolution. |
| `profile_dir` | `~/.hnm/profile` | Path to the HNM `nix-env` profile. |
| `auto_gc` | `false` | Run `nix-store --gc` automatically after `hnm remove` / `hnm update`. |
| `max_generations` | `10` | Generations to keep after `install`/`remove`/`update`; older ones are pruned with `nix-env --delete-generations`. `0` disables pruning. |

If an old `~/.config/hnm/config.toml` from a pre-0.2 install is found and no
`config.hk` exists yet, `hnm` migrates it automatically on first run.

## State file

`hnm` tracks installed packages separately from the raw Nix profile, in
`~/.local/share/hnm/state.json` — install timestamps, pins, and
descriptions live there. Because this is separate from `nix-env`'s own
bookkeeping, it can drift (e.g. after a manual `nix-env` install, or after
`hnm rollback` switches generations). `hnm list -i`, `hnm rollback`, and
`hnm doctor` all reconcile or report on this drift — see
[`docs/hnm.html`](docs/hnm.html) for details.

## Development notes / changelog

See [`CHANGES.md`](CHANGES.md) for what changed in v0.2 and why — it's the
short version of a fairly large refactor (config format migration, several
previously-inert CLI flags wired up for real, generation tracking, state
sync, etc).

## License

Apache-2.0 — see [`LICENSE`](LICENSE).
