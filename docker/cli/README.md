# README

## CLI container development

```sh
# from the root directory of the repo
docker build . -f docker/cli/Dockerfile -t local/dead_man_switch-cli

# ensure container launches
docker run -it \
  -e PUID=1002 \
  -e PGID=1002 \
  -e TZ=Europe/London \
  -v $(pwd)/my-config-dir:/config \
  --rm \
  --name dead_man_switch-cli \
  local/dead_man_switch-cli

# ^ run from the command line (and exit with q/Esc)

##################################################
# ensure container fails if interactive terminal IS NOT detected
docker run \
  -e PUID=1002 \
  -e PGID=1002 \
  -e TZ=Europe/London \
  -v $(pwd)/my-config-dir:/config \
  --rm \
  --name dead_man_switch-cli \
  local/dead_man_switch-cli

# ERROR: No interactive terminal detected!
# Please run with: docker run -it (-i/--interactive, -t/--tty) ...
# (will attempt shutting down container in 3 seconds)
# s6-rc: warning: unable to start service terminal-check-interactive: command exited 1

##################################################
# ensure container fails if root user detected
docker run \
  -e PUID=0 \
  -e PGID=0 \
  -e TZ=Europe/London \
  -v $(pwd)/my-config-dir:/config \
  --rm \
  --name dead_man_switch-cli \
  local/dead_man_switch-cli

# ERROR: PUID or PGID cannot be 0, for security reasons!
# Current values: PUID=0, PGID=0
# Please ensure both have non-root IDs (e.g., PUID=1000 PGID=1000)
# (will attempt shutting down container in 3 seconds)
# s6-rc: warning: unable to start service non-root-check: command exited 1
```
