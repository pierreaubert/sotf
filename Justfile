# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------

default:
	just --list

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

download-once:
	cargo run --bin download

test-generate-audio-tests: prod-generate-audio-tests
	cargo run --bin prod-generate-audio-tests --release

test-rust:
	cargo check --all-targets
	cargo test --lib

test-ts:
	npm run test

test: test-rust test-ts

# ----------------------------------------------------------------------
# FORMAT
# ----------------------------------------------------------------------

alias format := fmt

fmt: fmt-rust fmt-ts

fmt-rust:
	cargo fmt --all

fmt-ts:
	npm run fmt

# ----------------------------------------------------------------------
# PROD
# ----------------------------------------------------------------------

alias build := prod

prod: prod-workspace prod-autoeq prod-sotf-player prod-sotf-recorder prod-generate-audio-tests prod-roomeq
	cargo build --release --bin plot_functions
	cargo build --release --bin download
	cargo build --release --bin benchmark_autoeq_speaker
	cargo build --release --bin benchmark_convergence
	cargo build --release --bin plot_autoeq_de
	cargo build --release --bin run_autoeq_de

prod-generate-audio-tests:
	cargo build --release --bin generate_audio_tests

prod-workspace:
	cargo build --release --workspace

prod-autoeq:
	cargo build --release --bin autoeq

prod-roomeq:
	cargo build --release --bin roomeq

prod-sotf-player:
	cargo build --release --bin sotf_player

prod-sotf-recorder:
	cargo build --release --bin sotf_recorder

# ----------------------------------------------------------------------
# BENCH
# ----------------------------------------------------------------------

bench: bench-convergence bench-autoeq-speaker

bench-convergence:
	cargo run --release --bin benchmark_convergence

bench-autoeq-speaker:
	# either jobs=1 or --no-parallel ; or a mix if you have a lot of
	# CPU cores
	cargo run --release --bin benchmark_autoeq_speaker -- --qa --jobs 1

# ----------------------------------------------------------------------
# CLEAN
# ----------------------------------------------------------------------

clean:
	cargo clean
	rm -rf src-*/dist
	rm -rf src-*/node_modules
	find . -name '*~' -exec rm {} \; -print
	find . -name 'Cargo.lock' -exec rm {} \; -print
	find . -name 'package-lock.json' -exec rm {} \; -print

# ----------------------------------------------------------------------
# DEV
# ----------------------------------------------------------------------

dev:
	cargo build --workspace
	cargo build --bin autoeq
	cargo build --bin plot_functions
	cargo build --bin download
	cargo build --bin benchmark_convergence
	cargo build --bin benchmark_autoeq_speaker
	cargo build --bin plot_autoeq_de
	cargo build --bin run_autoeq_de
	cargo build --bin sotf_audio_test

# ----------------------------------------------------------------------
# UPDATE
# ----------------------------------------------------------------------

update: update-rust update-pre-commit update-ts

update-rust:
	rustup update
	cargo update

update-pre-commit:
	pre-commit autoupdate

update-ts:
	npm run tauri update
	npm run upgrade

# ----------------------------------------------------------------------
# DEMO
# ----------------------------------------------------------------------

demo: headphone_loss_demo plot_functions

headphone_loss_demo:
	cargo run --release --example headphone_loss_demo -- \
	--spl "./data_tests/headphones/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" \
	--target "./data_tests/targets/harman-over-ear-2018.csv"

plot_functions:
	cargo run --release --bin plot_functions

# ----------------------------------------------------------------------
# EXAMPLES
# ----------------------------------------------------------------------

examples : examples-iir examples-de examples-autoeq examples-testfunctions

examples-iir :
	cargo run --release --example format_demo
	cargo run --release --example readme_example

examples-de :
	cargo run --release --example optde_basic
	cargo run --release --example optde_adaptive_demo
	cargo run --release --example optde_linear_constraints
	cargo run --release --example optde_nonlinear_constraints
	cargo run --release --example optde_parallel

examples-autoeq:
	cargo run --release --example headphone_loss_validation

examples-testfunctions:
	cargo run --release --example test_hartman_4d

# ----------------------------------------------------------------------
# CROSS
# ----------------------------------------------------------------------

cross : cross-macos-arm-2-linux-x86

