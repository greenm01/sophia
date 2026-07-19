#!/bin/sh
set -u

echo "sophia_qemu_zenity_wrapper schema=1 status=started"
GTK_A11Y=none GSK_RENDERER=cairo \
    /usr/bin/zenity --info --text=Sophia-application-launcher
launcher_status=$?
echo "sophia_qemu_zenity_wrapper schema=1 status=child_reaped"
: "$launcher_status"
echo "sophia_qemu_zenity_wrapper schema=1 status=complete"
exit 0
