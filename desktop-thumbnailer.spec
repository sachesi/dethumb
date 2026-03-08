%define _debugsource_template %{nil}
%define debug_package %{nil}

Name:           desktop-thumbnailer
Version:        0.1.0
Release:        1%{?dist}
Summary:        Linux .desktop and Windows EXE thumbnailer

License:        GPL-3.0-or-later
URL:            https://github.com/sachesi/desktop-thumbnailer

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
install -Dm0755 target/release/desktop-thumbnailer \
    %{buildroot}%{_bindir}/desktop-thumbnailer
install -Dm0644 desktop-thumbnailer.thumbnailer \
    %{buildroot}%{_datadir}/thumbnailers/desktop-thumbnailer.thumbnailer

%files
%license LICENSE
%doc README.md
%{_bindir}/desktop-thumbnailer
%{_datadir}/thumbnailers/desktop-thumbnailer.thumbnailer

%post
:

%postun
:

%changelog
* Sat May 17 2025 sachesi <sachesi.bb.passp@proton.me> - 0.2.0-1
- Fix spec for --build-in-place
- Correct binary and thumbnailer file names
- Remove unused Source0 and broken %%setup macro