# Debug: Build Docker image and open interactive shell
cross-debug-x86 :
	@echo "Building Docker image..."
	docker build -t autoeq-linux-x86 -f ./builds/Dockerfile.x86_64-unknown-linux-gnu .
	@echo "Starting interactive shell. Try: cargo build --release --target x86_64-unknown-linux-gnu"
	docker run -it --rm -v "$(pwd)":/project -w /project autoeq-linux-x86 /bin/bash

cross-macos-arm-2-linux-x86 :
	echo "This can take minutes!"
	@echo "Building Docker image..."
	docker build -t autoeq-linux-x86 -f ./builds/Dockerfile.x86_64-unknown-linux-gnu .
	@echo "Building in Docker container..."
	docker run --rm -v "$(pwd)":/project -w /project autoeq-linux-x86 \
		cargo build --release --target x86_64-unknown-linux-gnu
	@echo "Done! Binary at: target/x86_64-unknown-linux-gnu/release/autoeq"

cross-macos-arm-2-linux-arm64 :
	echo "This can take minutes!"
	cross build --release --target aarch64-unknown-linux-gnu

cross-macos-arm-2-win-x86-gnu :
	echo "This is not supported yet"
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-gnu

cross-macos-arm-2-win-x86-msvc :
	echo "This can take minutes!"
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-msvc

cross-macos-arm-2-win-arm-gnu :
	echo "This is not supported!"

cross-macos-arm-2-win-arm-msvc :
	echo "This is not supported!"


# ----------------------------------------------------------------------
# Install rustup
# ----------------------------------------------------------------------

install-rustup:
	curl https://sh.rustup.rs -sSf > ./scripts/install-rustup
	chmod +x ./scripts/install-rustup
	./scripts/install-rustup -y
	source ~/.cargo/env
	cargo install just
	cargo install cargo-wizard

# ----------------------------------------------------------------------
# Install macos
# ----------------------------------------------------------------------

install-macos-cross:
	# use git version until 0.2.6 is out
	cargo install cross --git https://github.com/cross-rs/cross
	cross target add x86_64-apple-ios

install-macos-brew:
	curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh > ./scripts/install-brew
	chmod +x ./scripts/install-brew
	NONINTERACTIVE=1 ./scripts/install-brew

install-macos: install-macos-brew install-rustup
	# need xcode
	xcode-select --install
	# need metal
	xcodebuild -downloadComponent MetalToolchain
	# chromedriver sheanigans
	brew install chromedriver
	xattr -d com.apple.quarantine $(which chromedriver)
	# optimisation library
	brew install nlopt cmake


# ----------------------------------------------------------------------
# Install linux
# ----------------------------------------------------------------------

install-linux-root:
	sudo apt update && sudo apt -y install \
	   perl curl build-essential gcc g++ pkg-config cmake ninja-build gfortran \
	   libssl-dev \
	   ca-certificates \
	   patchelf libopenblas-dev gfortran \
	   chromium-browser chromium-chromedriver

install-linux: install-linux-root install-rustup

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
			 rustup \
			 just \
			 libglib2.0-dev \
			 libgtk-3-dev \
			 libwebkit2gtk-4.1-dev \
			 libayatana-appindicator3-dev \
			 librsvg2-dev \
			 patchelf \
			 libopenblas-dev \
			 gfortran \
			 libasound2-dev

install-ubuntu-x86-driver :
		sudo apt install -y \
			 chromium-browser \
			 chromium-chromedriver

install-ubuntu-arm64-driver :
		sudo apt install -y firefox
		# where is the geckodriver ?

install-ubuntu-node:
		# use nvm
		curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
		NVM_DIR=$HOME/.nvm && nvm install stable

install-ubuntu-x86: install-ubuntu-common install-ubuntu-x86-driver install-ubuntu-node

install-ubuntu-arm64: install-ubuntu-common install-ubuntu-arm64-driver install-ubuntu-node


# ----------------------------------------------------------------------
# publish
# ----------------------------------------------------------------------

publish:
	cd src-testfunctions && cargo publish
	cd src-de && cargo publish
	cd src-cea2034 && cargo publish
	cd src-autoeq && cargo publish

# ----------------------------------------------------------------------
# QA
# ----------------------------------------------------------------------

qa: prod-autoeq \
	qa-ascilab-6b \
	qa-jbl-m2-flat qa-jbl-m2-score \
	qa-beyerdynamic-dt1990pro \
	qa-edifierw830nb

