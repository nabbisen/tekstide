#!/usr/bin/env bash
set -euo pipefail

# The sanctioned way to launch a real tekstide GUI for evidence-gathering,
# against scratch configuration AND scratch state -- never the maintainer's
# real ones, and never assembled by hand in a one-off `env` command.
#
#   rfcs/handoffs/launch-scratch-gui.sh [--backend=x11|wayland] <scratch-root> [project-dir] [binary]
#   rfcs/handoffs/launch-scratch-gui.sh --kill <scratch-root>
#   rfcs/handoffs/launch-scratch-gui.sh --kill --by-name [binary]
#
#   --backend:    x11 (default; xdotool-driven input, forces WAYLAND_DISPLAY
#                 unset) or wayland (native session, wtype-driven input,
#                 WAYLAND_DISPLAY left as inherited). Review 670 required-2:
#                 the rules file's own Testing Guidelines record that
#                 `xdotool`'s keys do not reach this app under Wayland, so a
#                 keyboard test needs the Wayland backend and `wtype` -- this
#                 script forcing X11 unconditionally would have made a
#                 now-mandatory script unable to reproduce this project's own
#                 existing keyboard evidence (`wtype -M ctrl -M alt -k 1`,
#                 RFC-076's own PR-076-B evidence). Review 670 round 2
#                 required-4: `--backend=wayland` REFUSES rather than prints
#                 a false claim when `WAYLAND_DISPLAY` is unset or empty --
#                 the backend is the one thing the rule insists be stated
#                 correctly with the evidence, and "printed Wayland, ran
#                 under nothing" is worse than refusing.
#   scratch-root: directory to hold ./state (XDG_STATE_HOME), ./config
#                 (XDG_CONFIG_HOME), ./launcher.pid (this launch's own pid
#                 and binary name, for `--kill`) and, if project-dir is
#                 omitted, ./project
#   project-dir:  real project directory to open; defaults to
#                 <scratch-root>/project, created if absent
#   binary:       defaults to this checkout's own release build
#   --kill <scratch-root>:
#                 stop *the instance this script launched against that
#                 scratch root* -- reads ./launcher.pid, confirms the pid is
#                 still that exact process (by comm name, guarding against
#                 pid reuse after the original exited) before killing it.
#   --kill --by-name [binary]:
#                 the old, unscoped teardown: kill *every* process on this
#                 machine matching binary's name (default `tekstide`), by
#                 exact process name. Review 670 required-3: this is unsafe
#                 on a shared desktop with other agents' sessions on it, so
#                 it is no longer the default -- an explicit flag, not a
#                 bare `--kill`, is what reaches for it now.
#
# Response 164's original guard: an operator error let a launch command
# silently drop its scratch XDG_STATE_HOME override on a retry and ran a
# real terminal session against ~/.local/state/tekstide/. A printed value
# is a diagnostic nobody reads at the moment it matters; refusing to start
# is a precondition that cannot be skipped by habit.
#
# `.git-exclude/rules/project-instructions-gui-common.md`'s "GUI Evidence
# and Automation Rules" (2026-11-11) made three things mandatory that this
# script did not yet do, added here (PR-076-D3's follow-up, "GUI evidence
# prerequisites"):
#   - isolate XDG_CONFIG_HOME as well as XDG_STATE_HOME -- read from the
#     source (`crates/tekstide-core/src/config/path.rs`,
#     `crates/tekstide-core/src/project/recent/store.rs`), those are the
#     *only* two directories this app reads at all; no cache or data root
#     exists to isolate;
#   - "one instance only": refuse to start if one is already running, and
#     a scoped `--kill` teardown mode (review 670 required-3: scoped to the
#     instance this script started, not every instance on the machine), the
#     same two halves the rule itself names;
#   - live in the repository rather than `.git-exclude/`, since the rule
#     now makes this script mandatory for every real-UI session, and a
#     mandatory script absent from a fresh clone is not a script anyone
#     else can run.
#
# One consequence of isolating XDG_CONFIG_HOME, to meet before a confusing
# git result rather than after: `crates/tekstide-core/src/runtime/git.rs`
# forwards HOME and XDG_CONFIG_HOME from Tekstide's own process environment
# into every `git` subprocess it spawns. HOME is left real here, so
# `~/.gitconfig` still applies -- but a maintainer whose git identity lives
# at `~/.config/git/config` (XDG_CONFIG_HOME-relative) will not be seen by
# git invoked from inside this isolated session.

repo_root=$(git rev-parse --show-toplevel)

usage() {
    echo "usage: $0 [--backend=x11|wayland] <scratch-root> [project-dir] [binary]" >&2
    echo "       $0 --kill <scratch-root>" >&2
    echo "       $0 --kill --by-name [binary]" >&2
    exit 2
}

