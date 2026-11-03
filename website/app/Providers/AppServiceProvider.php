<?php

namespace App\Providers;

use Illuminate\Http\Middleware\TrustProxies;
use Illuminate\Support\Facades\URL;
use Illuminate\Support\ServiceProvider;

class AppServiceProvider extends ServiceProvider
{
    /**
     * Register any application services.
     */
    public function register(): void
    {
        //
    }

    /**
     * Bootstrap any application services.
     */
    public function boot(): void
    {
        // Cloudflare Tunnel (cloudflared) forwards the visitor's scheme and address
        // in X-Forwarded-* headers. It may run on this machine or on another host
        // in the network, so the trusted addresses come from TRUSTED_PROXIES.
        TrustProxies::at(config('site.trusted_proxies'));

        // The site is only served over HTTPS (required for .app domains). Behind
        // Cloudflare Tunnel requests arrive as plain HTTP, so links must be
        // forced to https:// to avoid mixed content.
        if ($this->app->isProduction() && str_starts_with((string) config('app.url'), 'https://')) {
            URL::forceScheme('https');
        }
    }
}
