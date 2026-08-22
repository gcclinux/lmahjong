#!/usr/bin/env bash
#
# Build (debug or release) and run xMahjong on macOS.
#
# Prerequisites:
#   brew install sdl2 sdl2_image sdl2_mixer sdl2_ttf
#
# Usage:
#   ./run_macos.sh                    # debug build, normal run
#   ./run_macos.sh --release          # release build
#   ./run_macos.sh --dev --level 29   # dev mode, start at level 29
#   ./run_macos.sh --dev --level 50   # dev mode, start at level 50

set -e

RELEASE=0
DEV=0
LEVEL=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release|-r)
            RELEASE=1
            shift
            ;;
        --dev|-d)
            DEV=1
            shift
            ;;
        --level|-l)
            LEVEL="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: ./run_macos.sh [--release] [--dev] [--level N]"
            exit 1
            ;;
    esac
done

# Ensure SDL2 libraries are available via Homebrew
if ! command -v brew &>/dev/null; then
    echo -e "\033[31mError: Homebrew is required. Install from https://brew.sh\033[0m"
    exit 1
fi

MISSING=()
for pkg in sdl2 sdl2_image sdl2_mixer sdl2_ttf; do
    if ! brew list "$pkg" &>/dev/null; then
        MISSING+=("$pkg")
    fi
done

if [[ ${#MISSING[@]} -gt 0 ]]; then
    echo -e "\033[33mInstalling missing SDL2 dependencies: ${MISSING[*]}\033[0m"
    brew install "${MISSING[@]}"
fi

# Set library search paths for the linker (Homebrew on Apple Silicon vs Intel)
BREW_PREFIX="$(brew --prefix)"
export LIBRARY_PATH="${BREW_PREFIX}/lib:${LIBRARY_PATH:-}"
export CPATH="${BREW_PREFIX}/include:${CPATH:-}"

# Build
if [[ $RELEASE -eq 1 ]]; then
    echo -e "\033[36mBuilding (release)...\033[0m"
    cargo build --release
    PROFILE="release"
else
    echo -e "\033[36mBuilding (debug)...\033[0m"
    cargo build
    PROFILE="debug"
fi

# Run
EXE="target/$PROFILE/xmahjong"

RUN_ARGS=()
if [[ $DEV -eq 1 ]]; then
    RUN_ARGS+=("--dev")
    if [[ $LEVEL -gt 0 ]]; then
        RUN_ARGS+=("--level" "$LEVEL")
    fi
    echo -e "\033[33mRunning (DEV mode, level $LEVEL): $EXE ${RUN_ARGS[*]}\033[0m"
else
    echo -e "\033[32mRunning: $EXE\033[0m"
fi

exec "$EXE" "${RUN_ARGS[@]}"
