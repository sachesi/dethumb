%define _debugsource_template %{nil}
%define debug_package %{nil}

Name:           dethumb
Version:        0.3.0
Release:        1%{?dist}
Summary:        Linux .desktop and Windows EXE thumbnailer

License:        GPL-3.0-or-later
URL:            https://github.com/sachesi/dethumb

BuildRequires:  rust
BuildRequires:  cargo

%description
A small Rust utility that generates PNG thumbnails for Linux .desktop
files and Windows .exe executables. Integrates with Nautilus and other
GTK file managers via the freedesktop thumbnailer protocol.

%prep
# Nothing to do — built in-place with --build-in-place

%build
cargo build --release

%install
install -Dm0755 target/release/dethumb \
    %{buildroot}%{_bindir}/dethumb
install -Dm0644 dethumb.thumbnailer \
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
* Fri Apr 03 2026 sachesi <sachesi.bb.passp@proton.me> - 0.3.0-1
- Refactor and harden EXE PE-resource icon extraction paths
- Improve malformed resource handling and PNG scanning robustness
- Add focused regression tests for icon group parsing and candidate selection

* Thu Apr 02 2026 sachesi <sachesi.bb.passp@proton.me> - 0.2.10-1
- Refactor architecture around library entrypoints and core module boundaries
- Harden icon/path validation and error handling for safer runtime behavior
- Improve image rendering pipeline and reduce duplicate fallback/output writing code
- Enable stricter rust/clippy lint configuration and add focused unit tests

* Sun Mar 09 2026 sachesi <sachesi.bb.passp@proton.me> - 0.2.0-1
- Reorganize project directories
- Refactor thumbnailer logic into focused modules for readability and maintainability
- Address review feedback in module refactor
- Harden path handling for safer thumbnail generation
- Reduce duplicate icon lookups and speed up resize filter
- Rename candidate generation unit tests

* Sat Mar 08 2026 sachesi <sachesi.bb.passp@proton.me> - 0.1.0-1
- Initial release
