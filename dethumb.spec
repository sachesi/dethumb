%define _debugsource_template %{nil}
%define debug_package %{nil}

Name:           dethumb
Version:        0.3.1
Release:        1%{?dist}
Summary:        Linux .desktop and Windows EXE thumbnailer

License:        GPL-3.0-or-later
URL:            https://github.com/sachesi/dethumb
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/%{name}-%{version}.tar.gz
Source1:        %{name}-%{version}-vendor.tar.zst

BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  gcc

%description
A small Rust utility that generates PNG thumbnails for Linux .desktop
files and Windows .exe executables. Integrates with Nautilus and other
GTK file managers via the freedesktop thumbnailer protocol.

%prep
%autosetup -n %{name}-%{version}
tar -xaf %{SOURCE1}

mkdir -p .cargo
cat > .cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF

%build
export CARGO_HOME=$PWD/.cargo-home
cargo build --release --frozen --offline

%install
install -Dm0755 target/release/dethumb \
    %{buildroot}%{_bindir}/dethumb
install -Dm0644 /packaging/usr/share/thumbnailers/dethumb.thumbnailer \
    %{buildroot}%{_datadir}/thumbnailers/dethumb.thumbnailer

%files
%license LICENSE
%doc README.md
%{_bindir}/dethumb
%{_datadir}/thumbnailers/dethumb.thumbnailer

%post
:

%postun
:

%changelog
* Thu Apr 23 2026 sachesi <sachesi.bb.passp@proton.me> - 0.3.1-1
- Prepare .spec to copr workflow, add Makefile
- Move .thumbnailer to packaging subdir
- Bump version to 0.3.1

* Fri Apr 03 2026 sachesi <sachesi.bb.passp@proton.me> - 0.3.0-1
- Refactor and harden EXE PE-resource icon extraction paths
- Improve malformed resource handling and PNG scanning robustness
- Add focused regression tests for icon group parsing and candidate selection

* Thu Apr 02 2026 sachesi <sachesi.bb.passp@proton.me> - 0.2.10-1
- Refactor architecture around library entrypoints and core module boundaries
- Harden icon/path validation and error handling for safer runtime behavior
- Improve image rendering pipeline and reduce duplicate fallback/output writing code
- Enable stricter rust/clippy lint configuration and add focused unit tests

* Mon Mar 09 2026 sachesi <sachesi.bb.passp@proton.me> - 0.2.0-1
- Reorganize project directories
- Refactor thumbnailer logic into focused modules for readability and maintainability
- Address review feedback in module refactor
- Harden path handling for safer thumbnail generation
- Reduce duplicate icon lookups and speed up resize filter
- Rename candidate generation unit tests

* Sun Mar 08 2026 sachesi <sachesi.bb.passp@proton.me> - 0.1.0-1
- Initial release
