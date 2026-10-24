# Fedora f45's mate-panel spec with the smallest diff, so a change there shows what to carry
# over. The differences: master's pinned commit, its snapshot's version and sources, autogen,
# libxfce4windowing, the carried patches, the 1.28 applet library built from Fedora's own
# 1.28.4 and installed beside master's, the caminito Release and the changelog (ADR 0011,
# CMN-170).
#
# The libxfce4windowing window list and workspace switcher are only on master (16). Master's
# configure.ac says 1.29.0, so the version is a snapshot before it, as on Debian, which is
# newer than Fedora's 1.28.4 (ADR 0029).
%global commit 58ac6c101bdf2e2975c0395723ad16e9c9764f5c
%global shortcommit %(c=%{commit}; echo ${c:0:7})
%global commitdate 20260925
# The mate-submodules commit that master's submodule points to. A GitHub archive leaves the
# submodule out.
%global submodules_commit dd9bd3436f2cb6410b04eefbd69306d93cd07b30
# Master's applet library is libmate-panel-applet-4.so.2, and Fedora's applets in other
# packages (mate-applets, mate-power-manager, mate-sensors-applet and more) are linked to the
# 1.28 branch's .so.1. An applet that is a program of its own talks to the panel over D-Bus,
# so the 1.28 library is built from the release Fedora's applets were built against, and
# installed beside master's (73). One the panel loads into its own process can't use it there.
%global stockver 1.28.4

Name:          mate-panel
Version:       1.29.0~%{commitdate}git%{shortcommit}
Release:       0.caminito2%{?dist}
Summary:       MATE Desktop panel and applets
#libs are LGPL-2.0-or-later
#applications GPL-2.0-or-later
License:       GPL-2.0-or-later
URL:           http://mate-desktop.org
Source0:       https://github.com/mate-desktop/%{name}/archive/%{commit}/%{name}-%{commit}.tar.gz
Source1:       mate-panel_fedora-30.layout
Source2:       mate-panel_rhel.layout
Source3:       mate-panel_fedora-40.layout
Source4:       https://github.com/mate-desktop/mate-submodules/archive/%{submodules_commit}/mate-submodules-%{submodules_commit}.tar.gz
Source5:       http://pub.mate-desktop.org/releases/1.28/%{name}-%{stockver}.tar.xz

# Caminito's carried patches, from packaging/patches/mate-panel/ (ADR 0030), which
# packaging/fedora/build copies beside this. Their gates and what each does are in
# packaging/arch/mate-panel-caminito/PKGBUILD.
Patch101:      0001-wayland-tasklist-Tell-the-compositor-where-each-butt.patch
Patch102:      0002-action-button-Shut-down-through-the-session-manager-.patch
Patch103:      0003-wayland-tasklist-Show-the-active-workspace-s-windows.patch
Patch104:      0004-multimonitor-Put-Caminito-s-primary-output-first-on-.patch
Patch105:      0005-wayland-workspace-Show-a-workspace-s-new-name-when-i.patch
Patch106:      0006-workspace-switcher-Set-the-workspace-count-and-names.patch
Patch107:      0007-workspace-switcher-Lay-the-workspaces-out-by-the-com.patch
Patch108:      0008-showdesktop-Show-Caminito-s-desktop-on-Wayland.patch
Patch109:      0009-main-Export-the-main-menu-and-the-run-dialog-on-D-Bu.patch
Patch110:      0010-panel-Let-an-object-held-by-a-pixel-position-move-le.patch
Patch111:      0011-properties-Choose-the-panel-s-display-under-Caminito.patch
Patch112:      0012-wayland-workspace-Show-the-workspaces-of-the-panel-s.patch
Patch113:      0013-wayland-workspace-Press-the-button-of-the-group-s-ac.patch
Patch114:      0014-wayland-workspace-Take-the-scroll-wheel-over-the-but.patch
Patch115:      0015-toplevel-Float-a-panel-off-its-edge-under-Caminito.patch

Requires:      %{name}-libs%{?_isa} = %{version}-%{release}
#for fish
Recommends:    fortune-mod
# rhbz (#1007219)
Requires:      caja-schemas

BuildRequires: desktop-file-utils
BuildRequires: gobject-introspection-devel
BuildRequires: gtk-doc
BuildRequires: gtk3-devel
BuildRequires: gtk-layer-shell-devel
BuildRequires: libmateweather-devel
BuildRequires: libwnck3-devel
BuildRequires: libSM-devel
BuildRequires: libxfce4windowing-devel
BuildRequires: make
BuildRequires: mate-common
BuildRequires: mate-desktop-devel
BuildRequires: mate-menus-devel
BuildRequires: wayland-devel
BuildRequires: yelp-tools

%description
MATE Desktop panel applets

%package libs
Summary:     Shared libraries for mate-panel
License:     LGPL-2.0-or-later
Requires:    %{name}%{?_isa} = %{version}-%{release}

%description libs
Shared libraries for libmate-desktop

%package devel
Summary:     Development files for mate-panel
Requires:    %{name}-libs%{?_isa} = %{version}-%{release}

%description devel
Development files for mate-panel

