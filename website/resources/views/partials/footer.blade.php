<footer class="footer">
    <div class="container footer-grid">
        <div class="footer-brand">
            <a class="brand" href="{{ lroute('home') }}">
                <img src="{{ asset('img/logo.png') }}" alt="" width="30" height="30">
                <span>2DriveScan</span>
            </a>
            <p>{{ __('site.footer.tagline') }}</p>
            <p class="muted">{{ __('site.footer.made') }}</p>
        </div>
        <div>
            <h3>{{ __('site.footer.product') }}</h3>
            <a href="{{ lroute('home') }}#features">{{ __('site.nav.features') }}</a>
            <a href="{{ lroute('home') }}#demo">{{ __('site.nav.demo') }}</a>
            <a href="{{ lroute('download') }}">{{ __('site.nav.download') }}</a>
            <a href="{{ lroute('changelog') }}">{{ __('site.nav.changelog') }}</a>
        </div>
        <div>
            <h3>{{ __('site.footer.project') }}</h3>
            <a href="{{ $repoUrl }}" rel="noopener">{{ __('site.footer.source') }}</a>
            <a href="{{ $repoUrl }}/releases" rel="noopener">{{ __('site.footer.releases') }}</a>
            <a href="{{ $repoUrl }}/issues/new/choose" rel="noopener">{{ __('site.footer.issues') }}</a>
            <a href="{{ lroute('stats') }}">{{ __('site.footer.stats') }}</a>
            <a href="{{ $repoUrl }}/blob/main/LICENSE" rel="noopener">{{ __('site.footer.license') }}</a>
            @if (config('site.donate_url'))
                <a href="{{ lroute('home') }}#donate">{{ __('site.nav.donate') }}</a>
            @endif
        </div>
        <div>
            <h3>{{ __('site.nav.language') }}</h3>
            @foreach (config('site.locales') as $code => $label)
                <a href="{{ lroute(current_page(), $code) }}" hreflang="{{ $code }}" lang="{{ $code }}">{{ $label }}</a>
            @endforeach
        </div>
    </div>
    <div class="container footer-bottom">
        <span>© {{ date('Y') }} 2DriveScan</span>
        @if ($release)
            <span>v{{ $release['version'] }}</span>
        @endif
    </div>
</footer>
