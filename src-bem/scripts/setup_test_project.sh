#!/usr/bin/env bash
#
# Setup script for Mesh2HRTF test project
#
# This script downloads and sets up a real Mesh2HRTF project
# for testing the NumCalc FFI wrapper.
#
# Usage:
#   ./scripts/setup_test_project.sh [output_dir]
#
# If output_dir is not specified, uses /tmp/mesh2hrtf_test

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}ℹ${NC} $*"
}

success() {
    echo -e "${GREEN}✓${NC} $*"
}

warning() {
    echo -e "${YELLOW}⚠${NC} $*"
}

error() {
    echo -e "${RED}✗${NC} $*"
}

# Default output directory
OUTPUT_DIR="${1:-/tmp/mesh2hrtf_test}"

echo "╔════════════════════════════════════════════════════════╗"
echo "║     Mesh2HRTF Test Project Setup Script               ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""

info "Output directory: ${OUTPUT_DIR}"
echo ""

# Create output directory
if [ -d "${OUTPUT_DIR}" ]; then
    warning "Directory already exists: ${OUTPUT_DIR}"
    read -p "Remove and recreate? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "${OUTPUT_DIR}"
        success "Removed existing directory"
    else
        error "Aborted"
        exit 1
    fi
fi

mkdir -p "${OUTPUT_DIR}"
success "Created output directory"

# Clone Mesh2HRTF repository
info "Cloning Mesh2HRTF repository..."
MESH2HRTF_DIR="${OUTPUT_DIR}/Mesh2HRTF"

if ! git clone --depth 1 https://github.com/Any2HRTF/Mesh2HRTF.git "${MESH2HRTF_DIR}"; then
    error "Failed to clone Mesh2HRTF repository"
    exit 1
fi
success "Cloned Mesh2HRTF repository"

# Find example projects
info "Looking for example projects..."
EXAMPLE_PROJECTS=(
    "${MESH2HRTF_DIR}/mesh2hrtf/NumCalc/data/reference_hrtfs/KU100"
    "${MESH2HRTF_DIR}/mesh2hrtf/examples"
)

PROJECT_DIR=""
for dir in "${EXAMPLE_PROJECTS[@]}"; do
    if [ -d "${dir}" ] && [ -f "${dir}/NC.inp" ]; then
        PROJECT_DIR="${dir}"
        success "Found example project: ${PROJECT_DIR}"
        break
    fi
done

if [ -z "${PROJECT_DIR}" ]; then
    warning "No example project with NC.inp found"
    info "Searching for any NC.inp files..."

    # Search for any NC.inp files
    NC_INP_FILES=$(find "${MESH2HRTF_DIR}" -name "NC.inp" 2>/dev/null || true)

    if [ -n "${NC_INP_FILES}" ]; then
        echo "Found NC.inp files:"
        echo "${NC_INP_FILES}"

        # Use the first one found
        FIRST_NC_INP=$(echo "${NC_INP_FILES}" | head -n 1)
        PROJECT_DIR=$(dirname "${FIRST_NC_INP}")
        success "Using project: ${PROJECT_DIR}"
    else
        error "No NC.inp files found in repository"
        error "The repository structure may have changed"
        exit 1
    fi
fi

# Verify project structure
info "Verifying project structure..."

if [ ! -f "${PROJECT_DIR}/NC.inp" ]; then
    error "NC.inp not found in ${PROJECT_DIR}"
    exit 1
fi
success "NC.inp found"

# Check for mesh files
MESH_FILES=$(find "${PROJECT_DIR}" -name "*.msh" -o -name "*.nodes" -o -name "*.ele" 2>/dev/null || true)
if [ -n "${MESH_FILES}" ]; then
    success "Found mesh files"
else
    warning "No mesh files found (*.msh, *.nodes, *.ele)"
fi

# Create symbolic link for easy access
LINK_PATH="${OUTPUT_DIR}/test_project"
ln -sf "${PROJECT_DIR}" "${LINK_PATH}"
success "Created symbolic link: ${LINK_PATH} -> ${PROJECT_DIR}"

# Build NumCalc if source is available
info "Checking for NumCalc source..."
NUMCALC_SRC="${MESH2HRTF_DIR}/mesh2hrtf/NumCalc/src"

if [ -d "${NUMCALC_SRC}" ]; then
    success "Found NumCalc source directory"

    if [ -f "${NUMCALC_SRC}/Makefile" ]; then
        info "Building NumCalc..."

        pushd "${NUMCALC_SRC}" > /dev/null

        if make; then
            success "NumCalc built successfully"

            # Find the executable
            NUMCALC_EXE=$(find . -name "NumCalc" -type f -executable 2>/dev/null | head -n 1)

            if [ -n "${NUMCALC_EXE}" ]; then
                NUMCALC_PATH="$(cd "$(dirname "${NUMCALC_EXE}")" && pwd)/$(basename "${NUMCALC_EXE}")"
                success "NumCalc executable: ${NUMCALC_PATH}"
            fi
        else
            warning "NumCalc build failed (non-critical)"
        fi

        popd > /dev/null
    else
        warning "No Makefile found for NumCalc"
    fi
else
    warning "NumCalc source directory not found"
fi

# Print summary and instructions
echo ""
echo "╔════════════════════════════════════════════════════════╗"
echo "║                   Setup Complete!                      ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""

info "Test project location:"
echo "  ${PROJECT_DIR}"
echo ""

if [ -n "${NUMCALC_PATH:-}" ]; then
    info "NumCalc executable:"
    echo "  ${NUMCALC_PATH}"
    echo ""
fi

info "To run integration tests:"
echo ""
echo "  # Set environment variables"
echo "  export TEST_PROJECT_DIR=\"${LINK_PATH}\""
if [ -n "${NUMCALC_PATH:-}" ]; then
    echo "  export NUMCALC_PATH=\"${NUMCALC_PATH}\""
fi
echo ""
echo "  # Run tests"
echo "  cd $(dirname "$0")/.."
echo "  cargo test --test test_numcalc_integration --features ffi -- --ignored --nocapture"
echo ""

info "To run the demo:"
echo ""
echo "  # Set environment variables"
echo "  export TEST_PROJECT_DIR=\"${LINK_PATH}\""
if [ -n "${NUMCALC_PATH:-}" ]; then
    echo "  export NUMCALC_PATH=\"${NUMCALC_PATH}\""
fi
echo ""
echo "  # Run demo"
echo "  cd $(dirname "$0")/.."
echo "  cargo run --release --example numcalc_ffi_demo --features ffi"
echo ""

# Create an environment file for easy sourcing
ENV_FILE="${OUTPUT_DIR}/test_env.sh"
cat > "${ENV_FILE}" << EOF
#!/usr/bin/env bash
# Source this file to set up environment for NumCalc FFI testing
# Usage: source ${ENV_FILE}

export TEST_PROJECT_DIR="${LINK_PATH}"
EOF

if [ -n "${NUMCALC_PATH:-}" ]; then
    echo "export NUMCALC_PATH=\"${NUMCALC_PATH}\"" >> "${ENV_FILE}"
fi

echo "export PATH=\"\${PATH}:$(dirname "${NUMCALC_PATH:-}")\"" >> "${ENV_FILE}"

chmod +x "${ENV_FILE}"

success "Created environment file: ${ENV_FILE}"
echo ""
info "Quick start:"
echo "  source ${ENV_FILE}"
echo "  cd $(dirname "$0")/.."
echo "  cargo run --release --example numcalc_ffi_demo --features ffi"
echo ""

success "Setup complete!"
