#!/bin/sh
set -eu

export HOME=/root
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL=C
export XDG_RUNTIME_DIR=/tmp/sophia-runtime
export LIBGL_DRIVERS_PATH=/usr/lib/dri

mkdir -p /proc /sys /dev /run /run/udev /tmp /tmp/.X11-unix "$XDG_RUNTIME_DIR"
mount -t proc proc /proc 2>/dev/null || true
mount -t sysfs sysfs /sys 2>/dev/null || true
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
mkdir -p /dev/pts
mount -t devpts devpts /dev/pts
mount -t tmpfs tmpfs /run 2>/dev/null || true
chmod 700 "$XDG_RUNTIME_DIR"

scenario="session"
two_xterm=false
cmdline=""
IFS= read -r cmdline < /proc/cmdline || true
case " $cmdline " in
    *" sophia.scenario=emergency-recovery "*) scenario="emergency-recovery" ;;
    *" sophia.scenario=gtk-classic "*) scenario="gtk-classic" ;;
    *" sophia.scenario=gtk-confined "*) scenario="gtk-confined" ;;
    *" sophia.scenario=xmonad-m7 "*) scenario="xmonad-m7" ;;
    *" sophia.scenario=xmonad-m8-launcher "*) scenario="xmonad-m8-launcher" ;;
    *" sophia.scenario=xmonad-m8-mix "*) scenario="xmonad-m8-mix" ;;
    *" sophia.scenario=xmonad-m8-soak "*) scenario="xmonad-m8-soak" ;;
esac
case " $cmdline " in
    *" sophia.two_xterm=1 "*) two_xterm=true ;;
esac

if [ "$scenario" = "emergency-recovery" ]; then
    echo "sophia_qemu_guest schema=1 status=booting gpu=virtio-gpu scenario=emergency-recovery"
elif [ "$scenario" = "gtk-classic" ] || [ "$scenario" = "gtk-confined" ]; then
    echo "sophia_qemu_guest schema=1 status=booting gpu=virtio-gpu scenario=$scenario"
elif [ "$scenario" = "xmonad-m7" ] || [ "$scenario" = "xmonad-m8-launcher" ] || [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ]; then
    echo "sophia_qemu_guest schema=1 status=booting gpu=virtio-gpu scenario=$scenario"
else
    echo "sophia_qemu_guest schema=1 status=booting gpu=virtio-gpu ticks=300"
fi

udevd --daemon
udevadm control --log-priority=err

modprobe virtio_pci
modprobe virtio_gpu
modprobe virtio_input
modprobe evdev
udevadm trigger --action=add
udevadm settle --timeout=5

attempt=0
while [ ! -e /dev/dri/card0 ] && [ "$attempt" -lt 100 ]; do
    sleep 0.05
    attempt=$((attempt + 1))
done

if [ ! -e /dev/dri/card0 ]; then
    echo "sophia_qemu_guest schema=1 status=failed reason=virtio_gpu_drm_missing"
    poweroff -f
fi

connector_count=0
connected_count=0
for connector in /sys/class/drm/card[0-9]-*; do
    if [ ! -f "$connector/status" ]; then
        continue
    fi
    connector_count=$((connector_count + 1))
    status=""
    IFS= read -r status < "$connector/status" || true
    if [ "$status" = "connected" ]; then
        connected_count=$((connected_count + 1))
    fi
done
echo "sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=$connector_count connected=$connected_count"

input_devices=""
for device in /dev/input/event*; do
    if [ -e "$device" ]; then
        if [ -z "$input_devices" ]; then
            input_devices="$device"
        else
            input_devices="$input_devices,$device"
        fi
    fi
done

