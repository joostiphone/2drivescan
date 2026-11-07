@php
    $locale = app()->getLocale();
    $page = current_page();
    $title = trim($__env->yieldContent('title')) ?: __('site.meta_title');
    $description = trim($__env->yieldContent('description')) ?: __('site.meta_description');
    $css = public_path('assets/site.css');
    $js = public_path('assets/site.js');
    // Built here: in the template itself Blade would read "@context" as a directive.
    $jsonLd = [
        '@context' => 'https://schema.org',
        '@type' => 'SoftwareApplication',
        'name' => '2DriveScan',
        'applicationCategory' => 'UtilitiesApplication',
        'operatingSystem' => 'macOS, Windows, Linux',
        'description' => __('site.meta_description'),
        'url' => url('/'),
        'image' => asset('img/icon-1024.png'),
        'softwareVersion' => $release['version'] ?? null,
        'license' => 'https://opensource.org/licenses/MIT',
        'offers' => ['@type' => 'Offer', 'price' => '0', 'priceCurrency' => 'EUR'],
    ];
@endphp
<!doctype html>
<html lang="{{ $locale }}" class="no-js">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ $title }}</title>
    <meta name="description" content="{{ $description }}">
    <meta name="theme-color" content="#0a0c16">
    <meta name="color-scheme" content="dark">
    <link rel="canonical" href="{{ lroute($page, $locale) }}">
    @foreach (array_keys(config('site.locales')) as $code)
        <link rel="alternate" hreflang="{{ $code }}" href="{{ lroute($page, $code) }}">
    @endforeach
    <link rel="alternate" hreflang="x-default" href="{{ lroute($page, 'en') }}">

    <meta property="og:type" content="website">
    <meta property="og:site_name" content="2DriveScan">
    <meta property="og:title" content="{{ $title }}">
    <meta property="og:description" content="{{ $description }}">
    <meta property="og:url" content="{{ lroute($page, $locale) }}">
    <meta property="og:image" content="{{ asset('img/og.png') }}">
    <meta property="og:image:width" content="1200">
    <meta property="og:image:height" content="630">
    <meta property="og:locale" content="{{ $locale === 'nl' ? 'nl_NL' : 'en_US' }}">
    <meta name="twitter:card" content="summary_large_image">

    <link rel="icon" href="{{ asset('favicon.ico') }}" sizes="any">
    <link rel="icon" type="image/png" href="{{ asset('favicon.png') }}">
    <link rel="apple-touch-icon" href="{{ asset('img/logo.png') }}">
    <link rel="stylesheet" href="{{ asset('assets/site.css') }}?v={{ is_file($css) ? filemtime($css) : 1 }}">
    <script src="{{ asset('assets/site.js') }}?v={{ is_file($js) ? filemtime($js) : 1 }}" defer></script>

    <script type="application/ld+json">
        {!! json_encode($jsonLd, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_HEX_TAG) !!}
    </script>
</head>
<body class="page-{{ $page }}">
    <a class="skip-link" href="#main">{{ __('site.skip') }}</a>
    <div class="backdrop" aria-hidden="true"></div>

    @include('partials.nav')

    <main id="main">
        @yield('content')
    </main>

    @include('partials.footer')
</body>
</html>
