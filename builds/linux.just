# ----------------------------------------------------------------------
# Install linux
# ----------------------------------------------------------------------

[group('install')]
install-linux-root:
	sudo apt update && sudo DEBIAN_FRONTEND=noninteractive apt -y install \
	   perl \
	   curl \
	   build-essential \
	   gcc g++ pkg-config cmake ninja-build \
	   libssl-dev \
	   ca-certificates \
	   patchelf

[group('install')]
install-linux: install-linux-root install-rustup

[group('install')]
install-ubuntu-common:
		sudo DEBIAN_FRONTEND=noninteractive apt install -y \
			 libglib2.0-dev \
			 libxkbcommon-x11-dev \
			 libgtk-3-dev \
			 libwebkit2gtk-4.1-dev \
			 libayatana-appindicator3-dev \
			 librsvg2-dev \
			 libasound2-dev \
			 libnetcdf-dev \
			 libopencv-dev \
			 libclang-dev
			 librust-alsa-dev \
			 librust-servo-fontconfig-dev \
			 python3-venv \
			 librust-libsqlite3-sys-dev \
			 librust-xkbcommon-dev

