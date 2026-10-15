# Fedora f45's mate-settings-daemon spec with the smallest diff, so a change there shows what
# to carry over. The differences: the 1.29.1 release, its tarball's address, Fedora's
# backports, which 1.29.1 has, out, what its Wayland code depends on, --enable-wayland, the
# carried patches, the caminito Release and the changelog (ADR 0011, CMN-170).
#
# The first tag with the Wayland background and clipboard. Its wlrandr plugin is off under
# Caminito, which keeps the saved layouts itself (0005, ADR 0014).
%global branch 1.29

# Needed to break circular dependencies between mate-settings-daemon(-devel)
# and mate-control-center(-filesystem), especially for new RHEL releases.
%global bootstrap 0

Name:          mate-settings-daemon
Version:       %{branch}.1
Release:       0.caminito1%{?dist}
Summary:       MATE Desktop settings daemon
# Automatically converted from old format: GPLv2+ - review is highly recommended.
License:       GPL-2.0-or-later
URL:           http://mate-desktop.org
Source0:       https://github.com/mate-desktop/%{name}/releases/download/v%{version}/%{name}-%{version}.tar.xz

# fix rhbz (#1140329)
%if 0%{?rhel}
Patch0:        mate-settings-daemon_fix-xrdb-plugin-for-rhel.patch
%endif

# Fedora's upstream commits 1 to 5 are in 1.29.1.

# Caminito's carried patches, from packaging/patches/mate-settings-daemon/ (ADR 0030), which
# packaging/fedora/build copies beside this. Their gates and what each does are in
# packaging/arch/mate-settings-daemon-caminito/PKGBUILD.
Patch101:      0001-xsettings-Mirror-to-the-GNOME-settings-only-under-Ca.patch
Patch102:      0002-background-Draw-under-Caja-s-desktop-under-Caminito.patch
Patch103:      0003-typing-break-Do-not-start-under-Caminito.patch
Patch104:      0005-wlrandr-Do-not-start-under-Caminito.patch
Patch105:      0006-background-Do-not-rebuild-the-windows-from-inside-a-.patch
Patch106:      0007-plugin-info-Start-an-X11Only-plugin-under-Caminito-w.patch
Patch107:      0008-osd-window-Show-on-the-overlay-layer-under-Caminito.patch
Patch108:      0009-input-helper-Take-a-touchpad-to-be-present-under-Cam.patch
Patch109:      0010-media-keys-Grab-the-keys-through-Caminito.patch
Patch110:      0011-keybindings-Grab-the-shortcuts-through-Caminito.patch
Patch111:      0012-background-Do-not-start-under-Caminito.patch
# Fedora's build patches, from the series' fedora/, after it (ADR 0030). The first takes out
# the one upstream commit that needs mate-desktop 1.29, which Fedora doesn't have. It goes
# when Fedora's mate-desktop is 1.29.
Patch201:      0004-Revert-xrandr-Handle-hotplug_mode_update-for-VM-gues.patch

BuildRequires: dconf-devel
BuildRequires: desktop-file-utils
BuildRequires: gtk3-devel
BuildRequires: gtk-layer-shell-devel
BuildRequires: libayatana-appindicator-gtk3-devel
BuildRequires: libmatemixer-devel
BuildRequires: libcanberra-devel
BuildRequires: libmatekbd-devel
BuildRequires: libnotify-devel
BuildRequires: libSM-devel
BuildRequires: make
BuildRequires: mate-common
BuildRequires: mate-desktop-devel
BuildRequires: nss-devel
BuildRequires: polkit-devel
BuildRequires: pulseaudio-libs-devel
BuildRequires: systemd-devel
BuildRequires: wayland-devel
BuildRequires: wayland-protocols-devel

Requires:       libmatekbd%{?_isa} >= 0:1.6.1-1
%if !0%{?bootstrap}
# needed for xrandr capplet
Requires:       mate-control-center-filesystem
%endif
# since f34
%if 0%{?fedora} && 0%{?fedora} >= 34
Requires:       xmodmap
Requires:       xrdb
%endif

%description
This package contains the daemon which is responsible for setting the
various parameters of a MATE session and the applications that run
under it.

%package devel
Summary:        Development files for mate-settings-daemon
Requires:       %{name}%{?_isa} = %{version}-%{release}

%description devel
This package contains the daemon which is responsible for setting the
various parameters of a MATE session and the applications that run
under it.

%prep
%autosetup -p1

NOCONFIGURE=1 ./autogen.sh

%build
# --enable-wayland makes a missing Wayland library an error. Without it, configure turns the
# wlrandr plugin and the Wayland background and clipboard off and says nothing.
%configure                             \
   --enable-pulse                      \
   --disable-static                    \
   --disable-schemas-compile           \
   --enable-polkit                     \
   --with-x                            \
   --with-nssdb                        \
   --enable-wayland

make %{?_smp_mflags} V=1

%install
%{make_install}

find %{buildroot} -name '*.la' -exec rm -rf {} ';'

desktop-file-validate %{buildroot}%{_sysconfdir}/xdg/autostart/mate-settings-daemon.desktop

%find_lang %{name} --with-gnome --all-name


%files -f %{name}.lang
%doc AUTHORS COPYING README
%dir %{_sysconfdir}/mate-settings-daemon
%dir %{_sysconfdir}/mate-settings-daemon/xrandr
%{_sysconfdir}/xdg/autostart/mate-settings-daemon.desktop
%{_sysconfdir}/xrdb/
%{_libdir}/mate-settings-daemon
%{_libexecdir}/mate-settings-daemon
%{_libexecdir}/msd-datetime-mechanism
%{_libexecdir}/msd-locate-pointer
%{_udevrulesdir}/61-mate-settings-daemon-rfkill.rules
%{_datadir}/mate-control-center/keybindings/50-accessibility.xml
%{_datadir}/dbus-1/services/org.mate.SettingsDaemon.service
%{_datadir}/dbus-1/system-services/org.mate.SettingsDaemon.DateTimeMechanism.service
%{_datadir}/dbus-1/system.d/org.mate.SettingsDaemon.DateTimeMechanism.conf
%{_datadir}/icons/hicolor/*/*/*
%{_datadir}/mate-settings-daemon
%{_datadir}/glib-2.0/schemas/org.mate.*.xml
%{_datadir}/polkit-1/actions/org.mate.settingsdaemon.datetimemechanism.policy
%{_mandir}/man1/*

%files devel
%{_includedir}/mate-settings-daemon
%{_libdir}/pkgconfig/mate-settings-daemon.pc


%changelog
* Thu Oct 15 2026 Juan Pablo Lassala <jpablolassala@gmail.com> - 1.29.1-0.caminito1
- mate-settings-daemon 1.29.1, with Caminito's carried patches, from Fedora f45's spec
  (CMN-170)
