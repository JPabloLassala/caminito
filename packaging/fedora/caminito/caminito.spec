# Caminito, as caminito-git's PKGBUILD and Debian's packaging/debian/caminito/ package it, for
# Fedora (CMN-171): the compositor with Cargo, and the helpers and data with Meson. It
# packages the checkout's committed HEAD, so uncommitted work is never packaged.
# packaging/fedora/build makes the tarball with `git archive` and defines `caminito_count`
# and `caminito_commit`, which are the version's. Build the patched packages first (CMN-170):
# this one needs their -devel packages, and requires them.
%global rev_count %{?caminito_count}%{!?caminito_count:0}
%global rev_commit %{?caminito_commit}%{!?caminito_commit:0000000000000000000000000000000000000000}
%global shortcommit %(c=%{rev_commit}; echo ${c:0:7})

# The workspace's tests, on a private session bus, as the PKGBUILD's check() runs them. They
# are off by default: caminito-theme's goldens and four keymap tests fail on aarch64, as on
# master. `rpmbuild --with tests` runs them.
%bcond tests 0

# The patched and backported packages' baselines, from their specs (ADR 0011). A stock
# package never satisfies them: Fedora's are 1.28.
%global marco_baseline 1.29.1
%global panel_baseline 1.29.0~20260925git58ac6c1
%global session_baseline 1.28.0^20260721git57a9178
%global msd_baseline 1.29.1
%global control_center_baseline 1.29.1
%global utils_baseline 1.29.1
# caja and mate-power-manager have Fedora's own Version, so no baseline tells them from
# Fedora's. Their Release does: 0.caminito<n>, which sorts before Fedora's 1 and so before
# any stock Release. A `>=` on it would let a stock build satisfy it, because Fedora's
# Release is higher, so these are ranges: from the first caminito Release, and below 1.
%global caja_version 1.28.0
%global power_manager_version 1.28.1

Name:           caminito
Version:        0.0~r%{rev_count}.g%{shortcommit}
Release:        1%{?dist}
Summary:        Wayland compositor for the MATE desktop
License:        GPL-2.0-or-later
URL:            https://github.com/JPabloLassala/caminito
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  meson
BuildRequires:  pkgconf-pkg-config
BuildRequires:  libxkbcommon-devel
BuildRequires:  glib2-devel
BuildRequires:  cairo-devel
BuildRequires:  pango-devel
BuildRequires:  gdk-pixbuf2-devel
BuildRequires:  gtk3-devel
BuildRequires:  gtk4-devel
BuildRequires:  libseat-devel
BuildRequires:  systemd-devel
BuildRequires:  mesa-libgbm-devel
BuildRequires:  libdisplay-info-devel
BuildRequires:  libinput-devel
BuildRequires:  libcanberra-devel
BuildRequires:  gtk-session-lock-devel
BuildRequires:  mate-desktop-devel
BuildRequires:  mate-panel-devel >= %{panel_baseline}
BuildRequires:  pam-devel
# The tests: a private session bus, pixman for the headless backend, libwayland-client for the
# fixture's clients and the Xwayland the X11 tier spawns, and python-dbusmock, the BlueZ
# caminito-bluetooth's tests run against (255).
%if %{with tests}
BuildRequires:  dbus-daemon
BuildRequires:  pixman-devel
BuildRequires:  wayland-devel
BuildRequires:  xorg-x11-server-Xwayland
BuildRequires:  python3-dbusmock
%endif

