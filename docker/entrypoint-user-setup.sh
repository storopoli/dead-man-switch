#!/bin/sh
set -e

PRINT_USER_CONFIG="${PRINT_USER_CONFIG:-false}"
USERNAME="${USERNAME:-dms}"
PUID="${PUID:-1000}"
PGID="${PGID:-1000}"
HOME="/home/${USERNAME}"

# Create group if it doesn't exist
if ! getent group "${PGID}" >/dev/null 2>&1; then
  groupadd -g "${PGID}" "${USERNAME}"
fi

# Create user if it doesn't exist
if ! id -u "${PUID}" >/dev/null 2>&1; then
  useradd -u "${PUID}" -g "${PGID}" -M -d "${HOME}" -s /usr/sbin/nologin "${USERNAME}"
fi

# Ensure home directory exists
mkdir -p "${HOME}"

# Ensure correct ownership - exclude well known files (that may be read-only bind mounted)
find "${HOME}" -xdev \( -type f -name config.toml \) -prune -o \
  -exec chown -h "${PUID}:${PGID}" {} +

# Optional: Print configuration details
if [ "${PRINT_USER_CONFIG}" = "true" ]; then
  cat << EOF
───────────────────────────────────────
GID/UID Configuration
───────────────────────────────────────
Username:   ${USERNAME}
User UID:   ${PUID}
User GID:   ${PGID}
Home Dir:   ${HOME}
───────────────────────────────────────
EOF
fi

export USERNAME PUID PGID HOME

return
