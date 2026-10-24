# The Fly.io machine Claude works Caminito's tickets in, through Remote Control: the sandbox of
# .sandcastle/Containerfile, with gh and tmux, and its state on the volume at /data. Keep the
# package list in step with that file's.
FROM docker.io/menci/archlinuxarm:base-devel

# pacman's download sandbox needs Landlock, which the container doesn't give it.
RUN sed -i '/^\[options\]/a DisableSandbox' /etc/pacman.conf

# .sandcastle/Containerfile's list, and github-cli and tmux.
RUN pacman -Syu --noconfirm --needed git curl jq \
    gcc pkgconf rustup cargo-deny libxkbcommon pixman wayland dbus libcanberra \
    pango gdk-pixbuf2 meson gtk3 gtk4 xorg-server-xvfb xorg-xauth xorg-xwayland seatd \
    systemd-libs mesa libdisplay-info libinput gtk-session-lock mate-desktop pam pam_wrapper \
    nodejs npm desktop-file-utils python-dbusmock libnm mate-panel libxfce4windowing gtk-layer-shell gvfs \
    github-cli tmux \
  && pacman -Scc --noconfirm

RUN useradd -m -d /home/agent agent
USER agent
RUN rustup toolchain install stable --profile minimal --component rustfmt,clippy
RUN curl -fsSL https://claude.ai/install.sh | bash
USER root

# What has to outlive a restart lives on the volume: Claude's login and settings, gh's, cargo's
# cache and the checkout with its target/. The shells of `fly ssh console` read the same environment
# from /etc/caminito.env, in case they don't get the image's.
ENV PATH="/home/agent/.local/bin:$PATH" \
    CLAUDE_CONFIG_DIR=/data/claude \
    GH_CONFIG_DIR=/data/gh \
    CARGO_HOME=/data/cargo
RUN printf 'export %s\n' 'PATH="/home/agent/.local/bin:$PATH"' CLAUDE_CONFIG_DIR=/data/claude \
      GH_CONFIG_DIR=/data/gh CARGO_HOME=/data/cargo > /etc/caminito.env \
  && echo '. /etc/caminito.env' | tee /etc/profile.d/caminito.sh >> /etc/bash.bashrc

COPY entrypoint /usr/local/bin/entrypoint
ENTRYPOINT ["/usr/local/bin/entrypoint"]
