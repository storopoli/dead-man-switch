# README

## WEB container development

```sh
# from the root directory of the repo
docker build . -f docker/web/Dockerfile -t local/dead_man_switch-web

# ensure container launches
docker run -p 3000:3000 \
  -e PUID=1002 \
  -e PGID=1002 \
  -e TZ=Europe/London \
  -v $(pwd)/my-config-dir:/config \
  --rm \
  --name dead_man_switch-web \
  local/dead_man_switch-web

# ^ run from the command line (and exit with ctrl-C)

##################################################
# ensure container fails if interactive terminal IS detected
docker run -it -p 3000:3000 \
  -e PUID=1002 \
  -e PGID=1002 \
  -e TZ=Europe/London \
  -v $(pwd)/my-config-dir:/config \
  --rm \
  --name dead_man_switch-web \
  local/dead_man_switch-web

# ERROR: Interactive terminal detected!
# Please run without: docker run -it (-i/--interactive, -t/--tty) ...
# (will attempt shutting down container in 3 seconds)
# s6-rc: warning: unable to start service terminal-check-non-interactive: command exited 1

##################################################
# ensure container fails if root user detected
docker run -p 3000:3000 \
  -e PUID=0 \
  -e PGID=0 \
  -e TZ=Europe/London \
  -v $(pwd)/my-config-dir:/config \
  --rm \
  --name dead_man_switch-web \
  local/dead_man_switch-web

# ERROR: PUID or PGID cannot be 0, for security reasons!
# Current values: PUID=0, PGID=0
# Please ensure both have non-root IDs (e.g., PUID=1000 PGID=1000)
# (will attempt shutting down container in 3 seconds)
# s6-rc: warning: unable to start service non-root-check: command exited 1
```