# What the binaries and helpers link rpm finds itself. The rest is what caminito-git's
# depends names at run time, by Fedora's names: librsvg's pixbuf loader for SVG theme
# images, Menta from mate-themes, the Xwayland it spawns, glvnd's libEGL and mesa's EGL and GBM
# and drivers, which it dlopens, zenity for Marco's two dialogs, mate-screensaver's PAM
# service and lock dialogs for caminito-locker, the notification daemon and polkit agent
# mate-session autostarts, hicolor's directory for the logo, mate-desktop's
# org.mate.background, which caminito-wallpaperd reads on its first run, and
# glib-networking, GIO's TLS, which it fetches a picture of the day with.
Requires:       librsvg2
Requires:       mate-themes
Requires:       xorg-x11-server-Xwayland
Requires:       libglvnd-egl
Requires:       mesa-libEGL
Requires:       mesa-libgbm
Requires:       mesa-dri-drivers
Requires:       glib-networking
Requires:       zenity
Requires:       gtk-session-lock
Requires:       mate-desktop-libs
Requires:       mate-screensaver
Requires:       mate-notification-daemon
Requires:       mate-polkit
Requires:       hicolor-icon-theme
# The daemon caminito-bluetooth and the Bluetooth applet talk to (255, 259).
Requires:       bluez
# The session that `caminito --session` starts (08, 17). The patched and backported packages
# are required at their baselines, so that a stock package never satisfies them (16, ADR
# 0011, ADR 0029): marco, the panel, the session manager, the settings daemon, the control
# center and mate-utils by their Version. caja draws the desktop, on the primary output (81),
# mate-utils has mate-screenshot, which the screenshot keys run (104), mate-power-manager has
# the brightness and power keys (107). caminito links none of them.
Requires:       marco >= %{marco_baseline}
Requires:       mate-panel >= %{panel_baseline}
Requires:       mate-session-manager >= %{session_baseline}
Requires:       mate-settings-daemon >= %{msd_baseline}
Requires:       mate-control-center >= %{control_center_baseline}
Requires:       mate-utils >= %{utils_baseline}
Requires:       (caja >= %{caja_version}-0.caminito1 with caja < %{caja_version}-1)
Requires:       (mate-power-manager >= %{power_manager_version}-0.caminito1 with mate-power-manager < %{power_manager_version}-1)
# libheif's heif-dec decodes a HEIC's pictures, run as a program, so nothing links it:
# without it the Wallpaper dialog greys HEICs out and says why (217).
Suggests:       libheif-tools

%description
Caminito is a Wayland compositor for MATE, the counterpart of Marco: it reads Marco's
settings, draws its themes and binds its keys, so that MATE runs on Wayland as it does on
X11.

This package has the compositor, caminito-displays, caminito-about, the wallpaper's drawer
and Wallpaper dialog, caminito-bluetooth and the Bluetooth panel applet, the palette and
locker helpers, and the "MATE on Caminito" session that display managers list.

%prep
%autosetup
# The one step with the network: every later cargo command is offline.
cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"

%build
export CARGO_TARGET_DIR=target
# The revision in the version string (182), which `caminito --version` prints and
# caminito-about shows: this package's, so they agree with `rpm -q`.
export CAMINITO_REVISION=%{version}
export RUSTFLAGS="%{build_rustflags}"
cargo build --frozen --release
%meson
%meson_build

%install
%meson_install
install -Dm755 target/release/caminito target/release/caminito-displays \
    target/release/caminito-about target/release/caminito-wallpaperd \
    target/release/caminito-wallpaper target/release/caminito-bluetooth -t %{buildroot}%{_bindir}

%check
# The meson_test macro expands to more than one line, so it cannot follow dbus-run-session.
dbus-run-session -- meson test -C %{_vpath_builddir} --no-rebuild --print-errorlogs
%if %{with tests}
export CARGO_TARGET_DIR=target
export CAMINITO_REVISION=%{version}
export RUSTFLAGS="%{build_rustflags}"
dbus-run-session -- cargo test --frozen --release
%endif

%files
%license COPYING
%{_bindir}/caminito
%{_bindir}/caminito-about
%{_bindir}/caminito-bluetooth
%{_bindir}/caminito-displays
%{_bindir}/caminito-wallpaper
%{_bindir}/caminito-wallpaperd
%{_bindir}/caminito-gtk-palette
%{_bindir}/caminito-locker
%{_libdir}/mate-panel/libcaminito-bluetooth-applet.so
%{_libdir}/mate-panel/libcaminito-power-applet.so
%{_datadir}/caminito/
%{_datadir}/applications/caminito-bluetooth.desktop
%{_datadir}/glib-2.0/schemas/90_caminito.gschema.override
%{_datadir}/glib-2.0/schemas/org.mate.Caminito.gschema.xml
%{_datadir}/glib-2.0/schemas/org.mate.panel.applet.caminito-power.gschema.xml
%{_datadir}/icons/hicolor/scalable/apps/caminito.svg
%{_datadir}/mate-control-center/keybindings/50-caminito-*.xml
%{_datadir}/mate-panel/applets/org.mate.panel.CaminitoBluetoothApplet.mate-panel-applet
%{_datadir}/mate-panel/applets/org.mate.panel.CaminitoPowerApplet.mate-panel-applet
%{_datadir}/mate-panel/layouts/caminito.layout
%{_datadir}/wayland-sessions/mate-caminito.desktop
%config(noreplace) %{_sysconfdir}/xdg/autostart/caminito-bluetooth.desktop

%changelog
* Thu Oct 15 2026 Juan Pablo Lassala <jpablolassala@gmail.com> - %{version}-1
- Built from the checkout's committed HEAD (CMN-171)
