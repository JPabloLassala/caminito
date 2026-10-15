# Fedora f45's mate-session-manager spec with the smallest diff, so a change there shows what
# to carry over. The differences: master's pinned commit, its snapshot's version and sources,
# autogen, Fedora's backports, which master has, out, the carried patches, the caminito
# Release and the changelog (ADR 0011, CMN-170).
#
# mate-session-manager has no 1.29 tag, so the baseline is a master commit (16), after 1.28.0,
# as on Debian (ADR 0029).
%global commit 57a9178e766cd02176dc1c2d70cc48f963ee9fff
%global shortcommit %(c=%{commit}; echo ${c:0:7})
%global commitdate 20260721
# The mate-submodules commit that master's submodule points to. A GitHub archive leaves the
# submodule out.
%global submodules_commit 799639b54e6e5c2b04a3cd13610ebe54d21b71af

Name:          mate-session-manager
Summary:       MATE Desktop session manager
# Automatically converted from old format: GPLv2+ - review is highly recommended.
License:       GPL-2.0-or-later
Version:       1.28.0^%{commitdate}git%{shortcommit}
Release:       0.caminito1%{?dist}
Source0:       https://github.com/mate-desktop/%{name}/archive/%{commit}/%{name}-%{commit}.tar.gz
Source1:       https://github.com/mate-desktop/mate-submodules/archive/%{submodules_commit}/mate-submodules-%{submodules_commit}.tar.gz

# Fedora's backports of 5424817 and of pull request 323 are in master.

# Caminito's carried patches, from packaging/patches/mate-session-manager/ (ADR 0030), which
# packaging/fedora/build copies beside this. Their gates and what each does are in
# packaging/arch/mate-session-manager-caminito/PKGBUILD.
Patch101:      0001-main-Unset-GDK_BACKEND-under-Caminito.patch
Patch102:      0002-main-Skip-the-windowmanager-required-component-under.patch
Patch103:      0003-autostart-Skip-X11-only-entries-under-Caminito.patch
Patch104:      0004-autostart-Start-mate-power-manager-and-tilda-on-X11-.patch

BuildRequires: dbus-glib-devel
BuildRequires: desktop-file-utils
BuildRequires: gtk3-devel
BuildRequires: libSM-devel
BuildRequires: libXtst-devel
BuildRequires: make
BuildRequires: mate-common
BuildRequires: mesa-libGLES-devel
BuildRequires: systemd-devel
BuildRequires: xmlto
BuildRequires: xorg-x11-xtrans-devel

Requires: system-logos
# Needed for mate-settings-daemon
Requires: mate-control-center
# we need an authentication agent in the session
Requires: mate-polkit
# and we want good defaults
Requires: polkit-desktop-policy
# for gsettings shemas
Requires: mate-desktop-libs
# for /bin/dbus-launch
Requires: dbus-x11

%description
This package contains a session that can be started from a display
manager such as MDM. It will load all necessary applications for a
full-featured user session.

%prep
%autosetup -n %{name}-%{commit} -p1 -a 1
rm -rf mate-submodules
mv mate-submodules-%{submodules_commit} mate-submodules

NOCONFIGURE=1 ./autogen.sh

%build
%configure                    \
    --disable-static          \
    --enable-ipv6             \
    --with-default-wm=marco   \
    --with-systemd            \
    --enable-docbook-docs     \
    --disable-schemas-compile

make %{?_smp_mflags} V=1

%install
%{make_install}

desktop-file-install                               \
        --delete-original                          \
        --dir=%{buildroot}%{_datadir}/applications \
%{buildroot}%{_datadir}/applications/mate-session-properties.desktop

%find_lang %{name} --with-gnome --all-name


%files -f %{name}.lang
%doc AUTHORS COPYING README
%{_mandir}/man1/*
%{_bindir}/mate-session
%{_bindir}/mate-session-inhibit
%{_bindir}/mate-session-properties
%{_bindir}/mate-session-save
%{_bindir}/mate-wm
%{_libexecdir}/mate-session-check-accelerated
%{_libexecdir}/mate-session-check-accelerated-gl-helper
%{_libexecdir}/mate-session-check-accelerated-gles-helper
%{_datadir}/applications/mate-session-properties.desktop
%{_datadir}/mate-session-manager
%{_datadir}/icons/hicolor/*/apps/*.png
%{_datadir}/icons/hicolor/scalable/apps/mate-session-properties.svg
%{_datadir}/glib-2.0/schemas/org.mate.session.gschema.xml
%{_datadir}/xsessions/mate.desktop
%{_docdir}/mate-session-manager/dbus/mate-session.html


%changelog
* Thu Oct 15 2026 Juan Pablo Lassala <jpablolassala@gmail.com> - 1.28.0^20260721git57a9178-0.caminito1
- mate-session-manager's master at 57a9178, with Caminito's carried patches, from Fedora f45's spec (CMN-170)