guard_pid=""
guard_triggered_file="/tmp/sophia-input-guard.triggered"
if [ "$scenario" = "emergency-recovery" ]; then
    if [ -z "$input_devices" ]; then
        echo "sophia_qemu_guest_recovery schema=1 status=failed reason=input_devices_missing"
        sync
        poweroff -f
    fi
    guard_armed_file="/tmp/sophia-input-guard.armed"
    rm -f "$guard_armed_file" "$guard_triggered_file"
    /usr/bin/sophia sophia-session-input-guard \
        "--input-devices=$input_devices" \
        "--armed-file=$guard_armed_file" \
        "--triggered-file=$guard_triggered_file" \
        "--owner-pid=$$" &
    guard_pid=$!
    guard_armed=false
    attempt=0
    while [ "$attempt" -lt 600 ]; do
        if [ -s "$guard_armed_file" ]; then
            guard_armed=true
            break
        fi
        if ! kill -0 "$guard_pid" 2>/dev/null; then
            break
        fi
        sleep 0.05
        attempt=$((attempt + 1))
    done
    if [ "$guard_armed" != true ]; then
        echo "sophia_qemu_guest_recovery schema=1 status=failed reason=input_guard_arm_timeout"
        sync
        poweroff -f
    fi
    set -- sophia-live-session --display=:181 --native-scanout --max-runtime-ms=30000
    echo "sophia_qemu_guest_recovery schema=1 status=running chord=ctrl-alt-backspace"
elif [ "$scenario" = "gtk-classic" ] || [ "$scenario" = "gtk-confined" ]; then
    profile="classic"
    [ "$scenario" = "gtk-confined" ] && profile="confined"
    expected_stdout="$(printf 'sophia\n.')"
    expected_stdout="${expected_stdout%.}"
    set -- sophia-live-session --display=:181 --native-scanout --max-runtime-ms=30000 \
        --namespace-profile="$profile" --software-client-rendering \
        --client=zenity --client-arg=--entry --client-arg=--title \
        --client-arg='Sophia GTK proof' --client-arg=--text \
        --client-arg='Type sophia, then click OK' \
        --expect-client-stdout="$expected_stdout" --require-client-normal-exit \
        --expect-physical-text=sophia --expect-physical-pointer \
        --inject-surface-resize=640x360 --exit-after-input-proof
    echo "sophia_qemu_gtk schema=1 status=running profile=$profile"
elif [ "$scenario" = "xmonad-m7" ] || [ "$scenario" = "xmonad-m8-launcher" ] || [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ]; then
    if [ ! -x /usr/bin/xmonad ]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=xmonad_missing"
        sync
        poweroff -f
    fi
    runtime_ms=60000
    [ "$scenario" != "xmonad-m8-mix" ] || runtime_ms=180000
    [ "$scenario" != "xmonad-m8-soak" ] || runtime_ms=1860000
    set -- sophia-live-session --display=:181 --native-scanout --max-runtime-ms="$runtime_ms"
    if [ "$scenario" = "xmonad-m8-launcher" ]; then
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=terminal=/usr/bin/xterm
        set -- "$@" --session-app-arg=terminal=-cm --session-app-arg=terminal=-dc
        set -- "$@" --session-start=terminal --session-action-app=terminal=terminal
        echo "sophia_qemu_xmonad schema=1 status=running windows=1 profile=xmonad mode=normal"
    elif [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ]; then
        for program in /usr/bin/firefox /usr/bin/vkcube /usr/bin/zenity; do
            if [ ! -x "$program" ]; then
                echo "sophia_qemu_xmonad schema=1 status=failed reason=m8_application_missing program=$program"
                sync
                poweroff -f
            fi
        done
        export MOZ_ENABLE_WAYLAND=0
        export MOZ_FORCE_DISABLE_E10S=1
        export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json
        mkdir -p /tmp/firefox-profile
        printf '%s\n' \
            'user_pref("browser.tabs.remote.autostart", false);' \
            'user_pref("browser.tabs.remote.autostart.2", false);' \
            'user_pref("fission.autostart", false);' \
            > /tmp/firefox-profile/user.js
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=terminal=/usr/bin/xterm
        set -- "$@" --session-app-arg=terminal=-cm --session-app-arg=terminal=-dc
        set -- "$@" --session-app=vulkan=/usr/bin/vkcube --session-app-arg=vulkan=--wsi --session-app-arg=vulkan=xcb
        set -- "$@" --session-app-arg=vulkan=--width --session-app-arg=vulkan=640
        set -- "$@" --session-app-arg=vulkan=--height --session-app-arg=vulkan=720
        set -- "$@" --session-app=launcher=/usr/bin/zenity --session-app-arg=launcher=--info --session-app-arg=launcher=--text=Sophia-application-launcher
        set -- "$@" --session-app=firefox=/usr/bin/firefox --session-app-arg=firefox=--new-instance --session-app-arg=firefox=--no-remote
        set -- "$@" --session-app-arg=firefox=--profile --session-app-arg=firefox=/tmp/firefox-profile
        set -- "$@" --session-app-arg=firefox=file:///usr/share/sophia/firefox_m8_local_page.html
        set -- "$@" --session-start=terminal --session-start=vulkan
        set -- "$@" --session-action-app=terminal=terminal --session-action-app=launcher=launcher --session-action-app=firefox=firefox
        set -- "$@" --firefox-m8-proof
        echo "sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad mode=m8-app-mix"
    else
        set -- "$@" --secondary-terminal
        echo "sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad"
    fi
    set -- "$@" --wm-process=/usr/bin/sophia-x11-wm-bridge
    set -- "$@" --wm-process-arg=--profile=xmonad
    set -- "$@" --wm-process-arg=--wm=/usr/bin/xmonad
    set -- "$@" --wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux
