%define _debugsource_template %{nil}
%define debug_package %{nil}

# Default to source-archive builds (SRPM/local sources). For COPR SCM builds,
# pass: --without local_sources
%bcond_without local_sources

Name:           dethumb
Version:        0.3.1
Release:        1%{?dist}
Summary:        Linux .desktop and Windows EXE thumbnailer

License:        GPL-3.0-or-later
URL:            https://github.com/sachesi/dethumb
%if %{with local_sources}
Source0:        %{name}-%{version}.tar.gz
%endif

BuildRequires:  rust
BuildRequires:  cargo

%description
A small Rust utility that generates PNG thumbnails for Linux .desktop
files and Windows .exe executables. Integrates with Nautilus and other
GTK file managers via the freedesktop thumbnailer protocol.

%prep
%if %{with local_sources}
%autosetup -n %{name}-%{version}
%else
# COPR SCM / build-in-place mode: sources are provided by the checkout.
:
%endif

%build
cargo build --release --locked

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
* Mon Apr 06 2026 sachesi <sachesi.bb.passp@proton.me> - 0.3.1-1
- Bump project/package version to 0.3.1
- Keep COPR SCM make_srpm packaging support via .copr/Makefile

* Sat Apr 04 2026 sachesi <sachesi.bb.passp@proton.me> - 0.3.0-1
- Make spec COPR-friendly for both source-archive and SCM build-in-place workflows
- Add local_sources build condition and Source0 for SRPM/local builds
- Keep build-in-place fallback for COPR SCM builds without bundled sources

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
