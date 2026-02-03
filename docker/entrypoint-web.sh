#!/bin/sh
set -e

if [ -t 0 ]; then
  echo "Interactive mode not required; run without -it (-i/--interactive, -t/--tty) or equivalent" >&2
  exit 1
fi

# shellcheck source=/dev/null
if ! . /usr/local/bin/entrypoint-user-setup.sh; then
  echo "Failed to setup user" >&2
  exit 1
fi

# Execute the command as the specified user
exec setpriv --reuid="${PUID}" --regid="${PGID}" --clear-groups "$@"
