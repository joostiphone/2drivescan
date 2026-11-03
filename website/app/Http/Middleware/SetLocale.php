<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\App;
use Symfony\Component\HttpFoundation\Response;

class SetLocale
{
    public function handle(Request $request, Closure $next, string $locale = 'en'): Response
    {
        if (array_key_exists($locale, config('site.locales'))) {
            App::setLocale($locale);
        }

        return $next($request);
    }
}
