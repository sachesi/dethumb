%define _debugsource_template %{nil}
%define debug_package %{nil}


Name:           dethumb
Version:        0.3.3
Release:        1%{?dist}
Summary:        Thumbnailer for Linux .desktop files and Windows .exe binaries

License:        GPL-3.0-or-later
URL:            https://github.com/sachesi/dethumb
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/%{name}-%{version}.tar.gz

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
%autosetup -n %{name}-%{version}

%build
export CARGO_HOME=$PWD/.cargo-home
export RUSTFLAGS="%{?build_rustflags}"
cargo build --release

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
%{_datadir}/thumbnailers/dethumb.thumbnailer

%changelog
