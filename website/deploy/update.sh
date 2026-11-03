#!/usr/bin/env bash
# Pull-based deploy for 2drivescan.app.
#
# Runs every few minutes from a systemd timer (see install.sh). When the main
# branch on GitHub has new commits it updates the checkout, installs PHP
# dependencies, copies the app UI and screenshots into the site and rebuilds
# Laravel's caches. Nothing happens when there is nothing new.
#
#   update.sh           deploy only when GitHub has new commits
#   update.sh --force   deploy even if nothing changed
set -euo pipefail

REPO_DIR="${REPO_DIR:-/var/www/2drivescan}"
BRANCH="${BRANCH:-main}"
WEB_USER="${WEB_USER:-www-data}"
export COMPOSER_ALLOW_SUPERUSER=1
export COMPOSER_HOME="${COMPOSER_HOME:-/root/.composer}"

exec 9>/run/lock/2drivescan-update.lock
flock -n 9 || { echo "Another update is running."; exit 0; }

cd "$REPO_DIR"
git fetch --quiet origin "$BRANCH"
current="$(git rev-parse HEAD)"
latest="$(git rev-parse "origin/$BRANCH")"
if [[ "$current" == "$latest" && "${1:-}" != "--force" ]]; then
  exit 0
fi

echo "Deploying ${latest:0:7} (was ${current:0:7})"
git reset --hard --quiet "origin/$BRANCH"

cd "$REPO_DIR/website"
composer install --no-dev --no-interaction --prefer-dist --optimize-autoloader --quiet
php artisan site:sync --quiet
php artisan optimize:clear --quiet
php artisan optimize --quiet
php artisan releases:refresh --quiet || true

chown -R "$WEB_USER":"$WEB_USER" storage bootstrap/cache
echo "Deployed ${latest:0:7}"
