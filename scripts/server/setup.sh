#!/usr/bin/env bash
# Runs on the server, as root. Started by scripts/install-server.sh over SSH,
# but nothing stops you from running it by hand with the variables below set.
#
# Installs yaca-server as a systemd service behind nginx, with TLS from Let's
# Encrypt, and prints a connection link for the app.
#
#   YACA_DOMAIN     DNS name that already points at this machine (required unless YACA_INSECURE=1)
#   YACA_EMAIL      address for Let's Encrypt expiry notices (optional)
#   YACA_DEVICE     name of the first device's token            (default: phone)
#   YACA_USER       system user the agent runs as               (default: yaca)
#   YACA_PORT       local port of yaca-server                   (default: 7878)
#   YACA_BINARY     path of a prebuilt yaca-server to install   (else it is built)
#   YACA_SRC        existing checkout to build from             (else YACA_REPO is cloned)
#   YACA_REPO       git URL to clone                            (default: https://github.com/zorahm/yaca.git)
#   YACA_REF        branch or tag to build                      (default: main)
#   YACA_INSECURE   1 = serve plain HTTP on port 80, no TLS (for a LAN or a VPN only)
#   YACA_NO_SYSTEMD 1 = do not touch systemd (containers, tests)

set -euo pipefail

DOMAIN="${YACA_DOMAIN:-}"
EMAIL="${YACA_EMAIL:-}"
DEVICE="${YACA_DEVICE:-phone}"
RUN_AS="${YACA_USER:-yaca}"
PORT="${YACA_PORT:-7878}"
BINARY="${YACA_BINARY:-}"
SRC="${YACA_SRC:-}"
REPO="${YACA_REPO:-https://github.com/zorahm/yaca.git}"
REF="${YACA_REF:-main}"
INSECURE="${YACA_INSECURE:-0}"
NO_SYSTEMD="${YACA_NO_SYSTEMD:-0}"

say() { printf '\n==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "run as root (the installer uses sudo for you)"
command -v apt-get >/dev/null || die "only Debian and Ubuntu are supported for now"
if [ "$INSECURE" != 1 ] && [ -z "$DOMAIN" ]; then
  die "give a domain that points at this machine (a phone only trusts a real certificate), or set YACA_INSECURE=1 for plain HTTP on a trusted network"
fi
case "$DEVICE" in *[!A-Za-z0-9._-]*|"") die "the device name may use letters, digits and . _ - only";; esac
case "$RUN_AS" in *[!a-z0-9_-]*|"") die "bad user name";; esac

export DEBIAN_FRONTEND=noninteractive

say "Installing packages"
PACKAGES=(nginx curl ca-certificates git)
[ "$INSECURE" = 1 ] || PACKAGES+=(certbot python3-certbot-nginx)
[ -n "$BINARY" ] || PACKAGES+=(build-essential pkg-config libssl-dev libdbus-1-dev)
apt-get update -qq
apt-get install -y -qq "${PACKAGES[@]}"
# The binary links libdbus for the keyring, so it must be there to run too.
[ -z "$BINARY" ] || apt-get install -y -qq libdbus-1-3

say "User $RUN_AS"
if ! id "$RUN_AS" >/dev/null 2>&1; then
  # A real home and a real shell: the agent works in projects under this home
  # and runs the commands it is asked to run. No password, no sudo.
  useradd --create-home --shell /bin/bash "$RUN_AS"
  passwd -l "$RUN_AS" >/dev/null
fi
RUN_HOME="$(getent passwd "$RUN_AS" | cut -d: -f6)"
as_user() { sudo -u "$RUN_AS" -H env "PATH=$RUN_HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin" "$@"; }

if [ -n "$BINARY" ]; then
  say "Installing the uploaded binary"
  install -m 0755 "$BINARY" /usr/local/bin/yaca-server
else
  say "Building yaca-server (a few minutes; a small machine needs 2 GB of memory or a swap file)"
  if ! as_user bash -c 'command -v cargo >/dev/null'; then
    as_user bash -c 'curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal'
  fi
  if [ -z "$SRC" ]; then
    SRC="$RUN_HOME/yaca"
    if [ -d "$SRC/.git" ]; then
      as_user git -C "$SRC" fetch --depth 1 origin "$REF"
      as_user git -C "$SRC" checkout -q FETCH_HEAD
    else
      as_user git clone --depth 1 --branch "$REF" "$REPO" "$SRC"
    fi
  fi
  as_user bash -c "cd '$SRC' && cargo build --release -p yaca-server"
  install -m 0755 "$SRC/target/release/yaca-server" /usr/local/bin/yaca-server