if [ "$#" -ge 1 ] && [ "$1" = "--kill" ]; then
    if [ "${2:-}" = "--by-name" ]; then
        binary_name=${3:-tekstide}
        echo "UNSCOPED kill by exact process name '$binary_name' -- this stops every matching process on the machine, not only one this script started. Shared desktop: other agents' sessions can be running the same name." >&2
        if pgrep -x "$binary_name" >/dev/null; then
            pkill -x "$binary_name"
            echo "killed running '$binary_name' instance(s) by exact process name" >&2
        else
            echo "no running '$binary_name' instance found" >&2
        fi
        exit 0
    fi

    scratch_root=${2:?usage: $0 --kill <scratch-root>}
    pid_file="$scratch_root/launcher.pid"
    if [ ! -r "$pid_file" ]; then
        echo "no $pid_file -- nothing launched against this scratch root (or it was already torn down)" >&2
        exit 0
    fi
    read -r recorded_pid recorded_binary_name < "$pid_file"
    comm_file="/proc/$recorded_pid/comm"
    if [ -r "$comm_file" ] && [ "$(cat "$comm_file")" = "$recorded_binary_name" ]; then
        kill "$recorded_pid"
        echo "killed pid $recorded_pid ('$recorded_binary_name'), the instance launched against $scratch_root" >&2
    else
        echo "pid $recorded_pid is not running as '$recorded_binary_name' any more -- nothing to kill (it may already have exited)" >&2
    fi
    rm -f "$pid_file"
    exit 0
fi

backend=x11
case "${1:-}" in
--backend=x11)
    backend=x11
    shift
    ;;
--backend=wayland)
    backend=wayland
    shift
    ;;
--backend=*)
    echo "REFUSING: unknown --backend value in '$1' -- use --backend=x11 or --backend=wayland" >&2
    exit 2
    ;;
esac

# Review 670 round 2, required-4: `--backend=wayland` with no real Wayland
# session to inherit would otherwise print "backend: Wayland" and proceed
# anyway -- a false backend claim landing in an evidence sidecar as fact,
# the one thing the rule insists be stated correctly. Refuse instead, with
# the same discipline the two XDG guards already have.
if [ "$backend" = wayland ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "REFUSING: --backend=wayland requested but WAYLAND_DISPLAY is unset or empty -- there is no real Wayland session for this launch to inherit, so wtype could not reach the result." >&2
    exit 1
fi

if [ "$#" -lt 1 ]; then
    usage
fi

scratch_root=$1
project_dir=${2:-"$scratch_root/project"}
binary=${3:-"$repo_root/target/release/tekstide"}

if [ ! -x "$binary" ]; then
    echo "REFUSING: binary not found or not executable: $binary" >&2
    exit 1
fi

# "One instance only": required before this script starts one, not only
# documented as a convention -- a stale second instance is exactly what
# makes `xdotool`'s "the first window found" unreliable, which is the
# defect the rule's window-selection sentence exists to rule out.
binary_name=$(basename "$binary")
if pgrep -x "$binary_name" >/dev/null; then
    echo "REFUSING: an instance of '$binary_name' is already running -- this script requires zero running instances before it starts one. Stop it first: $0 --kill <scratch-root> (or $0 --kill --by-name $binary_name)" >&2
    exit 1
fi

state_home="$scratch_root/state"
config_home="$scratch_root/config"
mkdir -p "$state_home" "$config_home" "$project_dir"

resolved_scratch_root=$(readlink -f "$scratch_root")

resolved_state_home=$(readlink -f "$state_home")
real_state_home=$(readlink -f "${XDG_STATE_HOME:-$HOME/.local/state}")

case "$resolved_state_home" in
"$resolved_scratch_root"/*) ;;
*)
    echo "REFUSING: resolved XDG_STATE_HOME ($resolved_state_home) does not fall inside the intended scratch root ($resolved_scratch_root)." >&2
    exit 1
    ;;
esac

if [ "$resolved_state_home" = "$real_state_home" ]; then
    echo "REFUSING: resolved XDG_STATE_HOME ($resolved_state_home) is the real state directory." >&2
    exit 1
fi

resolved_config_home=$(readlink -f "$config_home")
real_config_home=$(readlink -f "${XDG_CONFIG_HOME:-$HOME/.config}")

case "$resolved_config_home" in
"$resolved_scratch_root"/*) ;;
*)
    echo "REFUSING: resolved XDG_CONFIG_HOME ($resolved_config_home) does not fall inside the intended scratch root ($resolved_scratch_root)." >&2
    exit 1
    ;;
esac

if [ "$resolved_config_home" = "$real_config_home" ]; then
    echo "REFUSING: resolved XDG_CONFIG_HOME ($resolved_config_home) is the real config directory." >&2
    exit 1
fi

echo "$$ $binary_name" > "$scratch_root/launcher.pid"

echo "XDG_STATE_HOME confirmed inside scratch root: $resolved_state_home" >&2
echo "XDG_CONFIG_HOME confirmed inside scratch root: $resolved_config_home" >&2
echo "project directory: $project_dir" >&2
if [ "$backend" = wayland ]; then
    echo "backend: Wayland (WAYLAND_DISPLAY left as inherited; wtype-driven input, not xdotool)" >&2
else
    echo "backend: X11 (WAYLAND_DISPLAY unset for this launch; xdotool-driven input, not wtype)" >&2
fi
echo "HOME is left real: ~/.gitconfig still applies to git inside this session; a git identity kept under XDG_CONFIG_HOME instead will not be seen." >&2
echo "pid of the launched instance will be $$ (exec replaces this shell without forking), recorded in $scratch_root/launcher.pid -- use this for the focus check before every keystroke, and for '$0 --kill $scratch_root' afterward." >&2

if [ "$backend" = wayland ]; then
    exec env XDG_STATE_HOME="$state_home" XDG_CONFIG_HOME="$config_home" "$binary" "$project_dir"
else
    exec env -u WAYLAND_DISPLAY XDG_STATE_HOME="$state_home" XDG_CONFIG_HOME="$config_home" "$binary" "$project_dir"
fi
