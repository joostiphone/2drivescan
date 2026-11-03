#!/usr/bin/env bash
# One-time setup of 2drivescan.app on a Debian/Ubuntu server with nginx,
# PHP-FPM (8.3+), Composer and git. Run as root:
#
#   bash install.sh
#
# Creates the release layout used by 2drivescan-deploy, installs the deploy
# script and a systemd timer that deploys new commits on main every 5
# minutes, and runs the first deploy. The nginx site (deploy/nginx.conf) and
# HTTPS (Cloudflare Tunnel) are set up separately; see website/README.md.
set -euo pipefail

APP_DIR="${APP_DIR:-/var/www/2drivescan}"
APP_USER="${APP_USER:-deploy}"
RAW="${RAW:-https://raw.githubusercontent.com/joostiphone/2drivescan/main/website}"
PHP_VERSION="${PHP_VERSION:-$(php -r 'echo PHP_MAJOR_VERSION.".".PHP_MINOR_VERSION;' 2>/dev/null || true)}"

[[ $EUID -eq 0 ]] || { echo "Run as root."; exit 1; }
php -r 'exit(version_compare(PHP_VERSION, "8.3.0", ">=") ? 0 : 1);' 2>/dev/null \
    || { echo "PHP 8.3 or newer is required."; exit 1; }
for bin in git composer curl nginx; do
    command -v "$bin" >/dev/null || { echo "$bin is not installed."; exit 1; }
done
systemctl list-unit-files "php${PHP_VERSION}-fpm.service" >/dev/null \
    || { echo "php${PHP_VERSION}-fpm is not installed."; exit 1; }

id "${APP_USER}" >/dev/null 2>&1 || useradd --system --create-home --shell /usr/sbin/nologin "${APP_USER}"
usermod -aG www-data "${APP_USER}"

mkdir -p "${APP_DIR}/releases" "${APP_DIR}/shared"
for d in app/public framework/cache/data framework/sessions framework/views logs; do
    mkdir -p "${APP_DIR}/shared/storage/${d}"
done

if [[ ! -f "${APP_DIR}/shared/.env" ]]; then
    curl -fsSL "${RAW}/.env.example" -o "${APP_DIR}/shared/.env"
    key="base64:$(php -r 'echo base64_encode(random_bytes(32));')"
    sed -i "s#^APP_KEY=.*#APP_KEY=${key}#" "${APP_DIR}/shared/.env"
fi
chown "${APP_USER}:www-data" "${APP_DIR}/shared/.env"
chmod 0640 "${APP_DIR}/shared/.env"
chown -R "${APP_USER}:www-data" "${APP_DIR}"

curl -fsSL "${RAW}/deploy/2drivescan-deploy" -o /usr/local/bin/2drivescan-deploy
chmod 0755 /usr/local/bin/2drivescan-deploy

if [[ ! -f /etc/default/2drivescan-deploy ]]; then
    cat > /etc/default/2drivescan-deploy <<CONF
APP_DIR=${APP_DIR}
APP_USER=${APP_USER}
REPO_URL=https://github.com/joostiphone/2drivescan.git
BRANCH=main
DOMAIN=2drivescan.app
KEEP_RELEASES=5
PHP_VERSION=${PHP_VERSION}
CONF
fi

cat > /etc/systemd/system/2drivescan-update.service <<'UNIT'
[Unit]
Description=Deploy 2drivescan.app when main changes on GitHub
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/2drivescan-deploy --if-changed
UNIT

cat > /etc/systemd/system/2drivescan-update.timer <<'UNIT'
[Unit]
Description=Check GitHub for 2drivescan.app updates every 5 minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=5min
Persistent=true

[Install]
WantedBy=timers.target
UNIT

systemctl daemon-reload
/usr/local/bin/2drivescan-deploy || echo "First deploy failed; check the output above (is the nginx site enabled?)."
systemctl enable --now 2drivescan-update.timer

echo
echo "Done. Enable the nginx site (example: deploy/nginx.conf, root ${APP_DIR}/current/website/public)"
echo "and publish it over HTTPS with Cloudflare Tunnel. Deploy log: journalctl -u 2drivescan-update"