qa-ascilab-6b:
	./target/release/autoeq --speaker="AsciLab F6B" --version asr --measurement CEA2034 \
	--algo autoeq:de --loss speaker-score -n 7 --min-freq=30 --max-q=6 \
	--qa 0.5

qa-jbl-m2-flat:
	./target/release/autoeq --speaker="JBL M2" --version eac --measurement CEA2034 \
	--algo autoeq:de --loss speaker-flat -n 7 --min-freq=20 --max-q=6 --peq-model hp-pk \
	--qa 0.5

qa-jbl-m2-score:
	./target/release/autoeq --speaker="JBL M2" --version eac --measurement CEA2034 \
	--algo autoeq:de --loss speaker-score -n 7 --min-freq=20 --max-q=6 --peq-model hp-pk \
	--qa 0.5

qa-beyerdynamic-dt1990pro: qa-beyerdynamic-dt1990pro-flat qa-beyerdynamic-dt1990pro-score	qa-beyerdynamic-dt1990pro-score2

qa-beyerdynamic-dt1990pro-score:
	./target/release/autoeq -n 5 \
	--curve ./data_tests/headphones/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv --loss headphone-score  \
	--qa 3.0

qa-beyerdynamic-dt1990pro-score2:
	./target/release/autoeq -n 7 \
	--curve ./data_tests/headphones/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--loss headphone-score	--max-db 6 --max-q 6 --algo mh:rga --maxeval 20000 --min-freq=20 --max-freq 10000 --peq-model hp-pk-lp --min-q 0.6 --min-db 0.25 \
	--qa 1.5

qa-beyerdynamic-dt1990pro-flat:
	./target/release/autoeq -n 5 \
	--curve ./data_tests/headphones/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--loss headphone-flat  --max-db 6 --max-q 6 --maxeval 20000 --algo mh:pso --min-freq=20 --max-freq 10000 --peq-model pk \
	--qa 0.5

qa-edifierw830nb: qa-edifierw830nb-autoeqde qa-edifierw830nb-mhrga qa-edifierw830nb-mhfirefly

qa-edifierw830nb-autoeqde:
	./target/release/autoeq -n 9 \
	--curve data_tests/headphones/asr/edifierw830nb/Edifier\ W830NB.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--min-freq 50 --max-freq 16000 --max-q 8 --max-db 8 \
	--loss headphone-score --min-spacing-oct 0.08 \
	--algo autoeq:de --population 70 --maxeval 8000 --seed 42 \
	--qa 14.0

qa-edifierw830nb-mhrga:
	./target/release/autoeq -n 5 \
	--curve data_tests/headphones/asr/edifierw830nb/Edifier\ W830NB.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--min-freq 50 --max-freq 16000 --max-q 8 --max-db 8 \
	--loss headphone-score \
	--min-spacing-oct 0.08 --atolerance 0.000001 --tolerance 0.0000001 --algo mh:rga --population 100 --maxeval 20000 \
	--qa 4.0

qa-edifierw830nb-mhfirefly:
	./target/release/autoeq -n 5 \
	--curve data_tests/headphones/asr/edifierw830nb/Edifier\ W830NB.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--min-freq 50 --max-freq 16000 --max-q 8 --max-db 8 \
	--loss headphone-score \
	--min-spacing-oct 0.08 --atolerance 0.000001 --tolerance 0.0000001 --algo mh:rga --population 80 --maxeval 3000 \
	--qa 4.0

# ----------------------------------------------------------------------
# POST
# ----------------------------------------------------------------------

post-install-npm:
	cd src-ui-frontend && npm install .

post-install-rust:
	rustup default stable
	cargo install just
	cargo install tauri-cli
	cargo check
	cd src-tauri && cargo tauri icon

post-install: post-install-rust post-install-npm

# ----------------------------------------------------------------------
# SIGNING
# ----------------------------------------------------------------------



# ----------------------------------------------------------------------
# Install windows
# ----------------------------------------------------------------------

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

