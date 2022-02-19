Workstyle
===

Sway/i3 workspaces with style:

This application will dynamically rename your workspaces to indicate which programs are running in each one.

A picture is better than a thousand words!

* The workspace bar could look like this (uses waybar)
![alt tag](https://github.com/pierrechevalier83/workstyle/blob/master/screenshots/bar.png)

* In context:
![alt tag](https://github.com/pierrechevalier83/workstyle/blob/master/screenshots/full.png)

Installation
===

```
cargo install workstyle
```

Usage
===

Simply run the executable:
```
workstyle
```

```
workspace --help
```
will give you some more context.

Sway configuration
===

Add this line to your sway config:
```
exec_always --no-startup-id workstyle &> /tmp/workstyle.log
```

You may also want to control the log level with the environment variable: RUST_LOG to error, info or debug.

Note that since your workspaces will be renamed all the time, you should configure your keybindings to use numbered workspaces instead of assuming that the name is the number:
Prefer
```
    bindsym $mod+1 workspace number 1
```
over
```
    bindsym $mod+1 workspace 1
```

SystemD integration
====

Alternatively you can use the workstyle.service file to configure systemd to automatically start workstyle after you login

Copy `workstyle.service` to `$HOME/.config/systemd/user/`

and run

```
systemctl --user enable workstyle.service
systemctl --user start workstyle.service
```

Configuration
===

The main configuration consists of deciding which icons to use for which applications.

The config file is located at `${XDG_CONFIG_HOME}/workstyle/config.toml` or `/etc/xdg/workstyle/config.toml` (the former takes precedence over the latter). It will be generated if missing. Read the generated file. The syntax is in TOML and should be pretty self-explanatory.

When an app isn't recogised in the config, `workstyle` will log the application name as an error.
Simply add that string (case insensitive) to your config file, with an icon of your choice.

If no matching icon can be found in the config, a blank space will be used.
To override this, set the default icon in the config as per below:
```toml
[other]
fallback_icon = "your icon"
```

If you prefer not to have multiple copies of the same icon when there are multiple matching windows, set this config option:
```toml
[other]
deduplicate_icons = true
```

Note that the crate [`find_unicode`](https://github.com/pierrechevalier83/find_unicode/) can help find a unicode character directly from the command line. It now supports all of nerdfonts unicode space.
