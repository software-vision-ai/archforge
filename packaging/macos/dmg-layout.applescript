-- Arranges the disk image's window: icon view, window size, icon positions and, when there is one,
-- the background picture. Run by package.sh: osascript dmg-layout.applescript <volume> <app> <yes|no>
on run argv
    set volumeName to item 1 of argv
    set appName to item 2 of argv
    set hasBackground to (item 3 of argv) is "yes"
    tell application "Finder"
        tell disk volumeName
            open
            set current view of container window to icon view
            set toolbar visible of container window to false
            set statusbar visible of container window to false
            set the bounds of container window to {200, 120, 860, 520}
            set viewOptions to the icon view options of container window
            set arrangement of viewOptions to not arranged
            set icon size of viewOptions to 128
            if hasBackground then
                set background picture of viewOptions to file ".background:background.tiff"
            end if
            set position of item (appName & ".app") of container window to {170, 200}
            set position of item "Applications" of container window to {490, 200}
            update without registering applications
            delay 1
            close
        end tell
    end tell
end run