fi
/usr/local/bin/yaca-server --version

say "Token for $DEVICE"
TOKEN=""
if as_user /usr/local/bin/yaca-server token list | cut -f1 | grep -qx "$DEVICE"; then
  echo "A token named \"$DEVICE\" already exists and cannot be shown again."
  echo "Add another with:  sudo -u $RUN_AS yaca-server token add <name>"
else
  TOKEN="$(as_user /usr/local/bin/yaca-server token add "$DEVICE")"
fi

if [ "$NO_SYSTEMD" != 1 ]; then
  say "systemd service"
  cat > /etc/systemd/system/yaca-server.service <<UNIT
[Unit]
Description=Yaca server
After=network-online.target
Wants=network-online.target

[Service]
User=$RUN_AS
WorkingDirectory=$RUN_HOME
ExecStart=/usr/local/bin/yaca-server run --listen 127.0.0.1:$PORT
Restart=on-failure
RestartSec=3
# The agent edits files and runs commands in the user's projects, so the home
# directory stays writable; the service cannot gain more privilege than the user has.
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
UNIT
  systemctl daemon-reload
  systemctl enable --now yaca-server
  systemctl restart yaca-server
fi

say "nginx"
SERVER_NAME="${DOMAIN:-_}"
PROXY=$(cat <<NGINX
    # One WebSocket per device. It is quiet for long stretches while a model
    # thinks, so the proxy must not close it for being idle.
    location = /ws {
        limit_req zone=yaca_ws burst=10 nodelay;
        proxy_pass http://127.0.0.1:$PORT;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_set_header X-Forwarded-For \$remote_addr;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
        proxy_buffering off;
        client_max_body_size 64m;
    }

    location = /health {
        proxy_pass http://127.0.0.1:$PORT;
    }

    location / {
        return 404;
    }
NGINX
)
# Guessing tokens is slow on the server's side already; this keeps a flood of
# connection attempts from reaching it at all.
cat > /etc/nginx/conf.d/yaca-limits.conf <<'NGINX'
limit_req_zone $binary_remote_addr zone=yaca_ws:10m rate=30r/m;
NGINX

# A machine without IPv6 (some VPS kernels, containers) refuses a listener on it.
LISTEN6=""
[ -e /proc/net/if_inet6 ] && LISTEN6="listen [::]:80;"

cat > /etc/nginx/sites-available/yaca <<NGINX
server {
    listen 80;
    $LISTEN6
    server_name $SERVER_NAME;

$PROXY
}
NGINX
ln -sf /etc/nginx/sites-available/yaca /etc/nginx/sites-enabled/yaca
rm -f /etc/nginx/sites-enabled/default
nginx -t
if [ "$NO_SYSTEMD" != 1 ]; then systemctl reload nginx || systemctl restart nginx; fi

SCHEME=http
if [ "$INSECURE" != 1 ]; then
  say "TLS certificate for $DOMAIN"
  CERTBOT=(certbot --nginx -d "$DOMAIN" --non-interactive --agree-tos --redirect)
  if [ -n "$EMAIL" ]; then CERTBOT+=(-m "$EMAIL"); else CERTBOT+=(--register-unsafely-without-email); fi
  "${CERTBOT[@]}"
  SCHEME=https
fi

if command -v ufw >/dev/null && ufw status | grep -q "Status: active"; then
  say "Firewall"
  ufw allow 'Nginx Full' >/dev/null || { ufw allow 80/tcp; ufw allow 443/tcp; }
fi

HOST="${DOMAIN:-$(hostname -I | awk '{print $1}')}"
if [ "$NO_SYSTEMD" != 1 ]; then
  say "Checking"
  sleep 1
  curl -fsS --max-time 10 "$SCHEME://$HOST/health" || die "the server does not answer at $SCHEME://$HOST/health"
fi

printf '\n'
printf '============================================================\n'
printf ' Yaca server is up:  %s://%s\n' "$SCHEME" "$HOST"
if [ -n "$TOKEN" ]; then
  printf ' Token (shown once): %s\n\n' "$TOKEN"
  printf ' One line for the app (paste it into the address field):\n\n'
  if [ "$SCHEME" = http ]; then
    printf '   yaca://connect?host=%s&token=%s&plain=1\n' "$HOST" "$TOKEN"
  else
    printf '   yaca://connect?host=%s&token=%s\n' "$HOST" "$TOKEN"
  fi
  if [ "$SCHEME" = http ]; then
    printf '\n WARNING: plain HTTP. Anyone on the path can read the token and the chats.\n'
    printf '          Use it only inside a VPN or a trusted LAN.\n'
  fi
fi
printf '============================================================\n'
