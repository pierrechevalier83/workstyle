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
cargo install workstyke
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

Configuration
===

The main configuration consists of deciding which icons to use for which applications.

The config file is located at `${XDG_CONFIG_HOME}/workstyle/config.toml`. It will be generated if missing. Read the generated file. The syntax is in TOML and should be pretty self-explanatory.

When an app isn't recogised in the config, `workstyle` will log the application name as an error.
Simply add that string (case insensitive) to your config file, with an icon of your choice.

Note that the crate [`find_unicode`](https://github.com/pierrechevalier83/find_unicode/) can help find a unicode character directly from the command line. It now supports all of nerdfonts unicode space.

