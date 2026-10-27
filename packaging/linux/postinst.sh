#!/bin/sh
# Refreshes the desktop database and the icon cache. Desktop integration never blocks installation
# or launch, so every step tolerates a missing tool.
set +e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q /usr/share/icons/hicolor
exit 0
