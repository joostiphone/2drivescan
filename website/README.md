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
- **The server deploys itself:** `deploy/update.sh` runs every 5 minutes from a systemd timer, pulls `main`
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

On a Debian/Ubuntu server with PHP 8.3+, Composer and git:

```bash
curl -fsSL https://raw.githubusercontent.com/joostiphone/2drivescan/main/website/deploy/install.sh | bash
```

Then point the web server at `/var/www/2drivescan/website/public` (example: `deploy/nginx.conf`, listening on
port 80 locally).

### HTTPS with Cloudflare Tunnel

`.app` domains only work over HTTPS. The domain is managed in Cloudflare, so the server publishes the site
through a Cloudflare Tunnel: `cloudflared` on the server opens an outbound connection to Cloudflare, no ports
have to be opened on the router, and Cloudflare handles the certificate.

1. Cloudflare dashboard → *Zero Trust → Networks → Tunnels → Create a tunnel* → type **Cloudflared**, name
   `2drivescan`.
2. Choose Debian/Ubuntu and run the install command it shows on the server (it contains the tunnel token).
3. Add two public hostnames, both with service **HTTP** → `localhost:80`:
   `2drivescan.app` and `www.2drivescan.app`. Cloudflare creates the DNS records itself.

Laravel trusts `X-Forwarded-*` headers from `127.0.0.1` (where cloudflared connects from), forces `https://`
links in production, and redirects `www.` to the bare domain.

### Useful commands

```bash
journalctl -u 2drivescan-update             # deploy log
/var/www/2drivescan/website/deploy/update.sh --force
php /var/www/2drivescan/website/artisan releases:refresh
```
