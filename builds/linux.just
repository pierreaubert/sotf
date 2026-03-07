# ----------------------------------------------------------------------
# Install linux
# ----------------------------------------------------------------------

[group('install')]
install-linux-root:
	sudo apt update && sudo apt -y install \
	   perl curl build-essential gcc g++ pkg-config cmake ninja-build gfortran \
	   libssl-dev \
	   ca-certificates \
	   patchelf libopenblas-dev gfortran \
	   chromium-browser chromium-chromedriver

[group('install')]
install-linux: install-linux-root install-rustup

[group('install')]
install-ubuntu-common:
		sudo apt install -y \
			 curl \
			 build-essential gcc g++ \
			 pkg-config \
			 libssl-dev \
			 ca-certificates \
			 cmake \
			 ninja-build \
			 perl \
			 libglib2.0-dev \
			 libxkbcommon-x11-dev \
			 libgtk-3-dev \
			 libwebkit2gtk-4.1-dev \
			 libayatana-appindicator3-dev \
			 librsvg2-dev \
			 patchelf \
			 libopenblas-dev \
			 gfortran \
			 libasound2-dev \
			 libnetcdf-dev \
			 libopencv-dev \
			 libclang-dev \
			 webkit2gtk-driver \
			 libchacha2-dev

[group('install')]
install-ubuntu-x86-driver :
		sudo apt install -y \
			 chromium-browser \
			 chromium-chromedriver

[group('install')]
install-ubuntu-arm64-driver :
		sudo apt install -y firefox
		# where is the geckodriver ?

[group('install')]
install-ubuntu-x86: install-ubuntu-common install-ubuntu-x86-driver

[group('install')]
install-ubuntu-arm64: install-ubuntu-common install-ubuntu-arm64-driver

