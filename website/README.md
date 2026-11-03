# 2drivescan.app

The website for [2DriveScan](../README.md), built with Laravel 13. It has no database and no build step:
templates are Blade, styles and scripts are plain files in `public/assets/`.

## How it stays up to date

- **Downloads and version numbers** come live from the GitHub Releases API (cached for 10 minutes, with the
  last good answer kept if GitHub is unreachable). A new app release shows up on the site by itself.
  `/get/mac`, `/get/windows`, `/get/appimage`, … always redirect to the newest installer.
- **The live demo** on the home page is the real app UI from `../ui`, running with its mock backend.
  `php artisan site:sync` copies it (plus screenshots from `../docs/screenshots` and the icons) into `public/`.
- **The changelog page** renders `../CHANGELOG.md` and `../CHANGELOG.nl.md`.
- **The server deploys itself:** a systemd timer runs `2drivescan-deploy --if-changed` every 5 minutes. When
  `main` moved it builds a fresh release next to the running one, health-checks it and switches over, or
  rolls back. Pushing to `main` is all it takes.
  from GitHub and rebuilds when something changed. Pushing to `main` is all it takes.

## Pages

| URL | |
| --- | --- |
| `/`, `/nl` | Home: hero, live demo, features, screenshots, downloads, FAQ |
| `/download`, `/nl/download` | All installers with sizes, SHA-256 and install steps |
| `/changelog`, `/nl/changelog` | Release history |
| `/get/{platform}` | Redirect to the newest installer (`mac`, `windows`, `windows-msi`, `windows-arm64`, `appimage`, `deb`, `rpm`, and `-arm64` variants) |
| `/sitemap.xml` | Sitemap for search engines |

Texts live in `lang/en/site.php` and `lang/nl/site.php`; keep both in sync.

## Local development

```bash
composer setup        # install, create .env, generate key, sync assets
composer dev          # sync assets and serve on http://localhost:8000
php artisan test
./vendor/bin/pint     # code style
```

## Server setup

Production runs on a Debian 13 container (CT 142) with nginx and PHP 8.4:

```
/var/www/2drivescan/
  releases/<timestamp>/   full clone of the repository (main), web root: website/public
  current -> releases/…   the live release
  shared/.env             production settings (APP_KEY, APP_URL, TRUSTED_PROXIES, GITHUB_TOKEN)
  shared/storage/         logs and cache, linked into every release
/usr/local/bin/2drivescan-deploy      deploy script (source: deploy/2drivescan-deploy)
/etc/default/2drivescan-deploy        its settings
2drivescan-update.timer               runs `2drivescan-deploy --if-changed` every 5 minutes
```

All server configuration lives in `deploy/`, exactly as used in production:

| File | Installed as |
| --- | --- |
| `2drivescan-deploy` | `/usr/local/bin/2drivescan-deploy` |
| `2drivescan-deploy.default` | `/etc/default/2drivescan-deploy` |
| `nginx.conf` | `/etc/nginx/sites-available/2drivescan` (anonymised access log, only `index.php` runs) |
| `php-fpm-pool.conf` | `/etc/php/8.4/fpm/pool.d/2drivescan.conf` (`open_basedir`, `disable_functions`) |
| `logrotate` | `/etc/logrotate.d/2drivescan` (Laravel logs only; nginx logs are rotated by Debian) |
| `systemd/2drivescan-update.{service,timer}` | `/etc/systemd/system/` |

To set up a new server (Debian/Ubuntu with nginx, PHP-FPM 8.3+, Composer and git), run as root:

```bash
curl -fsSL https://raw.githubusercontent.com/joostiphone/2drivescan/main/website/deploy/install.sh | bash
```

It never overwrites existing configuration files. After changing `deploy/2drivescan-deploy` in the repo,
copy it to `/usr/local/bin/` on the server (running `install.sh` again does that); the script does not
update itself.

### HTTPS with Cloudflare Tunnel

`.app` domains only work over HTTPS. The domain is managed in Cloudflare and published through a
Cloudflare Tunnel: `cloudflared` opens an outbound connection to Cloudflare, so no router ports are open,
and Cloudflare handles the certificate. In production the connector runs on another container in the same
network (CT 101) and forwards `2drivescan.app` and `www.2drivescan.app` to port 80 of CT 142.

Laravel trusts `X-Forwarded-*` headers only from `TRUSTED_PROXIES` (default `127.0.0.1,::1`; production adds
the connector's address), forces `https://` links in production, redirects `www.` to the bare domain, and
sets no cookies.

### Useful commands

```bash
journalctl -u 2drivescan-update -n 50     # automatic deploys
2drivescan-deploy                         # deploy main now
cat /var/www/2drivescan/current/.release  # live release and commit
php /var/www/2drivescan/current/website/artisan releases:refresh
```
