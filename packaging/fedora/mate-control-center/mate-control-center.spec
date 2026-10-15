# Fedora f45's mate-control-center spec with the smallest diff, so a change there shows what
# to carry over. The differences: the 1.29.1 release, its tarball's address, Fedora's
# patch of an upstream commit, which 1.29.1 has, out, the carried patches, the caminito
# Release and the changelog (ADR 0011, CMN-170).
%global branch 1.29

Name:          mate-control-center
Version:       %{branch}.1
Release:       0.caminito1%{?dist}
Summary:       MATE Desktop control-center
# Automatically converted from old format: LGPLv2+ and GPLv2+ - review is highly recommended.
License:       LicenseRef-Callaway-LGPLv2+ AND GPL-2.0-or-later
URL:           https://mate-desktop.org
Source0:       https://github.com/mate-desktop/mate-control-center/releases/download/v%{version}/%{name}-%{version}.tar.xz

# Fedora's patch of https://github.com/mate-desktop/mate-control-center/commit/b5f6b8c is in
# 1.29.1.

# Caminito's carried patches, from packaging/patches/mate-control-center/ (ADR 0030), which
# packaging/fedora/build copies beside this. Their gates and what each does are in
# packaging/arch/mate-control-center-caminito/PKGBUILD.
Patch101:      0001-wm-common-Know-Caminito-from-the-environment-off-X11.patch
Patch102:      0002-windows-Open-under-Caminito.patch
Patch103:      0003-keybindings-Do-not-crash-on-a-display-that-isn-t-X11.patch
Patch104:      0004-mouse-keyboard-Run-on-XWayland-under-Caminito.patch
Patch105:      0005-mouse-Show-the-touchpad-page-under-Caminito.patch
Patch106:      0006-system-info-Name-Caminito-in-the-Windowing-System-row.patch
Patch107:      0007-keybindings-Title-the-dialog-as-its-launcher-under-C.patch
Patch108:      0008-keybindings-Capture-a-mouse-button-under-Caminito.patch
Patch109:      0009-appearance-Leave-out-the-Background-tab-under-Camini.patch

BuildRequires: accountsservice-devel
BuildRequires: dconf-devel
BuildRequires: desktop-file-utils
BuildRequires: gtk3-devel
BuildRequires: libappindicator-gtk3-devel
BuildRequires: libcanberra-devel
BuildRequires: libgtop2-devel
BuildRequires: libmatekbd-devel
BuildRequires: librsvg2-devel
BuildRequires: libSM-devel
BuildRequires: libudisks2-devel
BuildRequires: libXScrnSaver-devel
BuildRequires: make
BuildRequires: mate-common
BuildRequires: mate-desktop-devel
BuildRequires: mate-menus-devel
BuildRequires: mate-settings-daemon-devel
BuildRequires: marco-devel
BuildRequires: polkit-devel
BuildRequires: systemd-devel

Requires: gsettings-desktop-schemas
# rhbz (#1234438)
Requires: mate-settings-daemon
# keyring support
Requires: gnome-keyring
Requires: %{name}-filesystem%{?_isa} = %{version}-%{release}


%description 
MATE Control Center configures system settings such as themes,
keyboards shortcuts, etc.

%package filesystem
Summary:      MATE Control Center directories
# NOTE: this is an "inverse dep" subpackage. It gets pulled in
# NOTE: by the main package an MUST not depend on the main package

%description filesystem
The MATE control-center provides a number of extension points
for applications. This package contains directories where applications
can install configuration files that are picked up by the control-center
utilities.

%package devel
Summary:      Development files for mate-settings-daemon
Requires:       %{name}%{?_isa} = %{version}-%{release}

%description devel
Development files for mate-control-center


%prep
%autosetup -p1

NOCONFIGURE=1 ./autogen.sh

%build
%configure                           \
           --disable-static          \
           --disable-schemas-compile \
           --disable-update-mimedb   \
           --enable-appindicator

# remove unused-direct-shlib-dependency
sed -i -e 's! -shared ! -Wl,--as-needed\0!g' libtool

make %{?_smp_mflags} V=1


%install
%{make_install}

find %{buildroot} -name '*.la' -exec rm -rf {} ';'
find %{buildroot} -name '*.a' -exec rm -rf {} ';'

desktop-file-install                                \
    --delete-original                               \
    --dir=%{buildroot}%{_datadir}/applications      \
%{buildroot}%{_datadir}/applications/*.desktop

# delete mime cache
rm %{buildroot}%{_datadir}/applications/mimeinfo.cache

%find_lang %{name} --with-gnome --all-name


%files -f %{name}.lang
%doc AUTHORS COPYING README
%config %{_sysconfdir}/xdg/menus/matecc.menu
%{_bindir}/mate-*
%{_sbindir}/mate-display-properties-install-systemwide
%{_datadir}/applications/*.desktop
%{_datadir}/desktop-directories/matecc.directory
%{_datadir}/icons/hicolor/*/apps/*.png
%{_datadir}/icons/hicolor/*/categories/*.png
%{_datadir}/icons/hicolor/scalable/apps/mate-*.svg
%{_datadir}/glib-2.0/schemas/org.mate.*.xml
%{_datadir}/mate-control-center/*
%{_datadir}/mate-time-admin/
%{_datadir}/mime/packages/mate-theme-package.xml
%{_datadir}/thumbnailers/mate-font-viewer.thumbnailer
%{_datadir}/polkit-1/actions/org.mate.randr.policy
%{_mandir}/man1/*.1.*

%files filesystem
%dir %{_datadir}/mate-control-center/
%dir %{_datadir}/mate-control-center/keybindings/

%files devel
%{_libdir}/pkgconfig/mate-default-applications.pc
%{_libdir}/pkgconfig/mate-keybindings.pc


%changelog
* Thu Oct 15 2026 Juan Pablo Lassala <jpablolassala@gmail.com> - 1.29.1-0.caminito1
- mate-control-center 1.29.1, with Caminito's carried patches, from Fedora f45's spec (CMN-170)
