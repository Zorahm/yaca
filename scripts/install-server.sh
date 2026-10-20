#!/usr/bin/env bash
# Sets Yaca's server up on a machine you can reach over SSH, and prints what the
# app needs to connect to it.
#
#   scripts/install-server.sh user@host --domain yaca.example.com [options]
#
# What happens on the server (scripts/server/setup.sh, run as root through sudo):
# a `yaca` user, yaca-server as a systemd service on 127.0.0.1, nginx in front of
# it with a Let's Encrypt certificate, a token for your first device. Nothing is
# left to do by hand except pasting the printed line into the app.
#
# Options:
#   --domain D      DNS name that already points at the server (needed for TLS)
#   --email E       address for Let's Encrypt notices
#   --device NAME   name of the first token (default: phone)
#   --binary PATH   upload a yaca-server you built, instead of building on the server
#   --src DIR       build from an existing checkout on the server
#   --repo URL      git URL to build from (default: this checkout's origin)
#   --ref REF       branch or tag to build (default: this checkout's branch)
#   --user NAME     system user the agent runs as (default: yaca)
#   --insecure      plain HTTP, no certificate; only for a VPN or a trusted LAN
#   --ssh-port N    SSH port of the server
#   --identity F    SSH key file

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

target=""
domain="" email="" device="phone" binary="" src="" repo="" ref="" runas="yaca" insecure=0
ssh_opts=()

while [ $# -gt 0 ]; do
  case "$1" in
    --domain) domain="$2"; shift 2;;
    --email) email="$2"; shift 2;;
    --device) device="$2"; shift 2;;
    --binary) binary="$2"; shift 2;;
    --src) src="$2"; shift 2;;
    --repo) repo="$2"; shift 2;;
    --ref) ref="$2"; shift 2;;
    --user) runas="$2"; shift 2;;
    --insecure) insecure=1; shift;;
    --ssh-port) ssh_opts+=(-p "$2"); shift 2;;
    --identity) ssh_opts+=(-i "$2"); shift 2;;
    -h|--help) sed -n '2,/^$/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0;;
    -*) die "unknown option $1";;
    *) [ -z "$target" ] || die "one server at a time"; target="$1"; shift;;
  esac
done
[ -n "$target" ] || die "usage: $0 user@host --domain yaca.example.com   (see --help)"
[ -n "$domain" ] || [ "$insecure" = 1 ] || die "give --domain, or --insecure for plain HTTP on a trusted network"
[ -z "$binary" ] || [ -f "$binary" ] || die "no such file: $binary"

# Defaults for building: the checkout this script lives in.
if [ -z "$binary" ] && [ -z "$src" ]; then
  if [ -z "$repo" ] && git -C "$here" rev-parse --git-dir >/dev/null 2>&1; then
    repo="$(git -C "$here" remote get-url origin 2>/dev/null || true)"
    # An SSH remote cannot be cloned by a server that has no key for it.
    case "$repo" in git@github.com:*) repo="https://github.com/${repo#git@github.com:}";; esac
  fi
  if [ -z "$ref" ] && git -C "$here" rev-parse --git-dir >/dev/null 2>&1; then
    ref="$(git -C "$here" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
  fi
fi

ssh_cmd=(ssh "${ssh_opts[@]}" -o ServerAliveInterval=30 "$target")

remote_binary=""
if [ -n "$binary" ]; then
  remote_binary="/tmp/yaca-server.$$"
  scp_opts=()
  # scp spells the port option differently from ssh.
  for ((i = 0; i < ${#ssh_opts[@]}; i++)); do
    case "${ssh_opts[$i]}" in -p) scp_opts+=(-P "${ssh_opts[$((i + 1))]}"); i=$((i + 1));; *) scp_opts+=("${ssh_opts[$i]}");; esac
  done
  printf 'Uploading %s\n' "$binary"
  scp "${scp_opts[@]}" "$binary" "$target:$remote_binary"
fi

# The variables travel as a quoted block in front of the script, so a value with
# spaces or quotes in it cannot become a command.
vars=""
add() { vars+="export $1=$(printf '%q' "$2")"$'\n'; }
add YACA_DOMAIN "$domain"; add YACA_EMAIL "$email"; add YACA_DEVICE "$device"
add YACA_USER "$runas"; add YACA_BINARY "$remote_binary"; add YACA_SRC "$src"
add YACA_REPO "${repo:-https://github.com/zorahm/yaca.git}"; add YACA_REF "${ref:-main}"
add YACA_INSECURE "$insecure"

printf 'Setting up %s\n' "$target"
{ printf '%s\n' "$vars"; cat "$here/server/setup.sh"; } |
  "${ssh_cmd[@]}" 'if [ "$(id -u)" -eq 0 ]; then exec bash -s; else exec sudo -n bash -s; fi' || { [ -z "$remote_binary" ] || "${ssh_cmd[@]}" "rm -f $remote_binary"; die "the setup failed; the output above says where"; }

[ -z "$remote_binary" ] || "${ssh_cmd[@]}" "rm -f $remote_binary"
