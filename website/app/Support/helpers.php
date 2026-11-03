<?php

use Illuminate\Support\Facades\Route;

if (! function_exists('lroute')) {
    /**
     * URL of a page in the given (or current) language. Pages are registered
     * once per locale with names like "en.download" and "nl.download".
     */
    function lroute(string $page, ?string $locale = null, array $params = []): string
    {
        return route(($locale ?? app()->getLocale()).'.'.$page, $params);
    }
}

if (! function_exists('current_page')) {
    /** Page name of the current route without its locale prefix ("download"). */
    function current_page(): string
    {
        $name = Route::currentRouteName() ?? 'en.home';

        return str_contains($name, '.') ? explode('.', $name, 2)[1] : 'home';
    }
}

if (! function_exists('human_bytes')) {
    function human_bytes(int $bytes): string
    {
        $units = ['B', 'KB', 'MB', 'GB'];
        $i = 0;
        $value = (float) $bytes;
        while ($value >= 1000 && $i < count($units) - 1) {
            $value /= 1000;
            $i++;
        }

        return number_format($value, $i === 0 ? 0 : 1, app()->getLocale() === 'nl' ? ',' : '.', '').' '.$units[$i];
    }
}
