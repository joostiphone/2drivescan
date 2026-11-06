<?php

return [
    // GitHub repository that hosts the app and its releases.
    'repo' => env('GITHUB_REPO', 'joostiphone/2drivescan'),

    // Optional token; raises the GitHub API rate limit for release lookups.
    'token' => env('GITHUB_TOKEN'),

    // Seconds release information is cached before GitHub is asked again.
    'cache_ttl' => (int) env('RELEASES_CACHE_TTL', 600),

    // Supported languages. The first one is served without a URL prefix.
    'locales' => [
        'en' => 'English',
        'nl' => 'Nederlands',
    ],

    // Proxies whose X-Forwarded-* headers are trusted: the machine running
    // cloudflared (Cloudflare Tunnel). Comma separated.
    'trusted_proxies' => array_values(array_filter(array_map('trim', explode(',', (string) env('TRUSTED_PROXIES', '127.0.0.1,::1'))))),

    // Voluntary donations: a plain link to PayPal (no PayPal script, iframe or
    // cookies). Leave DONATE_URL empty to hide the donate button and section.
    'donate_url' => env('DONATE_URL', 'https://www.paypal.com/donate/?hosted_button_id=SN6ACMPSATP3J'),

    // Path to the repository checkout that contains CHANGELOG.md, ui/ and docs/.
    'repo_path' => env('SITE_REPO_PATH', dirname(base_path())),
];