install-windows-vcpkg:
	# Use the current user's profile to avoid hard-coded usernames
	$dest = Join-Path $env:USERPROFILE 'source\repos\microsoft'
	# Find git on PATH or in common install locations; otherwise allow a zip fallback
	$gitCmd = (Get-Command git -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue)
	if (-not $gitCmd) {
	$possible = @(
	Join-Path $env:ProgramFiles 'Git\\cmd\\git.exe',
	Join-Path $env:'ProgramFiles(x86)' 'Git\\cmd\\git.exe',
	Join-Path $env:ProgramFiles 'Git\\bin\\git.exe'
	)
	foreach ($p in $possible) {
	if ($p -and (Test-Path $p)) { $gitCmd = $p; break }
	}
	}
	if ($gitCmd) { Write-Host "Using git at: $gitCmd" } else { Write-Host 'git not found on PATH; will use zip fallback to download vcpkg'; $useZip = $true }
	if (-not (Test-Path $dest)) { New-Item -ItemType Directory -Force -Path $dest | Out-Null }
	Set-Location $dest
	if (-not (Test-Path vcpkg)) {
	if (-not $useZip) {
	Write-Host 'Cloning vcpkg into' $dest
	& $gitCmd clone https://github.com/microsoft/vcpkg.git
	} else {
	Write-Host 'Downloading vcpkg zip fallback into' $dest
	$tmpZip = Join-Path $env:TEMP 'vcpkg-master.zip'
	try {
	Invoke-WebRequest -Uri 'https://github.com/microsoft/vcpkg/archive/refs/heads/master.zip' -OutFile $tmpZip -UseBasicParsing -ErrorAction Stop
	Expand-Archive -LiteralPath $tmpZip -DestinationPath $dest -Force
	if (Test-Path (Join-Path $dest 'vcpkg-master')) { Rename-Item (Join-Path $dest 'vcpkg-master') -NewName 'vcpkg' }
	} catch { Write-Error "Failed to download or extract vcpkg zip: $_"; exit 1 }
	}
	} else { Write-Host 'vcpkg already present, skipping clone' }
	Set-Location vcpkg
	if (-not (Test-Path .\vcpkg.exe)) {
	Write-Host 'Bootstrapping vcpkg...'
	.\bootstrap-vcpkg.bat
	} else {
	Write-Host 'vcpkg.exe already exists, skipping bootstrap'
	}
	# Install libraries for common MSVC triplets. Adjust triplets as needed.
	$triplets = @('x64-windows','x64-windows-static')
	foreach ($t in $triplets) {
	Write-Host "Installing nlopt and openblas for triplet: $t"
	.\vcpkg.exe install nlopt:$t openblas:$t
	}
	.\vcpkg.exe integrate install
	Write-Host 'vcpkg setup complete. vcpkg path:' (Get-Location)

