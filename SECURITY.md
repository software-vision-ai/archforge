# Security

## Reporting a problem

Please do not report security problems in a public issue. Use GitHub's private vulnerability
reporting for this repository (the **Security** tab, then **Report a vulnerability**). Say what you
found, how to reproduce it and what it affects; you will get an answer as soon as the report has
been read.

## Supported versions

There is no release yet. Fixes go into the `main` branch.

## What the application reads

The desktop application has no network access and no remote control surface. What it reads from
outside the process:

- its preferences, `prefs.json` in the per-user settings folder: a file that cannot be read, is not
  valid, or is larger than its limit is set aside under a new name and the defaults are used;
- the fonts installed on the computer, to draw scripts its bundled faces lack: each file is read
  under a size limit, never mapped, and checked with egui's own font parser before it is used;
- the full name of the account it runs under, for Home's avatar: on macOS what `/usr/bin/id -F`
  prints, on Linux the account's entry in `/etc/passwd`, elsewhere the login name;
- on Linux, the graphics devices listed under `/sys/class/drm`, to pick the graphics adapter;
- the environment variables `RUST_LOG` (log filter), `ARCHFORGE_VULKAN` (graphics backend),
  `USER` and `USERNAME`, and the folder variables (`HOME`, `APPDATA`, `XDG_CONFIG_HOME`, `WINDIR`,
  `LOCALAPPDATA`) that locate its settings and the installed fonts.

It writes only inside its settings folder: the preferences, its log (`logs/archforge.log`) and a
marker file that records which graphics choice is starting, so a choice that crashed is skipped at
the next start.

A problem in any of these, or anything that makes the application crash, hang or write outside its
settings folder, is in scope.
