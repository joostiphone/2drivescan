@php
    $locale = app()->getLocale();
    $page = current_page();
@endphp
<header class="nav" data-nav>
    <div class="nav-inner">
        <a class="brand" href="{{ lroute('home') }}">
            <img src="{{ asset('img/logo.png') }}" alt="" width="30" height="30">
            <span>2DriveScan</span>
        </a>

        <button class="nav-toggle" type="button" aria-expanded="false" aria-controls="nav-links" data-nav-toggle>
            @include('partials.icon', ['name' => 'menu'])
            <span class="sr-only">{{ __('site.nav.menu') }}</span>
        </button>

        <nav class="nav-links" id="nav-links">
            <a href="{{ lroute('home') }}#features">{{ __('site.nav.features') }}</a>
            <a href="{{ lroute('home') }}#demo">{{ __('site.nav.demo') }}</a>
            <a href="{{ lroute('download') }}" @class(['active' => $page === 'download'])>{{ __('site.nav.download') }}</a>
            <a href="{{ lroute('changelog') }}" @class(['active' => $page === 'changelog'])>{{ __('site.nav.changelog') }}</a>
            <a href="{{ $repoUrl }}" class="nav-github" rel="noopener">
                @include('partials.icon', ['name' => 'github'])
                <span>{{ __('site.nav.github') }}</span>
            </a>
            <div class="lang" aria-label="{{ __('site.nav.language') }}">
                @foreach (config('site.locales') as $code => $label)
                    <a href="{{ lroute($page, $code) }}" hreflang="{{ $code }}" lang="{{ $code }}" title="{{ $label }}"
                       @class(['active' => $code === $locale]) @if ($code === $locale) aria-current="true" @endif>{{ strtoupper($code) }}</a>
                @endforeach
            </div>
            <a class="btn btn-primary btn-sm" href="{{ lroute('download') }}">
                @include('partials.icon', ['name' => 'download'])
                {{ __('site.nav.download') }}
            </a>
        </nav>
    </div>
</header>
