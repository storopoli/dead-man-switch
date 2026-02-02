# Dead Man's Switch

[![AGPL-v3](https://img.shields.io/badge/License-AGPL&nbsp;v3-lightgrey.svg)](https://opensource.org/license/agpl-v3/)
[![Crates.io](https://img.shields.io/crates/v/dead-man-switch)](https://crates.io/crates/dead-man-switch)
[![docs](https://img.shields.io/crates/v/dead-man-switch?color=yellow&label=docs)](https://docs.rs/dead-man-switch)
[![startos](https://img.shields.io/badge/startos-blue)](https://github.com/storopoli/dead-man-switch-startos)

This is a simple implementation of a
[Dead Man's Switch](https://en.wikipedia.org/wiki/Dead_man%27s_switch).

Use at your own risk.
Check the f****(as in friendly) code.

![screenshot](https://github.com/storopoli/dead-man-switch/raw/main/screenshot.png)

Dead man's switches are designed to require positive action
or they will automatically deploy.
They are ideal for situations where you are worried about unforeseen death,
kidnapping, or memory loss.
If you don’t engage the trigger for a certain amount of time,
the switch automatically sends the desired message.

## Features

- **Simple**: Easy to use and setup.
- **Reliable**: Implemented in Rust.
- **Minimal**: Very few dependencies and needs minimal resources.
- **Warning**: Sends a warning email before the final email.
- **Attachments** (Optional): Send attachments with the final email.

## How it Works

> If you want a very simple explanation and the motivation behind the project,
> check my blog post [here](https://storopoli.com/posts/2024-03-23-dead-man-switch.html).

Upon starting the program it will create a [`config.toml`](config.example.toml)
file in an OS-agnostic config file location:

- Linux: `$XDG_CONFIG_HOME`, i.e. `$HOME/.config`, `/home/alice/.config`
- macOS: `$HOME/Library/Application Support`, i.e. `/Users/Alice/Library/Application Support`
- Windows: `{FOLDERID_RoamingAppData}`, i.e. `C:\Users\Alice\AppData\Roaming`

Edit the `config.toml` file to your liking.
Some default values are provided for inspiration.

Dead Man's Switch comprises of two timers:

1. **Warning Timer**: This timer is set to the `timer_warning` (seconds) value
   in the `config.toml` file.
   If the user do not check-in before timer reaches 0,
   it will send a warning email to the users' own specified email address,
   the `from` in the `config.toml`.
1. **Dead Man's Timer**: After the warning timer expires, the timer will change
   to a Dead Man's timer, and the timer will be set to the `timer_dead_man` (seconds).
   If the user do not check-in before timer reaches 0,
   it will send the final email to the specified email address in the `config.toml`,
   i.e. the `to` in the `config.toml`.

If you want to send attachments with the Dead Man's email,
you can specify the `attachments` option config in the `config.toml`
and provide the _absolute_ path to the file you want to attach.

To check-in, you just need to press the `c` key as in **c**heck-in.

## Installation

There are several ways to install Dead Man's Switch:

1. [Crates.io](https://crates.io/crates/dead-man-switch): `cargo install --locked dead-man-switch-tui`.
1. [GitHub](https://github.com/storopoli/dead-man-switch): `cargo install --git --locked https://github.com/storopoli/dead-man-switch -p dead-man-switch-tui`.
1. From source: Clone the repository and run `cargo install --locked --path .`.
1. Using Nix: `nix run github:storopoli/dead-man-switch`.
1. Using Nix Flakes: add this to your `flake.nix`:

   ```nix
   {
     # ...
     inputs.dead-man-switch = {
       url = "github:storopoli/dead-man-switch";
       inputs = {
         nixpkgs.follows = "nixpkgs";
         flake-parts.follows = "flake-parts";
       };
     };

     outputs = inputs @ { self, ... }:
     {
       imports = [
         {
           nixpkgs.overlays = [
             # ...
             inputs.dead-man-switch.overlays.default
           ];
         }
       ];
     };

   }
   ```

   Then `dead-man-switch` will be available as `pkgs.dead-man-switch`;

## Using as a Library

Dead Man's Switch can be used as a library.
This includes all the functions necessary to configure and send emails;
along with the timers.

To do so you can add the following to your `Cargo.toml`:

```toml
[dependencies]
dead-man-switch = "0.9.0"
```

## Web Interface

The Dead Man's Switch is also available as a web interface.

![web interface](https://github.com/storopoli/dead-man-switch/raw/main/web-interface.png)

To use the web interface, please follow the instructions below:

1. Change the configuration template file with your own values:

   ```bash
   cp config.example.toml config.toml
   ```

1. Copy the Docker Compose example file:

   ```bash
   cp docker-compose.example.yml docker-compose.yml
   ```

1. Run the Docker Compose:

   ```bash
   docker-compose up --detach
   ```

1. Make sure to [reverse proxy](https://docs.nginx.com/nginx/admin-guide/web-server/reverse-proxy/)
   the web interface with proper security measures such as HTTPS.

## StartOS package

<p align="center">
  <img src="https://raw.githubusercontent.com/storopoli/dead-man-switch-startos/refs/heads/master/icon.png" alt="Project Logo" width="21%">
</p>

The Web Interface can be easy deployed to any device that runs [startos](https://start9.com/).
Check the instructions at [`storopoli/dead-man-switch-startos`](https://github.com/storopoli/dead-man-switch-startos)

## License

The source code is licensed under an
[AGPL v3 License](https://opensource.org/license/agpl-v3/)

[![AGPL-v3](https://upload.wikimedia.org/wikipedia/commons/thumb/0/06/AGPLv3_Logo.svg/320px-AGPLv3_Logo.svg.png)](https://opensource.org/license/agpl-v3/)
