<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

/**
 * Sends www.2drivescan.app (and any other alias) to the canonical host from
 * APP_URL, so search engines and visitors see a single address.
 */
class CanonicalHost
{
    public function handle(Request $request, Closure $next): Response
    {
        $canonical = parse_url((string) config('app.url'), PHP_URL_HOST);

        if (app()->isProduction() && $canonical && $request->getHost() === 'www.'.$canonical) {
            return redirect()->away('https://'.$canonical.$request->getRequestUri(), 301);
        }

        return $next($request);
    }
}