else
    set -- sophia-live-session --display=:181 --native-scanout --max-ticks=300 \
        --expect-physical-text=sophia --expect-physical-pointer
    if [ "$two_xterm" = true ]; then
        set -- "$@" --secondary-terminal
    fi
fi

if [ -n "$input_devices" ]; then
    set -- "$@" "--input-devices=$input_devices"
if [ "$scenario" = "xmonad-m7" ] || [ "$scenario" = "xmonad-m8-launcher" ] || [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ]; then
    (
        while ! pidof sophia-x11-wm-bridge >/dev/null 2>&1; do sleep 0.05; done
        sleep 9
        while :; do
            wm_pid="$(pidof xmonad 2>/dev/null || true)"
            bridge_pid="$(pidof sophia-x11-wm-bridge 2>/dev/null || true)"
            [ -z "$wm_pid" ] || kill -TERM $wm_pid 2>/dev/null || true
            [ -z "$bridge_pid" ] || kill -TERM $bridge_pid 2>/dev/null || true
            echo "sophia_qemu_xmonad schema=1 status=restart_injected target=compatibility_bridge"
            [ "$scenario" = "xmonad-m8-soak" ] || break
            sleep 180
        done
    ) &
fi

fi

set +e
SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 /usr/bin/sophia "$@"
status=$?
set -e

if [ "$scenario" = "emergency-recovery" ]; then
    guard_done=false
    attempt=0
    while [ "$attempt" -lt 100 ]; do
        if ! kill -0 "$guard_pid" 2>/dev/null; then
            guard_done=true
            break
        fi
        sleep 0.05
        attempt=$((attempt + 1))
    done
    set +e
    if [ "$guard_done" = true ]; then
        wait "$guard_pid"
        guard_status=$?
    else
        kill -TERM "$guard_pid" 2>/dev/null || true
        wait "$guard_pid" 2>/dev/null || true
        guard_status=124
    fi
    set -e
    guard_pid=""
else
    guard_status=0
fi

if [ "$scenario" = "emergency-recovery" ]; then
    if [ "$status" -eq 0 ] && [ "$guard_status" -eq 0 ] \
        && [ -s "$guard_triggered_file" ]; then
        echo "sophia_qemu_guest_recovery schema=1 status=complete exit_status=0 guard_exit_status=0"
    else
        echo "sophia_qemu_guest_recovery schema=1 status=failed reason=recovery_exit exit_status=$status guard_exit_status=$guard_status"
    fi
elif [ "$scenario" = "gtk-classic" ] || [ "$scenario" = "gtk-confined" ]; then
    if [ "$status" -eq 0 ]; then
        echo "sophia_qemu_guest schema=1 status=complete scenario=$scenario"
    else
        echo "sophia_qemu_guest schema=1 status=failed reason=gtk_session_exit scenario=$scenario exit_status=$status"
    fi
elif [ "$scenario" = "xmonad-m7" ] || [ "$scenario" = "xmonad-m8-launcher" ] || [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ]; then
    if [ "$status" -eq 0 ]; then
        echo "sophia_qemu_guest schema=1 status=complete scenario=$scenario"
    else
        echo "sophia_qemu_guest schema=1 status=failed reason=xmonad_session_exit exit_status=$status"
    fi
elif [ "$status" -eq 0 ]; then
    echo "sophia_qemu_guest schema=1 status=complete ticks=300"
else
    echo "sophia_qemu_guest schema=1 status=failed reason=session_exit exit_status=$status"
fi

sync
poweroff -f
