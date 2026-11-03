#!/usr/bin/env bash
# One-time setup of 2drivescan.app on a Debian/Ubuntu server, matching
# production (see website/README.md). Run as root:
#
#   curl -fsSL https://raw.githubusercontent.com/joostiphone/2drivescan/main/website/deploy/install.sh | bash
#
# Installs, from this directory of the repository:
#   2drivescan-deploy           -> /usr/local/bin/2drivescan-deploy
#   2drivescan-deploy.default   -> /etc/default/2drivescan-deploy
#   nginx.conf                  -> /etc/nginx/sites-available/2drivescan (+ enabled)
#   php-fpm-pool.conf           -> /etc/php/<version>/fpm/pool.d/2drivescan.conf
#   logrotate                   -> /etc/logrotate.d/2drivescan
#   systemd/*                   -> /etc/systemd/system/
# then creates /var/www/2drivescan/{releases,shared}, deploys main and enables
# the 5-minute update timer. Existing configuration files are never
# overwritten; delete one first to reinstall it. HTTPS (Cloudflare Tunnel) is
# configured in Cloudflare, see website/README.md.
set -euo pipefail

RAW="${RAW:-https://raw.githubusercontent.com/joostiphone/2drivescan/main/website}"
APP_DIR="/var/www/2drivescan"
APP_USER="deploy"

[[ $EUID -eq 0 ]] || { echo "Run as root."; exit 1; }
for bin in git composer curl runuser nginx php; do
    command -v "$bin" >/dev/null || { echo "$bin is not installed."; exit 1; }
done
php -r 'exit(version_compare(PHP_VERSION, "8.3.0", ">=") ? 0 : 1);' || { echo "PHP 8.3 or newer is required."; exit 1; }
PHP_VERSION="$(php -r 'echo PHP_MAJOR_VERSION.".".PHP_MINOR_VERSION;')"
[[ -d "/etc/php/${PHP_VERSION}/fpm/pool.d" ]] || { echo "php${PHP_VERSION}-fpm is not installed."; exit 1; }
[[ -f /etc/nginx/snippets/fastcgi-php.conf ]] || { echo "nginx snippets/fastcgi-php.conf is missing."; exit 1; }

# Downloads a file from the repository unless the target already exists.
install_file() {
    local from="$1" to="$2" mode="${3:-0644}"
    if [[ -e "$to" ]]; then
        echo "keep     $to (already exists)"
        return
    fi
    curl -fsSL "${RAW}/deploy/${from}" -o "$to.tmp"
    chmod "$mode" "$to.tmp"
    mv -f "$to.tmp" "$to"
    echo "install  $to"
}

id "${APP_USER}" >/dev/null 2>&1 || useradd --create-home --shell /bin/bash "${APP_USER}"
usermod -aG www-data "${APP_USER}"

mkdir -p "${APP_DIR}/releases" "${APP_DIR}/shared"
for d in app/public framework/cache/data framework/sessions framework/views logs; do
    mkdir -p "${APP_DIR}/shared/storage/${d}"
done
if [[ ! -f "${APP_DIR}/shared/.env" ]]; then
    curl -fsSL "${RAW}/.env.example" -o "${APP_DIR}/shared/.env"
    sed -i "s#^APP_KEY=.*#APP_KEY=base64:$(php -r 'echo base64_encode(random_bytes(32));')#" "${APP_DIR}/shared/.env"
    echo "install  ${APP_DIR}/shared/.env (new APP_KEY)"
fi
chown -R "${APP_USER}:www-data" "${APP_DIR}"
find "${APP_DIR}/shared/storage" -type d -exec chmod 2775 {} +
chmod 0640 "${APP_DIR}/shared/.env"

# The deploy script is always refreshed so the server runs the repository's version.
curl -fsSL "${RAW}/deploy/2drivescan-deploy" -o /usr/local/bin/2drivescan-deploy.tmp
install -m 0755 /usr/local/bin/2drivescan-deploy.tmp /usr/local/bin/2drivescan-deploy
rm -f /usr/local/bin/2drivescan-deploy.tmp
echo "install  /usr/local/bin/2drivescan-deploy"

if [[ ! -e /etc/default/2drivescan-deploy ]]; then
    install_file 2drivescan-deploy.default /etc/default/2drivescan-deploy
    sed -i "s#^PHP_VERSION=.*#PHP_VERSION=\"${PHP_VERSION}\"#" /etc/default/2drivescan-deploy
else
    echo "keep     /etc/default/2drivescan-deploy (already exists)"
fi
install_file php-fpm-pool.conf "/etc/php/${PHP_VERSION}/fpm/pool.d/2drivescan.conf"
install_file nginx.conf /etc/nginx/sites-available/2drivescan
install_file logrotate /etc/logrotate.d/2drivescan
install_file systemd/2drivescan-update.service /etc/systemd/system/2drivescan-update.service
install_file systemd/2drivescan-update.timer /etc/systemd/system/2drivescan-update.timer

# The 2drivescan site is the server's default site (listen ... default_server),
# so Debian's stock site is disabled. Only its symlink is removed; the
# configuration stays in /etc/nginx/sites-available/default.
rm -f /etc/nginx/sites-enabled/default
ln -sfn /etc/nginx/sites-available/2drivescan /etc/nginx/sites-enabled/2drivescan
nginx -t
systemctl reload nginx
systemctl restart "php${PHP_VERSION}-fpm"

systemctl daemon-reload
/usr/local/bin/2drivescan-deploy
systemctl enable --now 2drivescan-update.timer

echo
echo "Done. 2drivescan.app is served on port 80; publish it over HTTPS with Cloudflare Tunnel."
echo "Deploy log: journalctl -u 2drivescan-update"