install-windows-node:
	# Install Node.js LTS in an automated, idempotent way.
	if (Get-Command node -ErrorAction SilentlyContinue) {
	Write-Host "Node is already installed:" (node -v)
	exit 0
	}
	Write-Host 'Node not found. Attempting automated install (winget -> choco -> MSI)'

	# Try winget first (Windows 10/11)
	if (Get-Command winget -ErrorAction SilentlyContinue) {
	Write-Host 'Attempting winget install of Node.js LTS'
	winget install --id OpenJS.NodeJS.LTS -e --silent
	if (Get-Command node -ErrorAction SilentlyContinue) { Write-Host 'Node installed via winget:' (node -v); exit 0 }
	Write-Host 'winget install did not succeed or node still not found. Falling back.'
	} else {
	Write-Host 'winget not present, skipping.'
	}

	# Try Chocolatey if present
	if (Get-Command choco -ErrorAction SilentlyContinue) {
	Write-Host 'Attempting Chocolatey install of nodejs-lts'
	choco install -y nodejs-lts
	if (Get-Command node -ErrorAction SilentlyContinue) { Write-Host 'Node installed via Chocolatey:' (node -v); exit 0 }
	Write-Host 'Chocolatey install did not succeed or node still not found. Falling back.'
	} else {
	Write-Host 'Chocolatey not present, skipping.'
	}

	# Fallback: download Node.js LTS MSI and install via msiexec
	$tmp = Join-Path $env:TEMP 'node-lts.msi'
	Write-Host 'Downloading Node.js LTS MSI to' $tmp
	$uri = 'https://nodejs.org/dist/latest-v18.x/node-v18.20.1-x64.msi'
	# Note: pin or update the URI if you want a different LTS
	try {
	Invoke-WebRequest -Uri $uri -OutFile $tmp -UseBasicParsing -ErrorAction Stop
	} catch {
	Write-Error "Failed to download Node MSI from $uri : $_"; exit 1
	}
	Write-Host 'Running MSI installer (requires elevation)'
	$msiArgs = "/i `"$tmp`" /qn /norestart"
	$proc = Start-Process msiexec.exe -ArgumentList $msiArgs -Wait -PassThru
	if ($proc.ExitCode -ne 0) { Write-Error "msiexec failed with exit code $($proc.ExitCode)"; exit 1 }
	if (Get-Command node -ErrorAction SilentlyContinue) { Write-Host 'Node installed via MSI:' (node -v); exit 0 }
	Write-Error 'Node installation failed. Please install manually from https://nodejs.org/en/download'

install-windows-llvm-arm:
	# Install LLVM/Clang for Windows (ARM/WOA) in an automated, idempotent way.
	# This tries winget -> choco -> GitHub release MSI fallback.
	if (Get-Command clang -ErrorAction SilentlyContinue) {
	Write-Host "clang/LLVM already installed:"; clang --version; exit 0
	}
	Write-Host 'clang not found. Attempting automated install (winget -> choco -> MSI)'

	# Try winget first
	if (Get-Command winget -ErrorAction SilentlyContinue) {
	Write-Host 'Attempting winget install of LLVM'
	winget install --id LLVM.LLVM -e --silent
	if (Get-Command clang -ErrorAction SilentlyContinue) { Write-Host 'LLVM installed via winget'; clang --version; exit 0 }
	Write-Host 'winget install did not succeed or clang still not found. Falling back.'
	} else {
	Write-Host 'winget not present, skipping.'
	}

	# Try Chocolatey
	if (Get-Command choco -ErrorAction SilentlyContinue) {
	Write-Host 'Attempting Chocolatey install of llvm'
	choco install -y llvm
	if (Get-Command clang -ErrorAction SilentlyContinue) { Write-Host 'LLVM installed via Chocolatey'; clang --version; exit 0 }
	Write-Host 'Chocolatey install did not succeed or clang still not found. Falling back.'
	} else {
	Write-Host 'Chocolatey not present, skipping.'
	}

	# Fallback: download LLVM installer from GitHub releases. We'll pick the latest stable release for x64 host
	# but target the Windows ARM/WOA components if available. Note: manual verification may be needed for WOA.
	$tmp = Join-Path $env:TEMP 'LLVM-installer.exe'
	$releaseUrl = 'https://github.com/llvm/llvm-project/releases/latest'
	Write-Host 'Attempting to download LLVM installer from GitHub releases (may require manual selection for WOA)'
	try {
	# The GitHub latest page redirects to the latest release; try to download a common Windows installer filename.
	# This is a best-effort fallback â€” for Windows-on-ARM-specific builds you may need the vendor-provided WOA installer.
	$candidates = @(
	'https://github.com/llvm/llvm-project/releases/download/llvmorg-18.0.6/LLVM-18.0.6-win64.exe',
	'https://github.com/llvm/llvm-project/releases/download/llvmorg-17.0.6/LLVM-17.0.6-win64.exe'
	)
	$downloaded = $false
	foreach ($u in $candidates) {
	try {
	Invoke-WebRequest -Uri $u -OutFile $tmp -UseBasicParsing -ErrorAction Stop
	$downloaded = $true; break
	} catch { Write-Host "Failed to download $u : $_" }
	}
	if (-not $downloaded) { Write-Error 'Could not download an LLVM installer fallback. Please install manually from https://releases.llvm.org/ or vendor WOA pages.'; exit 1 }
	} catch {
	Write-Error "Failed to download LLVM installer: $_"; exit 1
	}
	Write-Host 'Running LLVM installer (requires elevation)'
	$args = '/S'  # many LLVM installers support /S for silent; adjust if needed
	$proc = Start-Process -FilePath $tmp -ArgumentList $args -Wait -PassThru
	if ($proc.ExitCode -ne 0) { Write-Error "Installer failed with exit code $($proc.ExitCode)"; exit 1 }
	if (Get-Command clang -ErrorAction SilentlyContinue) { Write-Host 'LLVM installed via installer'; clang --version; exit 0 }
	Write-Error 'LLVM installation failed or clang not on PATH. Please install manually: https://releases.llvm.org/ or https://learn.arm.com/install-guides/llvm-woa/'

install-windows-arm: install-windows-vcpkg install-windows-node install-windows-llvm-arm
	rustup default stable
	rustup target add aarch64-pc-windows-msvc

install-windows-x86: install-windows-vcpkg install-windows-node
	rustup default stable
	rustup target add x86_64-pc-windows-msvc