%prep
%autosetup -n %{name}-%{commit} -p1 -a 4
rm -rf mate-panel/mate-submodules
mv mate-submodules-%{submodules_commit} mate-panel/mate-submodules

NOCONFIGURE=1 ./autogen.sh

# Beside the source and not in it, where mate-autogen would regenerate it too.
tar -xf %{SOURCE5} -C %{builddir}

%build

#libexecdir needed for gnome conflicts
# A release tarball has the API reference built, which a snapshot hasn't: --enable-gtk-doc
# builds it, for mate-panel-devel.
%configure                                        \
           --disable-static                       \
           --disable-schemas-compile              \
           --libexecdir=%{_libexecdir}/mate-panel \
           --enable-introspection                 \
           --enable-gtk-doc                       \
           --enable-x11                           \
           --enable-wayland                       \
           --with-in-process-applets=all

# remove unused-direct-shlib-dependency
sed -i -e 's! -shared ! -Wl,--as-needed\0!g' libtool

# https://bugzilla.redhat.com/show_bug.cgi?id=2273561
# refresh translation message catalogs
# workaround to avoid missing strftime l10n entries.
(
  cd po
  make mate-panel.pot-update
  make update-po
  for f in mate-panel.pot *.po
  do
    sed -i '/^#, /d' $f
  done
  make update-gmo
)

make %{?_smp_mflags} V=1

# Only the applet library of the 1.28 release.
pushd %{builddir}/%{name}-%{stockver}
%configure --disable-static --disable-introspection
make %{?_smp_mflags} V=1 -C libmate-panel-applet libmate-panel-applet-4.la
popd


%install
%{make_install}

# The 1.28 library and its soname's link, and not the libmate-panel-applet-4.so that programs
# are built against, which stays master's.
cp -a %{builddir}/%{name}-%{stockver}/libmate-panel-applet/.libs/libmate-panel-applet-4.so.1 \
      %{builddir}/%{name}-%{stockver}/libmate-panel-applet/.libs/libmate-panel-applet-4.so.1.* \
      %{buildroot}%{_libdir}/

find %{buildroot} -name '*.la' -exec rm -rf {} ';'
find %{buildroot} -name '*.a' -exec rm -rf {} ';'

desktop-file-install \
        --dir=%{buildroot}%{_datadir}/applications \
%{buildroot}%{_datadir}/applications/mate-panel.desktop

%if 0%{?fedora} && 0%{?fedora} <= 39
install -D -m 0644 %{SOURCE1} %{buildroot}%{_datadir}/mate-panel/layouts/fedora.layout
%endif
%if 0%{?fedora} && 0%{?fedora} >= 40
install -D -m 0644 %{SOURCE3} %{buildroot}%{_datadir}/mate-panel/layouts/fedora.layout
%endif
%if 0%{?rhel}
install -D -m 0644 %{SOURCE2} %{buildroot}%{_datadir}/mate-panel/layouts/rhel.layout
%endif

%find_lang %{name} --with-gnome --all-name


%files -f %{name}.lang
%doc AUTHORS COPYING README
%{_mandir}/man1/*
%{_bindir}/mate-desktop-item-edit
%{_bindir}/mate-panel
%{_bindir}/mate-panel-test-applets
#%%{_libexecdir}/mate-panel
%{_datadir}/glib-2.0/schemas/org.mate.panel.*.xml
%{_datadir}/applications/mate-panel.desktop
%{_datadir}/icons/hicolor/*/*/*
%{_datadir}/mate-panel
#%%{_datadir}/dbus-1/services/org.mate.panel.applet.ClockAppletFactory.service
#%%{_datadir}/dbus-1/services/org.mate.panel.applet.FishAppletFactory.service
#%%{_datadir}/dbus-1/services/org.mate.panel.applet.NotificationAreaAppletFactory.service
#%%{_datadir}/dbus-1/services/org.mate.panel.applet.WnckletFactory.service
%{_libdir}/mate-panel

%files libs
%doc COPYING.LIB
%{_libdir}/libmate-panel-applet-4.so.1*
%{_libdir}/libmate-panel-applet-4.so.2*
%{_libdir}/girepository-1.0/MatePanelApplet-4.0.typelib

%files devel
%doc %{_datadir}/gtk-doc/html/mate-panel-applet/
%{_libdir}/libmate-panel-applet-4.so
%{_includedir}/mate-panel-4.0
%{_libdir}/pkgconfig/libmatepanelapplet-4.0.pc
%{_datadir}/gir-1.0/MatePanelApplet-4.0.gir


%changelog
* Sat Oct 24 2026 Juan Pablo Lassala <jpablolassala@gmail.com> - 1.29.0~20260925git58ac6c1-0.caminito2
- Carry 0015: a panel has a Floating property, which stands it off its edge, with rounded
  corners (CMN-223)

* Thu Oct 15 2026 Juan Pablo Lassala <jpablolassala@gmail.com> - 1.29.0~20260925git58ac6c1-0.caminito1
- mate-panel's master at 58ac6c1, with Caminito's carried patches, from Fedora f45's spec
  (CMN-170)
