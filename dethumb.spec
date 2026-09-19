%define _debugsource_template %{nil}
%define debug_package %{nil}


Name:           dethumb
# The release workflow sets Version to the tag it builds; OBS counts the Release.
Version:        0.3.3
Release:        0
Summary:        Thumbnailer for Linux .desktop files and Windows .exe binaries

License:        GPL-3.0-or-later
URL:            https://github.com/sachesi/dethumb
# Named as the Debian source package names them, which OBS builds from the same files.
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/%{name}_%{version}.orig.tar.gz
# The crates the build needs, from the release, so that it runs without a network.
Source1:        %{url}/releases/download/v%{version}/%{name}-%{version}-vendor.tar.xz#/%{name}_%{version}.orig-vendor.tar.xz

BuildRequires:  cargo >= 1.85
BuildRequires:  rust >= 1.85
BuildRequires:  gcc

%global _description %{expand:
dethumb generates PNG thumbnails for Linux .desktop files and Windows .exe
binaries, integrating with file managers via the freedesktop thumbnailer
protocol. Desktop entries are rendered from their Icon= entry using the
system icon theme; executables have their embedded icon extracted from PE
resources.}

%description %{_description}

%prep
%autosetup -n %{name}-%{version} -b 1

%build
export CARGO_HOME=$PWD/.cargo-home
export RUSTFLAGS="%{?build_rustflags}"
cargo build --release --offline --locked

%install
install -Dpm 0755 target/release/dethumb %{buildroot}%{_bindir}/dethumb
install -Dpm 0644 packaging/usr/share/thumbnailers/dethumb.thumbnailer \
  %{buildroot}%{_datadir}/thumbnailers/dethumb.thumbnailer

%check
test -x %{buildroot}%{_bindir}/dethumb

%files
%license LICENSE
%doc README.md docs
%{_bindir}/dethumb
# openSUSE wants every directory owned; nothing required here owns this one.
%dir %{_datadir}/thumbnailers
%{_datadir}/thumbnailers/dethumb.thumbnailer

%changelog
