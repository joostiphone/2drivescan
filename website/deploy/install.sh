#!/usr/bin/env bash
# One-time setup of 2drivescan.app on a Debian/Ubuntu server. Run as root:
#
#   bash install.sh
#
# It clones the repository, configures Laravel, deploys once and installs a
# systemd timer that keeps the site up to date. It does not change your web
# server configuration; see deploy/nginx.conf for an example.
set -euo pipefail

REPO_URL="${REPO_URL:-https://github.com/joostiphone/2drivescan.git}"
REPO_DIR="${REPO_DIR:-/var/www/2drivescan}"
WEB_USER="${WEB_USER:-www-data}"
SITE_URL="${SITE_URL:-https://2drivescan.app}"

[[ $EUID -eq 0 ]] || { echo "Run as root."; exit 1; }

php_ok="$(php -r 'echo version_compare(PHP_VERSION, "8.3.0", ">=") ? "yes" : "no";' 2>/dev/null || echo no)"
if [[ "$php_ok" != "yes" ]]; then
  echo "PHP 8.3 or newer is required (found: $(php -r 'echo PHP_VERSION;' 2>/dev/null || echo none))."
  exit 1
fi
for bin in git composer; do
  command -v "$bin" >/dev/null || { echo "$bin is not installed."; exit 1; }
done
for ext in mbstring xml curl tokenizer ctype fileinfo; do
  php -m | grep -qi "^$ext\$" || echo "Warning: PHP extension '$ext' seems to be missing."
done

if [[ ! -d "$REPO_DIR/.git" ]]; then
  git clone --quiet "$REPO_URL" "$REPO_DIR"
fi

cd "$REPO_DIR/website"
if [[ ! -f .env ]]; then
  cp .env.example .env
  sed -i "s#^APP_URL=.*#APP_URL=${SITE_URL}#" .env
  COMPOSER_ALLOW_SUPERUSER=1 composer install --no-dev --no-interaction --prefer-dist --quiet
  php artisan key:generate --force --quiet
fi

chmod +x deploy/update.sh
REPO_DIR="$REPO_DIR" WEB_USER="$WEB_USER" deploy/update.sh --force

cat > /etc/systemd/system/2drivescan-update.service <<UNIT
[Unit]
Description=Update 2drivescan.app from GitHub
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
Environment=REPO_DIR=${REPO_DIR}
Environment=WEB_USER=${WEB_USER}
ExecStart=${REPO_DIR}/website/deploy/update.sh
UNIT

cat > /etc/systemd/system/2drivescan-update.timer <<UNIT
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
systemctl enable --now 2drivescan-update.timer

echo
echo "Done. Point your web server at ${REPO_DIR}/website/public (example: deploy/nginx.conf)"
echo "and publish it over HTTPS with Cloudflare Tunnel (see website/README.md)."
echo "Logs of automatic updates: journalctl -u 2drivescan-update"
