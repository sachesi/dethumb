# dethumb

Generates PNG thumbnails for Linux `.desktop` files and Windows `.exe`
binaries. Integrates with file managers via the freedesktop thumbnailer
protocol.

## Usage

```
dethumb <input> <output.png> <size> [--debug]
```

Input type is detected by extension:
- `.desktop` — renders the icon from the `Icon=` entry using the system
  icon theme
- `.exe` — extracts the embedded icon from PE resources

## Packages

Fedora 44, 45 and Rawhide, from the Copr project
[sachesi/software](https://copr.fedorainfracloud.org/coprs/sachesi/software/):

```
sudo dnf copr enable sachesi/software
sudo dnf install dethumb
```

openSUSE Tumbleweed and Slowroll, from the OBS project
[home:sachesi:software](https://build.opensuse.org/project/show/home:sachesi:software); for
Slowroll the address has `openSUSE_Slowroll` in it, and on aarch64 `openSUSE_Factory_ARM`:

```
sudo zypper addrepo https://download.opensuse.org/repositories/home:sachesi:software/openSUSE_Tumbleweed/home:sachesi:software.repo
sudo zypper install dethumb
```

Debian testing and Ubuntu 26.04, from the same OBS project; for Ubuntu the addresses
have `xUbuntu_26.04` in place of `Debian_Testing`:

```
sudo install -d /etc/apt/keyrings
curl -fsSL https://download.opensuse.org/repositories/home:sachesi:software/Debian_Testing/Release.key | sudo gpg --dearmor -o /etc/apt/keyrings/sachesi-software.gpg
echo 'deb [signed-by=/etc/apt/keyrings/sachesi-software.gpg] https://download.opensuse.org/repositories/home:sachesi:software/Debian_Testing/ /' | sudo tee /etc/apt/sources.list.d/sachesi-software.list
sudo apt update
sudo apt install dethumb
```

Arch Linux: the AUR package `dethumb`, built from
[packaging/aur/PKGBUILD](packaging/aur/PKGBUILD), which each release tag updates.

The same packages are attached to each [release](https://github.com/sachesi/dethumb/releases).

## Build

```
cargo build --release
```

## License

GPL-3.0-or-later — see `LICENSE`.

## Release checks

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit
```
